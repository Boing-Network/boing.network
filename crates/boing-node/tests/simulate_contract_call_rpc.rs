//! JSON-RPC `boing_simulateContractCall` — unsigned contract call simulation.

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use boing_node::rpc::rpc_router;
use boing_node::security::RateLimitConfig;
use boing_primitives::{
    nonce_derived_contract_address, AccessList, Account, AccountId, AccountState, SignedTransaction,
    Transaction, TransactionPayload,
};
use boing_state::StateStore;
use ed25519_dalek::SigningKey;
use http_body_util::BodyExt;
use rand::rngs::OsRng;
use tokio::sync::RwLock;
use tower::ServiceExt;

fn node_with_proposer_key(signing_key: &SigningKey, balance: u128) -> boing_node::node::BoingNode {
    let proposer = AccountId(signing_key.verifying_key().to_bytes());
    let genesis = boing_node::chain::ChainState::genesis(proposer);
    let chain = boing_node::chain::ChainState::from_genesis(genesis.clone());
    let mut consensus = boing_consensus::ConsensusEngine::single_validator(proposer);
    let _ = consensus.propose_and_commit(genesis);

    let mut state = StateStore::new();
    state.insert(Account {
        id: proposer,
        state: AccountState {
            balance,
            nonce: 0,
            stake: 0, ..Default::default() },
    });

    let native_aggregates = state.compute_native_aggregates();
    boing_node::node::BoingNode {
        chain,
        consensus,
        state,
        executor: boing_execution::BlockExecutor::new(),
        producer: boing_node::block_producer::BlockProducer::new(proposer).with_max_txs(100),
        vm: boing_execution::Vm::new(),
        scheduler: boing_execution::TransactionScheduler::new(),
        mempool: boing_node::mempool::Mempool::new(),
        p2p: boing_p2p::P2pNode::default(),
        dapp_registry: boing_node::dapp_registry::DappRegistry::new(),
        intent_pool: boing_node::intent_pool::IntentPool::new(),
        qa_pool: boing_node::node::pending_qa_pool_default(),
        persistence: None,
        receipts: HashMap::new(),
        native_aggregates,
        head_broadcast: None,
    validator_signing_key: None,
    pending_commit: None,
        early_votes: HashMap::new(),
        stake_validator_set: None,
        slashed_equivocations: HashMap::new(),
        observed_votes: HashMap::new(),
        slash_registry: boing_governance::SlashRegistry::new(),
}
}

fn return_42_runtime_bytecode() -> Vec<u8> {
    let mut code = vec![0x7f];
    code.extend(std::iter::repeat_n(0u8, 31));
    code.push(0x42);
    code.extend([
        0x60, 0x00, 0x52,
        0x60, 0x20, 0x60, 0x00, 0xf3,
        0x00,
    ]);
    code
}

async fn rpc_call(
    app: &mut axum::Router,
    method: &str,
    params: serde_json::Value,
) -> serde_json::Value {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params,
    });
    let req = Request::builder()
        .method("POST")
        .uri("/")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).expect("json")
}

#[tokio::test]
async fn rpc_simulate_contract_call_invalid_params() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let node = Arc::new(RwLock::new(node_with_proposer_key(&signing_key, 1_000_000)));
    let mut app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None, None);

    let v = rpc_call(&mut app, "boing_simulateContractCall", serde_json::json!([])).await;
    let err = v.get("error").expect("error");
    assert_eq!(err.get("code"), Some(&serde_json::json!(-32602)));
}

#[tokio::test]
async fn rpc_simulate_contract_call_unknown_contract() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let node = Arc::new(RwLock::new(node_with_proposer_key(&signing_key, 1_000_000)));
    let mut app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None, None);

    let dead = AccountId([0xab; 32]);
    let contract_hex = format!("0x{}", hex::encode(dead.0));
    let v = rpc_call(
        &mut app,
        "boing_simulateContractCall",
        serde_json::json!([contract_hex, "0x"]),
    )
    .await;
    let r = v.get("result").expect("result").as_object().unwrap();
    assert_eq!(r.get("success"), Some(&serde_json::json!(false)));
    let err = r.get("error").and_then(|x| x.as_str()).unwrap();
    assert!(
        err.contains("Account not found") || err.contains("not found"),
        "{err}"
    );
}

