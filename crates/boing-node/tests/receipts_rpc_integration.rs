//! RPC integration: deploy, contract call, simulate failing tx; receipts over JSON-RPC.

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
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

/// Runtime code: return 32-byte word with low byte `0x42`.
fn return_42_runtime_bytecode() -> Vec<u8> {
    let mut code = vec![0x7f];
    code.extend(std::iter::repeat(0u8).take(31));
    code.push(0x42);
    code.extend([
        0x60, 0x00, 0x52, // MSTORE
        0x60, 0x20, 0x60, 0x00, 0xf3, // RETURN: offset on top → push size, push offset
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
async fn rpc_receipts_deploy_call_and_simulate_failure() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let proposer = AccountId(signing_key.verifying_key().to_bytes());
    let node = Arc::new(RwLock::new(node_with_proposer_key(&signing_key, 1_000_000)));
    let mut app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None);

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
    let deploy_id = signed_deploy.tx.id();
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

    let rec_v = rpc_call(
        &mut app,
        "boing_getTransactionReceipt",
        serde_json::json!([format!("0x{}", hex::encode(deploy_id.0))]),
    )
    .await;
    let rec = rec_v.get("result").expect("result");
    assert_eq!(rec.get("success"), Some(&serde_json::json!(true)));

    let contract = nonce_derived_contract_address(&proposer, 0);
    let call_tx = Transaction {
        nonce: 1,
        sender: proposer,
        payload: TransactionPayload::ContractCall {
            contract,
            calldata: vec![],
        },
        access_list: AccessList::new(vec![proposer, contract], vec![proposer, contract]),
    };
    let signed_call = SignedTransaction::new(call_tx, &signing_key);
    let call_id = signed_call.tx.id();
    let hex_call = format!(
        "0x{}",
        hex::encode(bincode::serialize(&signed_call).unwrap())
    );

    let v2 = rpc_call(
        &mut app,
        "boing_submitTransaction",
        serde_json::json!([hex_call]),
    )
    .await;
    assert!(v2.get("error").is_none(), "{v2:?}");
    {
        let mut n = node.write().await;
        n.produce_block_if_ready().expect("block with call");
    }

    let rec_call = rpc_call(
        &mut app,
        "boing_getTransactionReceipt",
        serde_json::json!([format!("0x{}", hex::encode(call_id.0))]),
    )
    .await;
    let rec_call = rec_call.get("result").expect("result");
    assert_eq!(rec_call.get("success"), Some(&serde_json::json!(true)));
    let rd = rec_call
        .get("return_data")
        .and_then(|x| x.as_str())
        .unwrap();
    assert!(rd.starts_with("0x"));
    let raw = hex::decode(rd.trim_start_matches("0x")).unwrap();
    assert_eq!(raw.len(), 32);
    assert_eq!(raw[31], 0x42);

    let bad_tx = Transaction {
        nonce: 99,
        sender: proposer,
        payload: TransactionPayload::ContractCall {
            contract,
            calldata: vec![],
        },
        access_list: AccessList::new(vec![proposer, contract], vec![proposer, contract]),
    };
    let signed_bad = SignedTransaction::new(bad_tx, &signing_key);
    let hex_bad = format!(
        "0x{}",
        hex::encode(bincode::serialize(&signed_bad).unwrap())
    );
    let sim = rpc_call(
        &mut app,
        "boing_simulateTransaction",
        serde_json::json!([hex_bad]),
    )
    .await;
    let sim_r = sim.get("result").expect("result");
    assert_eq!(sim_r.get("success"), Some(&serde_json::json!(false)));

    let blk = rpc_call(
        &mut app,
        "boing_getBlockByHeight",
        serde_json::json!([2, true]),
    )
    .await;
    let blk_o = blk.get("result").expect("result").as_object().unwrap();
    let receipts = blk_o.get("receipts").and_then(|x| x.as_array()).unwrap();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].get("success"), Some(&serde_json::json!(true)));
    let hdr = blk_o.get("header").and_then(|h| h.as_object()).unwrap();
    assert!(hdr.get("receipts_root").is_some());
}

#[tokio::test]
async fn rpc_get_sync_state_matches_chain_height() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let node = Arc::new(RwLock::new(node_with_proposer_key(&signing_key, 1_000_000)));
    let mut app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None);

    let sync_v = rpc_call(&mut app, "boing_getSyncState", serde_json::json!([])).await;
    let sync = sync_v.get("result").expect("result").as_object().unwrap();
    let height_v = rpc_call(&mut app, "boing_chainHeight", serde_json::json!([])).await;
    let h = height_v.get("result").and_then(|x| x.as_u64()).unwrap();

    assert_eq!(sync.get("head_height"), Some(&serde_json::json!(h)));
    assert_eq!(sync.get("finalized_height"), Some(&serde_json::json!(h)));
    let hash = sync
        .get("latest_block_hash")
        .and_then(|x| x.as_str())
        .expect("latest_block_hash");
    assert!(hash.starts_with("0x"));
    assert_eq!(
        hex::decode(hash.trim_start_matches("0x")).unwrap().len(),
        32
    );
}

