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
    SignedTransaction, Transaction, TransactionPayload, CONTRACT_DEPLOY_INIT_CODE_MARKER,
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
    }
}

/// Runtime code: return 32-byte word with low byte `0x42`.
fn return_42_runtime_bytecode() -> Vec<u8> {
    let mut code = vec![0x7f];
    code.extend(std::iter::repeat_n(0u8, 31));
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
    let mut app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None, None);

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
async fn rpc_get_network_info_shape() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let node = Arc::new(RwLock::new(node_with_proposer_key(&signing_key, 1_000_000)));
    let mut app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None, None);

    let v = rpc_call(&mut app, "boing_getNetworkInfo", serde_json::json!([])).await;
    let info = v.get("result").expect("result").as_object().unwrap();
    assert_eq!(info.get("head_height"), info.get("finalized_height"));
    assert!(info.get("target_block_time_secs").is_some());
    let consensus = info
        .get("consensus")
        .and_then(|x| x.as_object())
        .expect("consensus");
    assert_eq!(consensus.get("model"), Some(&serde_json::json!("hotstuff_bft")));
    assert_eq!(consensus.get("validator_count"), Some(&serde_json::json!(1)));
    let rpc = info.get("rpc").and_then(|x| x.as_object()).expect("rpc");
    let na = rpc
        .get("not_available")
        .and_then(|x| x.as_array())
        .expect("not_available");
    let tags: Vec<&str> = na.iter().filter_map(|x| x.as_str()).collect();
    assert!(
        !tags.contains(&"chain_wide_total_stake"),
        "chain-wide stake sum is exposed under chain_native.total_stake"
    );
    assert!(tags.contains(&"staking_apy"));
    let cn = info
        .get("chain_native")
        .and_then(|x| x.as_object())
        .expect("chain_native");
    assert_eq!(cn.get("account_count"), Some(&serde_json::json!(1)));
    assert_eq!(cn.get("total_balance"), Some(&serde_json::json!("1000000")));
    assert_eq!(cn.get("total_stake"), Some(&serde_json::json!("0")));
    assert_eq!(cn.get("total_native_held"), Some(&serde_json::json!("1000000")));
    assert_eq!(cn.get("as_of_height"), Some(&serde_json::json!(0)));
    let dev = info
        .get("developer")
        .and_then(|x| x.as_object())
        .expect("developer");
    assert_eq!(dev.get("sdk_npm_package"), Some(&serde_json::json!("boing-sdk")));
    assert_eq!(
        dev.get("websocket")
            .and_then(|w| w.get("path"))
            .and_then(|x| x.as_str()),
        Some("/ws")
    );
    let disc = dev
        .get("api_discovery_methods")
        .and_then(|x| x.as_array())
        .expect("api_discovery_methods");
    let disc_names: Vec<&str> = disc.iter().filter_map(|x| x.as_str()).collect();
    assert!(disc_names.contains(&"boing_getRpcMethodCatalog"));
    let http = dev.get("http").and_then(|x| x.as_object()).expect("developer.http");
    assert_eq!(
        http.get("live_path").and_then(|x| x.as_str()),
        Some("/live")
    );
    assert_eq!(
        http.get("ready_path").and_then(|x| x.as_str()),
        Some("/ready")
    );
    assert_eq!(
        http.get("supports_jsonrpc_batch"),
        Some(&serde_json::json!(true))
    );
    assert_eq!(
        http.get("jsonrpc_batch_max_env").and_then(|x| x.as_str()),
        Some("BOING_RPC_MAX_BATCH")
    );
    assert_eq!(
        http.get("request_id_header").and_then(|x| x.as_str()),
        Some("x-request-id")
    );
    assert_eq!(
        http.get("openapi_http_path").and_then(|x| x.as_str()),
        Some("/openapi.json")
    );
    assert!(info.get("rpc_surface").is_some());
    assert!(info.get("end_user").is_some());
}

#[tokio::test]
async fn http_get_root_is_method_not_allowed_with_allow_post() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let node = Arc::new(RwLock::new(node_with_proposer_key(&signing_key, 1_000_000)));
    let app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None, None);

    let req = Request::builder()
        .uri("/")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(
        res.headers()
            .get(axum::http::header::ALLOW)
            .and_then(|v| v.to_str().ok()),
        Some("GET, HEAD, POST, OPTIONS")
    );
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let s = std::str::from_utf8(&bytes).unwrap();
    assert!(s.contains("POST /") && s.contains("JSON-RPC"));
}