#[tokio::test]
async fn rpc_simulate_contract_call_return_data_after_deploy() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let proposer = AccountId(signing_key.verifying_key().to_bytes());
    let node = Arc::new(RwLock::new(node_with_proposer_key(&signing_key, 1_000_000)));
    let mut app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None, None);

    let deploy_tx = Transaction {
        nonce: 0,
        sender: proposer,
        payload: TransactionPayload::ContractDeploy {
            bytecode: return_42_runtime_bytecode(),
            create2_salt: None,
        },
        access_list: AccessList::default(),
    };
    let signed_deploy = SignedTransaction::new(deploy_tx, &signing_key);
    let hex_deploy = format!(
        "0x{}",
        hex::encode(bincode::serialize(&signed_deploy).unwrap())
    );

    let sub = rpc_call(
        &mut app,
        "boing_submitTransaction",
        serde_json::json!([hex_deploy]),
    )
    .await;
    assert!(sub.get("error").is_none(), "{sub:?}");
    {
        let mut n = node.write().await;
        n.produce_block_if_ready().expect("block");
    }

    let contract = nonce_derived_contract_address(&proposer, 0);
    let contract_hex = format!("0x{}", hex::encode(contract.0));
    let proposer_hex = format!("0x{}", hex::encode(proposer.0));

    let tip = rpc_call(&mut app, "boing_chainHeight", serde_json::json!([]))
        .await
        .get("result")
        .and_then(|x| x.as_u64())
        .unwrap();

    let v = rpc_call(
        &mut app,
        "boing_simulateContractCall",
        serde_json::json!([contract_hex, "0x", proposer_hex, tip]),
    )
    .await;
    let r = v.get("result").expect("result").as_object().unwrap();
    assert_eq!(r.get("success"), Some(&serde_json::json!(true)));
    assert_eq!(
        r.get("access_list_covers_suggestion"),
        Some(&serde_json::json!(true))
    );
    let rd = r
        .get("return_data")
        .and_then(|x| x.as_str())
        .expect("return_data");
    let raw = hex::decode(rd.trim_start_matches("0x")).unwrap();
    assert_eq!(raw.len(), 32);
    assert_eq!(raw[31], 0x42);

    let v2 = rpc_call(
        &mut app,
        "boing_simulateContractCall",
        serde_json::json!([contract_hex, "0x", serde_json::Value::Null, "latest"]),
    )
    .await;
    let r2 = v2.get("result").expect("result").as_object().unwrap();
    assert_eq!(r2.get("success"), Some(&serde_json::json!(true)));

    let wrong_height = if tip > 0 { tip - 1 } else { 999u64 };
    let bad = rpc_call(
        &mut app,
        "boing_simulateContractCall",
        serde_json::json!([contract_hex, "0x", proposer_hex, wrong_height]),
    )
    .await;
    let err = bad.get("error").expect("error");
    assert_eq!(err.get("code"), Some(&serde_json::json!(-32602)));
}

#[tokio::test]
async fn rpc_supported_methods_includes_simulate_contract_call() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let node = Arc::new(RwLock::new(node_with_proposer_key(&signing_key, 1_000_000)));
    let mut app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None, None);

    let sm = rpc_call(
        &mut app,
        "boing_rpcSupportedMethods",
        serde_json::json!([]),
    )
    .await;
    let arr = sm
        .get("result")
        .and_then(|x| x.as_array())
        .expect("supported methods");
    let names: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
    assert!(names.contains(&"boing_simulateContractCall"));
}
