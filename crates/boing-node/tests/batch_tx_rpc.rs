//! Native atomic batch tx: mempool admission rules + JSON-RPC end-to-end (submit → block → receipt).

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use boing_node::mempool::{Mempool, MempoolError};
use boing_node::rpc::rpc_router;
use boing_node::security::RateLimitConfig;
use boing_primitives::{
    AccessList, Account, AccountId, AccountState, BatchCall, BatchValidationError,
    SignedTransaction, Transaction, TransactionPayload, MAX_BATCH_CALLDATA_BYTES, MAX_BATCH_CALLS,
};
use boing_state::StateStore;
use ed25519_dalek::SigningKey;
use http_body_util::BodyExt;
use rand::rngs::OsRng;
use tokio::sync::RwLock;
use tower::ServiceExt;

fn acct(b: u8) -> AccountId {
    AccountId([b; 32])
}

fn batch_tx(key: &SigningKey, nonce: u64, calls: Vec<BatchCall>) -> SignedTransaction {
    let sender = AccountId(key.verifying_key().to_bytes());
    let mut tx = Transaction {
        nonce,
        sender,
        payload: TransactionPayload::Batch { calls },
        access_list: AccessList::default(),
    };
    tx.access_list = tx.suggested_parallel_access_list();
    SignedTransaction::new(tx, key)
}

fn xfer(to: u8, amount: u128) -> BatchCall {
    BatchCall::Transfer { to: acct(to), amount }
}

// ---------------------------------------------------------------- mempool admission

#[test]
fn mempool_rejects_malformed_batches() {
    let pool = Mempool::new();
    let key = SigningKey::generate(&mut OsRng);

    let empty = batch_tx(&key, 0, vec![]);
    assert!(matches!(
        pool.insert(empty),
        Err(MempoolError::InvalidBatch(BatchValidationError::Empty))
    ));

    let too_many = batch_tx(&key, 0, vec![xfer(2, 1); MAX_BATCH_CALLS + 1]);
    assert!(matches!(
        pool.insert(too_many),
        Err(MempoolError::InvalidBatch(BatchValidationError::TooManyCalls { got: 17, max: 16 }))
    ));

    let big = batch_tx(
        &key,
        0,
        vec![BatchCall::ContractCall {
            contract: acct(3),
            calldata: vec![0u8; MAX_BATCH_CALLDATA_BYTES + 1],
        }],
    );
    assert!(matches!(
        pool.insert(big),
        Err(MempoolError::InvalidBatch(BatchValidationError::CalldataTooLarge { .. }))
    ));

    let overflow = batch_tx(&key, 0, vec![xfer(2, u128::MAX), xfer(3, 1)]);
    assert!(matches!(
        pool.insert(overflow),
        Err(MempoolError::InvalidBatch(BatchValidationError::AmountOverflow))
    ));

    assert_eq!(pool.len(), 0, "nothing malformed was admitted");

    // Exactly at the limit is fine.
    let max = batch_tx(&key, 0, vec![xfer(2, 1); MAX_BATCH_CALLS]);
    pool.insert(max).unwrap();
    assert_eq!(pool.len(), 1);
}

#[test]
fn disallowed_step_kinds_cannot_be_encoded() {
    // Only Transfer (0) and ContractCall (1) exist as BatchCall variants: a deploy / bond / vote
    // step (any other discriminant) fails bincode decoding, so the RPC rejects it before admission.
    let key = SigningKey::generate(&mut OsRng);
    let ok = batch_tx(&key, 0, vec![xfer(2, 1)]);
    let mut bytes = bincode::serialize(&ok).unwrap();
    // layout: nonce(8) sender(32) payload-variant(4) count(8) step-variant(4) ...
    let step_disc = 8 + 32 + 4 + 8;
    assert_eq!(&bytes[step_disc..step_disc + 4], &0u32.to_le_bytes());
    bytes[step_disc..step_disc + 4].copy_from_slice(&2u32.to_le_bytes());
    assert!(bincode::deserialize::<SignedTransaction>(&bytes).is_err());
}

// ---------------------------------------------------------------- RPC e2e