#[tokio::test]
async fn http_options_root_lists_post_or_cors_methods() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let node = Arc::new(RwLock::new(node_with_proposer_key(&signing_key, 1_000_000)));
    let app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None, None);

    let req = Request::builder()
        .method("OPTIONS")
        .uri("/")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert!(
        matches!(res.status(), StatusCode::NO_CONTENT | StatusCode::OK),
        "unexpected status {}",
        res.status()
    );
    let allow = res
        .headers()
        .get(axum::http::header::ALLOW)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let acm = res
        .headers()
        .get(axum::http::header::ACCESS_CONTROL_ALLOW_METHODS)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        allow.contains("POST")
            || acm.to_ascii_uppercase().contains("POST"),
        "expected POST in Allow or Access-Control-Allow-Methods: allow={allow:?} acm={acm:?}"
    );
}

#[tokio::test]
async fn rpc_post_propagates_x_request_id() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let node = Arc::new(RwLock::new(node_with_proposer_key(&signing_key, 1_000_000)));
    let app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None, None);

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "boing_chainHeight",
        "params": [],
    });
    let req = Request::builder()
        .method("POST")
        .uri("/")
        .header("content-type", "application/json")
        .header("x-request-id", "integration-test-req-id")
        .body(Body::from(body.to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let rid = res
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .expect("x-request-id");
    assert_eq!(rid, "integration-test-req-id");
}

#[tokio::test]
async fn rpc_post_generates_x_request_id_when_absent() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let node = Arc::new(RwLock::new(node_with_proposer_key(&signing_key, 1_000_000)));
    let app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None, None);

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "boing_chainHeight",
        "params": [],
    });
    let req = Request::builder()
        .method("POST")
        .uri("/")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let rid = res
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .expect("x-request-id");
    assert!(rid.len() >= 32, "expected uuid hex: {rid}");
}

#[tokio::test]
async fn http_live_and_ready_plain_get() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let node = Arc::new(RwLock::new(node_with_proposer_key(&signing_key, 1_000_000)));
    let app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None, None);

    for path in ["/live", "/ready"] {
        let req = Request::builder()
            .uri(path)
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let s = std::str::from_utf8(&bytes).unwrap();
        assert!(s.contains("ok") || s.contains("ready"));
    }
}

#[tokio::test]
async fn rpc_jsonrpc_batch_returns_array() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let node = Arc::new(RwLock::new(node_with_proposer_key(&signing_key, 1_000_000)));
    let app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None, None);

    let batch = serde_json::json!([
        {"jsonrpc":"2.0","id":1,"method":"boing_chainHeight","params":[]},
        {"jsonrpc":"2.0","id":2,"method":"boing_clientVersion","params":[]},
    ]);
    let req = Request::builder()
        .method("POST")
        .uri("/")
        .header("content-type", "application/json")
        .body(Body::from(batch.to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let arr: Vec<serde_json::Value> = serde_json::from_slice(&bytes).expect("batch json");
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0].get("id"), Some(&serde_json::json!(1)));
    assert_eq!(arr[0].get("result"), Some(&serde_json::json!(0)));
    assert_eq!(arr[1].get("id"), Some(&serde_json::json!(2)));
    assert!(arr[1]
        .get("result")
        .and_then(|x| x.as_str())
        .is_some_and(|s| s.starts_with("boing-node/")));
}

