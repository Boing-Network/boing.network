//! Native AMM: deploy CP pool → add liquidity → swap over JSON-RPC (checklist **A4.3** node-side).
//!
//! Boing Express + browser origin is manual; this test is the same protocol path wallets use.

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use boing_execution::{
    constant_product_amount_out, constant_product_pool_bytecode, encode_add_liquidity_calldata,
    encode_swap_calldata, reserve_a_key, reserve_b_key,
};
use boing_node::rpc::rpc_router;
use boing_node::security::RateLimitConfig;
use boing_primitives::{
    nonce_derived_contract_address, AccessList, Account, AccountId, AccountState,
    SignedTransaction, Transaction, TransactionPayload,
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
            stake: 0,
        },
    });

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
    }
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

fn hex_account(id: &AccountId) -> String {
    format!("0x{}", hex::encode(id.0))
}

fn hex_key32(k: &[u8; 32]) -> String {
    format!("0x{}", hex::encode(k))
}

#[tokio::test]
async fn native_amm_deploy_add_liquidity_swap_via_rpc() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let proposer = AccountId(signing_key.verifying_key().to_bytes());
    let node = Arc::new(RwLock::new(node_with_proposer_key(&signing_key, 10_000_000)));
    let mut app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None);

    let bytecode = constant_product_pool_bytecode();
    let deploy_tx = Transaction {
        nonce: 0,
        sender: proposer,
        payload: TransactionPayload::ContractDeployWithPurpose {
            bytecode,
            purpose_category: "dapp".to_string(),
            description_hash: None,
            create2_salt: None,
        },
        access_list: AccessList::default(),
    };
    let signed_deploy = SignedTransaction::new(deploy_tx, &signing_key);
    let hex_deploy = format!(
        "0x{}",
        hex::encode(bincode::serialize(&signed_deploy).unwrap())
    );

    let v = rpc_call(
        &mut app,
        "boing_submitTransaction",
        serde_json::json!([hex_deploy]),
    )
    .await;
    assert!(v.get("error").is_none(), "{v:?}");
    {
        let mut n = node.write().await;
        n.produce_block_if_ready().expect("block with deploy");
    }

    let contract = nonce_derived_contract_address(&proposer, 0);
    let al = AccessList::new(vec![proposer, contract], vec![proposer, contract]);

    let add_calldata = encode_add_liquidity_calldata(1_000, 2_000, 0);
    let add_tx = Transaction {
        nonce: 1,
        sender: proposer,
        payload: TransactionPayload::ContractCall {
            contract,
            calldata: add_calldata,
        },
        access_list: al.clone(),
    };
    let signed_add = SignedTransaction::new(add_tx, &signing_key);
    let hex_add = format!(
        "0x{}",
        hex::encode(bincode::serialize(&signed_add).unwrap())
    );

    let v2 = rpc_call(
        &mut app,
        "boing_submitTransaction",
        serde_json::json!([hex_add]),
    )
    .await;
    assert!(v2.get("error").is_none(), "{v2:?}");
    {
        let mut n = node.write().await;
        n.produce_block_if_ready().expect("block with add_liquidity");
    }

    let dx: u128 = 100;
    let dy = u128::from(constant_product_amount_out(1_000u64, 2_000u64, dx as u64));
    let swap_calldata = encode_swap_calldata(0, dx, dy);
    let swap_tx = Transaction {
        nonce: 2,
        sender: proposer,
        payload: TransactionPayload::ContractCall {
            contract,
            calldata: swap_calldata,
        },
        access_list: al,
    };
    let signed_swap = SignedTransaction::new(swap_tx, &signing_key);
    let hex_swap = format!(
        "0x{}",
        hex::encode(bincode::serialize(&signed_swap).unwrap())
    );

    let v3 = rpc_call(
        &mut app,
        "boing_submitTransaction",
        serde_json::json!([hex_swap]),
    )
    .await;
    assert!(v3.get("error").is_none(), "{v3:?}");
    {
        let mut n = node.write().await;
        n.produce_block_if_ready().expect("block with swap");
    }

    let ra = rpc_call(
        &mut app,
        "boing_getContractStorage",
        serde_json::json!([hex_account(&contract), hex_key32(&reserve_a_key())]),
    )
    .await;
    let ra_v = ra
        .get("result")
        .and_then(|r| r.get("value"))
        .and_then(|x| x.as_str())
        .expect("reserve A");
    let rb = rpc_call(
        &mut app,
        "boing_getContractStorage",
        serde_json::json!([hex_account(&contract), hex_key32(&reserve_b_key())]),
    )
    .await;
    let rb_v = rb
        .get("result")
        .and_then(|r| r.get("value"))
        .and_then(|x| x.as_str())
        .expect("reserve B");

    let ra_word = hex::decode(ra_v.trim_start_matches("0x")).unwrap();
    let rb_word = hex::decode(rb_v.trim_start_matches("0x")).unwrap();
    let ra_u = u128::from_be_bytes(ra_word[16..32].try_into().unwrap());
    let rb_u = u128::from_be_bytes(rb_word[16..32].try_into().unwrap());
    assert_eq!(ra_u, 1_000 + dx);
    assert_eq!(rb_u, 2_000 - dy);
}
