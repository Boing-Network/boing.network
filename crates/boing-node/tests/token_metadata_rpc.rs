//! `boing_getTokenMetadata` / `boing_listTokens` end to end: real deploy + `mint_first` transactions through
//! the RPC router and block producer, then metadata, decimals, supply and pagination are read back.
//! No DEX factory is configured: these methods must work without one.

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use boing_execution::{
    encode_mint_first_calldata, reference_fungible_template_bytecode,
    reference_fungible_template_v2_bytecode,
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
            balance: balance * boing_primitives::BASE_UNITS_PER_BOING,
            nonce: 0,
            stake: 0,
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
        liveness_miss_streak: HashMap::new(),
        leader_wait_started: None,
        open_validator_params: None,
        anchor_fallback_timeout: boing_node::node::DEFAULT_ANCHOR_FALLBACK_TIMEOUT,
        anchor_fallback_wait: None,
        last_local_vote_round: None,
        pending_since: None,
        proposal_timeout: boing_node::node::DEFAULT_PROPOSAL_TIMEOUT,
        media_blob_store: None,
        bls_dual_key_registry: boing_primitives::BlsDualKeyRegistry::new(),
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

async fn submit_and_produce(
    app: &mut axum::Router,
    node: &Arc<RwLock<boing_node::node::BoingNode>>,
    key: &SigningKey,
    tx: Transaction,
) {
    let signed = SignedTransaction::new(tx, key);
    let hex_tx = format!("0x{}", hex::encode(bincode::serialize(&signed).unwrap()));
    let v = rpc_call(app, "boing_submitTransaction", serde_json::json!([hex_tx])).await;
    assert!(v.get("error").is_none(), "{v:?}");
    let mut n = node.write().await;
    n.produce_block_if_ready().expect("block");
}

#[tokio::test]
async fn token_metadata_covers_tokens_outside_any_dex_pool() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let proposer = AccountId(signing_key.verifying_key().to_bytes());
    let node = Arc::new(RwLock::new(node_with_proposer_key(
        &signing_key,
        50_000_000,
    )));
    let mut app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None, None);

    // Token A: v2 template deployed with name + symbol.
    submit_and_produce(
        &mut app,
        &node,
        &signing_key,
        Transaction {
            nonce: 0,
            sender: proposer,
            payload: TransactionPayload::ContractDeployWithPurposeAndMetadata {
                bytecode: reference_fungible_template_v2_bytecode(),
                purpose_category: "token".to_string(),
                description_hash: None,
                asset_name: Some("Meta Check".to_string()),
                asset_symbol: Some("MCHK".to_string()),
                create2_salt: None,
            },
            access_list: AccessList::default(),
        },
    )
    .await;
    // Token B: v1 template with no metadata at all.
    submit_and_produce(
        &mut app,
        &node,
        &signing_key,
        Transaction {
            nonce: 1,
            sender: proposer,
            payload: TransactionPayload::ContractDeployWithPurpose {
                bytecode: reference_fungible_template_bytecode(),
                purpose_category: "token".to_string(),
                description_hash: None,
                create2_salt: None,
            },
            access_list: AccessList::default(),
        },
    )
    .await;
    let token_a = nonce_derived_contract_address(&proposer, 0);
    let token_b = nonce_derived_contract_address(&proposer, 1);

    // One-time mint on token A.
    let al = AccessList::new(vec![proposer, token_a], vec![proposer, token_a]);
    submit_and_produce(
        &mut app,
        &node,
        &signing_key,
        Transaction {
            nonce: 2,
            sender: proposer,
            payload: TransactionPayload::ContractCall {
                contract: token_a,
                calldata: encode_mint_first_calldata(&proposer, 1_000_000),
            },
            access_list: al,
        },
    )
    .await;

    // Single lookup, three accepted param shapes.
    for params in [
        serde_json::json!([hex_account(&token_a)]),
        serde_json::json!([{ "id": hex_account(&token_a) }]),
        serde_json::json!({ "id": hex_account(&token_a) }),
    ] {
        let r = rpc_call(&mut app, "boing_getTokenMetadata", params).await;
        assert!(r.get("error").is_none(), "{r:?}");
        let t = r.get("result").expect("result");
        assert_eq!(t["id"], hex_account(&token_a));
        assert_eq!(t["name"], "Meta Check");
        assert_eq!(t["symbol"], "MCHK");
        assert_eq!(t["decimals"], 0);
        assert_eq!(t["decimalsSource"], "template");
        assert_eq!(t["standard"], "reference-fungible-v2");
        assert_eq!(t["metadataSource"], "deploy");
        assert_eq!(t["totalSupply"], "1000000");
        assert_eq!(t["totalSupplySource"], "mint_first");
        assert_eq!(t["deployHeight"], 1);
        assert_eq!(t["deployer"], hex_account(&proposer));
    }

    // Token B has no name/symbol anywhere on chain: nulls, not an abbreviation; supply unknown (never minted).
    let rb = rpc_call(
        &mut app,
        "boing_getTokenMetadata",
        serde_json::json!([hex_account(&token_b)]),
    )
    .await;
    let b = rb.get("result").expect("result");
    assert!(b["name"].is_null() && b["symbol"].is_null() && b["metadataSource"].is_null());
    assert_eq!(b["decimals"], 0);
    assert_eq!(b["standard"], "reference-fungible-v1");
    assert!(b["totalSupply"].is_null());

    // Not a contract: null result, not an error.
    let rn = rpc_call(
        &mut app,
        "boing_getTokenMetadata",
        serde_json::json!([hex_account(&AccountId([0xCD; 32]))]),
    )
    .await;
    assert!(
        rn.get("error").is_none() && rn["result"].is_null(),
        "{rn:?}"
    );

    // Malformed id is -32602.
    let re = rpc_call(
        &mut app,
        "boing_getTokenMetadata",
        serde_json::json!(["0x12"]),
    )
    .await;
    assert_eq!(re["error"]["code"], -32602);

    // Directory + pagination (no DEX factory configured).
    let all = rpc_call(
        &mut app,
        "boing_listTokens",
        serde_json::json!([{ "limit": 50 }]),
    )
    .await;
    assert!(all.get("error").is_none(), "{all:?}");
    let r = &all["result"];
    assert_eq!(r["total"], 2);
    assert_eq!(r["tokens"].as_array().unwrap().len(), 2);
    assert!(r["nextCursor"].is_null());
    let first_page = rpc_call(
        &mut app,
        "boing_listTokens",
        serde_json::json!([{ "limit": 1 }]),
    )
    .await;
    let cur = first_page["result"]["nextCursor"]
        .as_str()
        .expect("cursor")
        .to_string();
    let second = rpc_call(
        &mut app,
        "boing_listTokens",
        serde_json::json!([{ "limit": 1, "cursor": cur }]),
    )
    .await;
    assert_eq!(second["result"]["tokens"].as_array().unwrap().len(), 1);
    assert_ne!(
        first_page["result"]["tokens"][0]["id"],
        second["result"]["tokens"][0]["id"]
    );
    assert!(second["result"]["nextCursor"].is_null());

    // Advertised in discovery.
    let info = rpc_call(&mut app, "boing_getNetworkInfo", serde_json::json!([])).await;
    let methods = info["result"]["developer"]["token_metadata_methods"]
        .as_array()
        .expect("token_metadata_methods");
    assert!(methods.iter().any(|m| m == "boing_getTokenMetadata"));
    assert!(methods.iter().any(|m| m == "boing_listTokens"));
}