#[tokio::test]
async fn rpc_jsonrpc_batch_invalid_element_errors() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let node = Arc::new(RwLock::new(node_with_proposer_key(&signing_key, 1_000_000)));
    let app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None, None);

    let batch = serde_json::json!([
        1,
        {"jsonrpc":"2.0","id":2,"method":"boing_chainHeight","params":[]},
    ]);
    let req = Request::builder()
        .method("POST")
        .uri("/")
        .header("content-type", "application/json")
        .body(Body::from(batch.to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let arr: Vec<serde_json::Value> = serde_json::from_slice(&bytes).expect("batch json");
    assert_eq!(arr.len(), 2);
    assert_eq!(
        arr[0].pointer("/error/code"),
        Some(&serde_json::json!(-32600))
    );
    assert_eq!(arr[1].get("result"), Some(&serde_json::json!(0)));
}

#[tokio::test]
async fn rpc_jsonrpc_batch_notifications_only_returns_204() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let node = Arc::new(RwLock::new(node_with_proposer_key(&signing_key, 1_000_000)));
    let app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None, None);

    let batch = serde_json::json!([
        {"jsonrpc": "2.0", "method": "boing_chainHeight", "params": []},
    ]);
    let req = Request::builder()
        .method("POST")
        .uri("/")
        .header("content-type", "application/json")
        .body(Body::from(batch.to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn rpc_jsonrpc_notification_returns_204() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let node = Arc::new(RwLock::new(node_with_proposer_key(&signing_key, 1_000_000)));
    let app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None, None);

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "boing_chainHeight",
        "params": []
    });
    let req = Request::builder()
        .method("POST")
        .uri("/")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn rpc_client_version_and_supported_methods() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let node = Arc::new(RwLock::new(node_with_proposer_key(&signing_key, 1_000_000)));
    let mut app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None, None);

    let cv = rpc_call(&mut app, "boing_clientVersion", serde_json::json!([])).await;
    let ver = cv.get("result").and_then(|x| x.as_str()).expect("clientVersion");
    assert!(ver.starts_with("boing-node/"));

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
    assert!(names.contains(&"boing_getSyncState"));
    assert!(names.contains(&"boing_getNetworkInfo"));
    assert!(names.contains(&"boing_getLogs"));
    assert!(names.contains(&"boing_getTransactionReceipt"));
    assert!(names.contains(&"boing_clientVersion"));
    assert!(names.contains(&"boing_rpcSupportedMethods"));
    assert!(names.contains(&"boing_health"));
    assert!(names.contains(&"boing_getRpcMethodCatalog"));
    assert!(names.contains(&"boing_getRpcOpenApi"));
}

#[tokio::test]
async fn rpc_unknown_method_includes_data() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let node = Arc::new(RwLock::new(node_with_proposer_key(&signing_key, 1_000_000)));
    let mut app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None, None);

    let v = rpc_call(
        &mut app,
        "boing_methodDoesNotExist",
        serde_json::json!([]),
    )
    .await;
    let err = v.get("error").expect("error");
    assert_eq!(err.get("code"), Some(&serde_json::json!(-32601)));
    let data = err.get("data").expect("data");
    assert_eq!(
        data.get("method").and_then(|x| x.as_str()),
        Some("boing_methodDoesNotExist")
    );
}

#[tokio::test]
async fn rpc_catalog_and_openapi_embedded() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let node = Arc::new(RwLock::new(node_with_proposer_key(&signing_key, 1_000_000)));
    let mut app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None, None);

    let cat = rpc_call(
        &mut app,
        "boing_getRpcMethodCatalog",
        serde_json::json!([]),
    )
    .await;
    let c = cat.get("result").expect("result").as_object().unwrap();
    let m = c.get("methods").and_then(|x| x.as_array()).expect("methods");
    assert!(m.len() >= 10, "embedded catalog should list many methods");

    let oa = rpc_call(&mut app, "boing_getRpcOpenApi", serde_json::json!([])).await;
    let root = oa.get("result").expect("result").as_object().unwrap();
    assert_eq!(root.get("openapi").and_then(|x| x.as_str()), Some("3.1.0"));
    let paths = root.get("paths").and_then(|x| x.as_object()).expect("paths");
    assert!(paths.contains_key("/"));
    let root_path = paths.get("/").and_then(|x| x.as_object()).expect("paths./");
    assert!(root_path.contains_key("get"));
    assert!(root_path.contains_key("head"));
    assert!(root_path.contains_key("post"));
    assert!(root_path.contains_key("options"));
    assert!(paths.contains_key("/ws"));
    assert!(paths.contains_key("/live"));
    assert!(paths.contains_key("/ready"));
    assert!(paths.contains_key("/openapi.json"));
    assert!(paths.contains_key("/.well-known/boing-rpc"));
}

#[tokio::test]
async fn rpc_health_shape() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let node = Arc::new(RwLock::new(node_with_proposer_key(&signing_key, 1_000_000)));
    let mut app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None, None);

    let v = rpc_call(&mut app, "boing_health", serde_json::json!([])).await;
    let h = v.get("result").expect("result").as_object().unwrap();
    assert_eq!(h.get("ok"), Some(&serde_json::json!(true)));
    assert!(h
        .get("client_version")
        .and_then(|x| x.as_str())
        .is_some_and(|s| s.starts_with("boing-node/")));
    assert_eq!(h.get("head_height"), Some(&serde_json::json!(0)));
    assert!(h.get("chain_id").is_some());
    assert!(h.get("chain_name").is_some());
    let surf = h.get("rpc_surface").and_then(|x| x.as_object()).expect("rpc_surface");
    assert_eq!(
        surf.get("http_rate_limit_requests_per_sec"),
        Some(&serde_json::json!(0))
    );
    assert!(surf.get("jsonrpc_batch_max").and_then(|x| x.as_u64()).is_some());
    assert_eq!(
        surf.get("websocket_max_connections"),
        Some(&serde_json::json!(0))
    );
    assert!(surf.get("ready_min_peers").is_some());
    assert!(surf
        .get("http_max_body_megabytes")
        .and_then(|x| x.as_u64())
        .is_some());
    assert_eq!(
        surf.get("get_logs_max_block_range"),
        Some(&serde_json::json!(128))
    );
    let m = h.get("rpc_metrics").and_then(|x| x.as_object()).expect("rpc_metrics");
    assert!(m.contains_key("rate_limited_total"));
    assert!(m.contains_key("method_not_found_total"));
}