fn node_with_key(signing_key: &SigningKey, balance: u128) -> boing_node::node::BoingNode {
    let proposer = AccountId(signing_key.verifying_key().to_bytes());
    let genesis = boing_node::chain::ChainState::genesis(proposer);
    let chain = boing_node::chain::ChainState::from_genesis(genesis.clone());
    let mut consensus = boing_consensus::ConsensusEngine::single_validator(proposer);
    let _ = consensus.propose_and_commit(genesis);
    let mut state = StateStore::new();
    state.insert(Account {
        id: proposer,
        state: AccountState {
            balance: balance * boing_primitives::BASE_UNITS_PER_BOING,
            ..Default::default()
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
        mempool: Mempool::new(),
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
        liveness_miss_streak: HashMap::new(),
        leader_wait_started: None,
        open_validator_params: None,
        anchor_fallback_timeout: boing_node::node::DEFAULT_ANCHOR_FALLBACK_TIMEOUT,
        anchor_fallback_wait: None,
        last_local_vote_round: None,
        media_blob_store: None,
        bls_dual_key_registry: boing_primitives::BlsDualKeyRegistry::new(),
    }
}

async fn rpc(app: &axum::Router, method: &str, params: serde_json::Value) -> serde_json::Value {
    let body = serde_json::json!({"jsonrpc":"2.0","id":1,"method":method,"params":params});
    let req = Request::builder()
        .method("POST")
        .uri("/")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

fn hex_tx(s: &SignedTransaction) -> String {
    format!("0x{}", hex::encode(bincode::serialize(s).unwrap()))
}

#[tokio::test]
async fn rpc_batch_success_then_failing_batch_reverts_and_consumes_nonce() {
    let key = SigningKey::generate(&mut OsRng);
    let sender = AccountId(key.verifying_key().to_bytes());
    let node = Arc::new(RwLock::new(node_with_key(&key, 1_000_000)));
    let app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None, None);

    // Capability flag that wallets use to decide batch vs sequential.
    let h = rpc(&app, "boing_health", serde_json::json!([])).await;
    let surface = &h["result"]["rpc_surface"];
    assert_eq!(surface["tx_payload_batch"], serde_json::json!(true), "{h:?}");
    assert_eq!(surface["tx_batch_max_calls"], serde_json::json!(MAX_BATCH_CALLS));

    // 1) Valid batch: three transfers to two recipients.
    let ok = batch_tx(&key, 0, vec![xfer(0xb0, 100), xfer(0xc0, 250), xfer(0xb0, 5)]);
    let ok_id = ok.tx.id();
    let v = rpc(&app, "boing_submitTransaction", serde_json::json!([hex_tx(&ok)])).await;
    assert!(v.get("error").is_none(), "{v:?}");
    // Submit returns the real tx id (not the legacy "ok" ack).
    let want = format!("0x{}", hex::encode(ok_id.0));
    assert_eq!(v["result"]["tx_hash"], serde_json::json!(want), "{v:?}");
    assert_eq!(v["result"]["tx_id"], serde_json::json!(want), "{v:?}");
    let dup = rpc(&app, "boing_submitTransaction", serde_json::json!([hex_tx(&ok)])).await;
    assert_eq!(dup["error"]["data"]["tx_hash"], serde_json::json!(want), "{dup:?}");
    node.write().await.produce_block_if_ready().expect("block with batch");

    let r = rpc(
        &app,
        "boing_getTransactionReceipt",
        serde_json::json!([format!("0x{}", hex::encode(ok_id.0))]),
    )
    .await;
    let rec = &r["result"];
    assert_eq!(rec["success"], serde_json::json!(true), "{r:?}");
    let after_ok = {
        let n = node.read().await;
        assert_eq!(n.state.get(&acct(0xb0)).unwrap().balance, 105);
        assert_eq!(n.state.get(&acct(0xc0)).unwrap().balance, 250);
        let s = n.state.get(&sender).unwrap();
        assert_eq!(s.nonce, 1, "one nonce for the whole batch");
        s.balance
    };

    // 2) Failing batch: first step is fine, second calls a missing contract → all reverted.
    let bad = batch_tx(
        &key,
        1,
        vec![
            xfer(0xb0, 1_000),
            BatchCall::ContractCall {
                contract: acct(0xdd),
                calldata: vec![],
            },
        ],
    );
    let bad_id = bad.tx.id();
    let v = rpc(&app, "boing_submitTransaction", serde_json::json!([hex_tx(&bad)])).await;
    assert!(v.get("error").is_none(), "failing batch is still admitted: {v:?}");
    node.write().await.produce_block_if_ready().expect("block with failing batch");

    let r = rpc(
        &app,
        "boing_getTransactionReceipt",
        serde_json::json!([format!("0x{}", hex::encode(bad_id.0))]),
    )
    .await;
    let rec = &r["result"];
    assert_eq!(rec["success"], serde_json::json!(false), "{r:?}");
    let err = rec["error"].as_str().unwrap_or_default();
    assert!(err.contains("step 1"), "{err}");
    let n = node.read().await;
    assert_eq!(n.state.get(&acct(0xb0)).unwrap().balance, 105, "step 0 was reverted");
    let s = n.state.get(&sender).unwrap();
    assert_eq!(s.nonce, 2, "failed batch consumes the nonce");
    // Sender is the proposer (earns block rewards), so only check the 1_000 did not leave it.
    assert!(s.balance + 1_000 > after_ok, "the 1_000 transfer did not leave the sender");
}

#[tokio::test]
async fn rpc_account_transactions_indexes_sender_recipients_and_pages() {
    let key = SigningKey::generate(&mut OsRng);
    let sender = AccountId(key.verifying_key().to_bytes());
    let node = Arc::new(RwLock::new(node_with_key(&key, 1_000_000)));
    let app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None, None);

    // Block 1: batch paying 0xb0 + 0xc0. Block 2: plain transfer to 0xb0. Block 3: batch to 0xc0.
    let t0 = batch_tx(&key, 0, vec![xfer(0xb0, 10), xfer(0xc0, 20)]);
    rpc(&app, "boing_submitTransaction", serde_json::json!([hex_tx(&t0)])).await;
    node.write().await.produce_block_if_ready().unwrap();
    let mut plain = Transaction {
        nonce: 1,
        sender,
        payload: TransactionPayload::Transfer { to: acct(0xb0), amount: 3 },
        access_list: AccessList::default(),
    };
    plain.access_list = plain.suggested_parallel_access_list();
    let t1 = SignedTransaction::new(plain, &key);
    rpc(&app, "boing_submitTransaction", serde_json::json!([hex_tx(&t1)])).await;
    node.write().await.produce_block_if_ready().unwrap();

    let hx = |a: AccountId| format!("0x{}", hex::encode(a.0));
    let v = rpc(&app, "boing_getAccountTransactions", serde_json::json!([hx(acct(0xb0))])).await;
    let txs = v["result"]["transactions"].as_array().unwrap().clone();
    assert_eq!(txs.len(), 2, "{v:?}");
    assert_eq!(txs[0]["payload"]["kind"], "transfer", "newest first");
    assert_eq!(txs[0]["direction"], "in");
    assert_eq!(txs[1]["payload"]["kind"], "batch");
    assert_eq!(txs[1]["tx_id"], serde_json::json!(hx(AccountId(t0.tx.id().0))));
    assert!(v["result"]["next_cursor"].is_null());

    // Incremental catch-up after another block.
    let t2 = batch_tx(&key, 2, vec![xfer(0xc0, 1)]);
    rpc(&app, "boing_submitTransaction", serde_json::json!([hex_tx(&t2)])).await;
    node.write().await.produce_block_if_ready().unwrap();

    // Sender sees all three; page size 2 then the rest via cursor.
    let p1 = rpc(&app, "boing_getAccountTransactions", serde_json::json!([hx(sender), {"limit": 2}])).await;
    let a = p1["result"]["transactions"].as_array().unwrap().clone();
    assert_eq!(a.len(), 2, "{p1:?}");
    assert_eq!(a[0]["direction"], "out");
    assert_eq!(a[0]["success"], true);
    let cur = p1["result"]["next_cursor"].as_str().unwrap().to_string();
    let p2 = rpc(&app, "boing_getAccountTransactions", serde_json::json!([hx(sender), {"limit": 2, "cursor": cur}])).await;
    let b = p2["result"]["transactions"].as_array().unwrap().clone();
    assert_eq!(b.len(), 1, "{p2:?}");
    assert!(p2["result"]["next_cursor"].is_null());
    assert_eq!(b[0]["tx_id"], txs[1]["tx_id"]);

    let bad = rpc(&app, "boing_getAccountTransactions", serde_json::json!([hx(sender), {"cursor": "nope"}])).await;
    assert_eq!(bad["error"]["code"], -32602);
}