#[tokio::test]
async fn rpc_simulate_includes_access_list_hints_and_contract_storage_rpc() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let proposer = AccountId(signing_key.verifying_key().to_bytes());
    let to = AccountId([2u8; 32]);
    let node = Arc::new(RwLock::new(node_with_proposer_key(&signing_key, 1_000_000)));
    {
        let mut n = node.write().await;
        n.state.insert(Account {
            id: to,
            state: AccountState::default(),
        });
    }
    let mut app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None);

    let tx = Transaction {
        nonce: 0,
        sender: proposer,
        payload: TransactionPayload::Transfer { to, amount: 1 },
        access_list: AccessList::default(),
    };
    let signed = SignedTransaction::new(tx, &signing_key);
    let hex_tx = format!("0x{}", hex::encode(bincode::serialize(&signed).unwrap()));
    let v = rpc_call(
        &mut app,
        "boing_simulateTransaction",
        serde_json::json!([hex_tx]),
    )
    .await;
    let r = v.get("result").expect("result").as_object().unwrap();
    assert_eq!(r.get("success"), Some(&serde_json::json!(true)));
    assert!(r.get("suggested_access_list").is_some());
    assert_eq!(
        r.get("access_list_covers_suggestion"),
        Some(&serde_json::json!(false))
    );

    let key_zero = format!("0x{}", hex::encode([0u8; 32]));
    let contract_hex = format!("0x{}", hex::encode(proposer.0));
    let stor = rpc_call(
        &mut app,
        "boing_getContractStorage",
        serde_json::json!([contract_hex, key_zero]),
    )
    .await;
    let st = stor.get("result").expect("result").as_object().unwrap();
    let expected_zero = format!("0x{}", "00".repeat(32));
    assert_eq!(st.get("value"), Some(&serde_json::json!(expected_zero)));
}

/// `LOG1` with one topic (31 zero bytes + `topic_tail`), empty data, then `STOP`.
fn log1_deploy_bytecode(topic_tail: u8) -> Vec<u8> {
    let mut v = vec![0x7f];
    v.extend(std::iter::repeat(0u8).take(31));
    v.push(topic_tail);
    v.push(0x7f);
    v.extend(std::iter::repeat(0u8).take(32));
    v.push(0x7f);
    v.extend(std::iter::repeat(0u8).take(32));
    v.push(0xa1);
    v.push(0x00);
    v
}

fn topic_word_hex(topic_tail: u8) -> String {
    format!("0x{}{:02x}", "00".repeat(31), topic_tail)
}

#[tokio::test]
async fn rpc_get_logs_bounded_filters() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let proposer = AccountId(signing_key.verifying_key().to_bytes());
    let node = Arc::new(RwLock::new(node_with_proposer_key(&signing_key, 1_000_000)));
    let mut app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None);

    // Deploy stores bytecode; constructor execution does not run the interpreter on this VM.
    let deploy_tx = Transaction {
        nonce: 0,
        sender: proposer,
        payload: TransactionPayload::ContractDeploy {
            bytecode: log1_deploy_bytecode(0xcd),
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
    let contract_hex = format!("0x{}", hex::encode(contract.0));
    let call_tx = Transaction {
        nonce: 1,
        sender: proposer,
        payload: TransactionPayload::ContractCall {
            contract,
            calldata: vec![],
        },
        access_list: AccessList::new(vec![proposer, contract], vec![proposer, contract]),
    };
    let signed_call = SignedTransaction::new(call_tx, &signing_key);
    let hex_call = format!(
        "0x{}",
        hex::encode(bincode::serialize(&signed_call).unwrap())
    );
    let v_call = rpc_call(
        &mut app,
        "boing_submitTransaction",
        serde_json::json!([hex_call]),
    )
    .await;
    assert!(v_call.get("error").is_none(), "{v_call:?}");
    {
        let mut n = node.write().await;
        n.produce_block_if_ready().expect("block with call");
    }

    let topic_hex = topic_word_hex(0xcd);

    let logs_ok = rpc_call(
        &mut app,
        "boing_getLogs",
        serde_json::json!([{
            "fromBlock": 2,
            "toBlock": 2,
            "address": contract_hex,
            "topics": [topic_hex.clone()],
        }]),
    )
    .await;
    let arr = logs_ok
        .get("result")
        .expect("result")
        .as_array()
        .expect("array");
    assert_eq!(arr.len(), 1);
    assert_eq!(
        arr[0].get("address"),
        Some(&serde_json::json!(contract_hex))
    );
    assert_eq!(arr[0].get("block_height"), Some(&serde_json::json!(2)));

    let logs_wrong_topic = rpc_call(
        &mut app,
        "boing_getLogs",
        serde_json::json!([{
            "fromBlock": 2,
            "toBlock": 2,
            "topics": [topic_word_hex(0xee)],
        }]),
    )
    .await;
    assert_eq!(
        logs_wrong_topic
            .get("result")
            .expect("result")
            .as_array()
            .unwrap()
            .len(),
        0
    );

    let other = AccountId([1u8; 32]);
    let other_hex = format!("0x{}", hex::encode(other.0));
    let logs_wrong_addr = rpc_call(
        &mut app,
        "boing_getLogs",
        serde_json::json!([{
            "fromBlock": 2,
            "toBlock": 2,
            "address": other_hex,
            "topics": [topic_hex],
        }]),
    )
    .await;
    assert_eq!(
        logs_wrong_addr
            .get("result")
            .expect("result")
            .as_array()
            .unwrap()
            .len(),
        0
    );

    let big_range = rpc_call(
        &mut app,
        "boing_getLogs",
        serde_json::json!([{ "fromBlock": 0, "toBlock": 200 }]),
    )
    .await;
    assert!(big_range.get("error").is_some());
    assert_eq!(
        big_range
            .get("error")
            .and_then(|e| e.get("code"))
            .and_then(|c| c.as_i64()),
        Some(-32602)
    );
}