#[tokio::test]
async fn http_live_ready_json_and_discovery_get() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let node = Arc::new(RwLock::new(node_with_proposer_key(&signing_key, 1_000_000)));
    let app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None, None);

    let live = Request::builder()
        .uri("/live.json")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(live).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v.get("ok"), Some(&serde_json::json!(true)));

    let wk = Request::builder()
        .uri("/.well-known/boing-rpc")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(wk).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let doc: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(doc.get("schema_version"), Some(&serde_json::json!(1)));
    assert_eq!(
        doc.get("openapi_url").and_then(|x| x.as_str()),
        Some("/openapi.json")
    );

    let oa = Request::builder()
        .uri("/openapi.json")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(oa).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let open: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(open.get("openapi").and_then(|x| x.as_str()), Some("3.1.0"));
}

#[tokio::test]
async fn http_head_root_and_live() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let node = Arc::new(RwLock::new(node_with_proposer_key(&signing_key, 1_000_000)));
    let app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None, None);

    let req = Request::builder()
        .method("HEAD")
        .uri("/")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);
    let allow = res.headers().get("allow").and_then(|v| v.to_str().ok());
    assert!(allow.is_some_and(|s| s.contains("HEAD")));

    let req = Request::builder()
        .method("HEAD")
        .uri("/live")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
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
    let mut app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None, None);

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
    v.extend(std::iter::repeat_n(0u8, 31));
    v.push(topic_tail);
    v.push(0x7f);
    v.extend(std::iter::repeat_n(0u8, 32));
    v.push(0x7f);
    v.extend(std::iter::repeat_n(0u8, 32));
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
    let mut app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None, None);

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

#[tokio::test]
async fn rpc_receipt_init_deploy_includes_logs() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let proposer = AccountId(signing_key.verifying_key().to_bytes());
    let node = Arc::new(RwLock::new(node_with_proposer_key(&signing_key, 1_000_000)));
    let mut app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None, None);

    let mut bytecode = vec![CONTRACT_DEPLOY_INIT_CODE_MARKER];
    bytecode.extend([
        0x60, 0x00, 0x60, 0x00, 0xa0, // LOG0 empty
        0x60, 0x00, 0x60, 0x00, 0x52, // MSTORE
        0x60, 0x01, 0x60, 0x00, 0xf3, // RETURN 1 byte runtime (STOP)
    ]);

    let deploy_tx = Transaction {
        nonce: 0,
        sender: proposer,
        payload: TransactionPayload::ContractDeploy {
            bytecode,
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
        n.produce_block_if_ready().expect("block with init deploy");
    }

    let rec_v = rpc_call(
        &mut app,
        "boing_getTransactionReceipt",
        serde_json::json!([format!("0x{}", hex::encode(deploy_id.0))]),
    )
    .await;
    let rec = rec_v.get("result").expect("result");
    assert_eq!(rec.get("success"), Some(&serde_json::json!(true)));
    let logs = rec.get("logs").and_then(|x| x.as_array()).expect("logs array");
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].get("topics"), Some(&serde_json::json!([])));
    assert_eq!(logs[0].get("data"), Some(&serde_json::json!("0x")));

    let contract = nonce_derived_contract_address(&proposer, 0);
    let addr_hex = format!("0x{}", hex::encode(contract.0));
    let height = rec.get("block_height").and_then(|h| h.as_u64()).expect("height");
    let gl = rpc_call(
        &mut app,
        "boing_getLogs",
        serde_json::json!([{
            "fromBlock": height,
            "toBlock": height,
            "address": addr_hex,
        }]),
    )
    .await;
    let rows = gl.get("result").and_then(|x| x.as_array()).expect("log rows");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("address"), Some(&serde_json::json!(addr_hex)));
}
