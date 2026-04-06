//! Native DEX pair directory: deploy factory + pool → `register_pair` → read `pairs_count` / `get_pair_at` via RPC.
//!
//! Asserts **`Log3`** on register ([`NATIVE_DEX_FACTORY_TOPIC_REGISTER`]) and contract **`return_data`** on view-style calls.

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use boing_execution::{
    encode_get_pair_at_calldata, encode_pairs_count_calldata, encode_register_pair_calldata,
    native_dex_factory_bytecode, NATIVE_DEX_FACTORY_TOPIC_REGISTER,
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

fn tx_id_hex(tx: &Transaction) -> String {
    format!("0x{}", hex::encode(tx.id().0))
}

fn assert_factory_register_log3(
    rpc: &serde_json::Value,
    token_a: &AccountId,
    token_b: &AccountId,
    pool: &AccountId,
) {
    let logs = rpc
        .get("result")
        .and_then(|r| r.get("logs"))
        .and_then(|l| l.as_array())
        .expect("transaction receipt logs");
    let t0 = format!("0x{}", hex::encode(NATIVE_DEX_FACTORY_TOPIC_REGISTER));
    let mut found = false;
    for log in logs {
        let topics = log.get("topics").and_then(|t| t.as_array()).expect("topics");
        if topics.len() != 3 {
            continue;
        }
        if topics[0].as_str().unwrap() != t0 {
            continue;
        }
        assert_eq!(topics[1].as_str().unwrap(), hex_account(token_a));
        assert_eq!(topics[2].as_str().unwrap(), hex_account(token_b));
        let data = log.get("data").and_then(|d| d.as_str()).expect("data");
        let raw = hex::decode(data.trim_start_matches("0x")).unwrap();
        assert_eq!(raw.len(), 32);
        assert_eq!(raw.as_slice(), pool.0.as_slice());
        found = true;
        break;
    }
    assert!(found, "expected Log3 for register_pair");
}

#[tokio::test]
async fn native_dex_factory_deploy_register_and_query_via_rpc() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let proposer = AccountId(signing_key.verifying_key().to_bytes());
    let node = Arc::new(RwLock::new(node_with_proposer_key(&signing_key, 50_000_000)));
    let mut app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None, None);

    let pool_bytecode = boing_execution::constant_product_pool_bytecode();
    let deploy_pool = Transaction {
        nonce: 0,
        sender: proposer,
        payload: TransactionPayload::ContractDeployWithPurpose {
            bytecode: pool_bytecode,
            purpose_category: "dapp".to_string(),
            description_hash: None,
            create2_salt: None,
        },
        access_list: AccessList::default(),
    };
    let signed_pool = SignedTransaction::new(deploy_pool, &signing_key);
    let hex_pool = format!(
        "0x{}",
        hex::encode(bincode::serialize(&signed_pool).unwrap())
    );
    let v0 = rpc_call(
        &mut app,
        "boing_submitTransaction",
        serde_json::json!([hex_pool]),
    )
    .await;
    assert!(v0.get("error").is_none(), "{v0:?}");
    {
        let mut n = node.write().await;
        n.produce_block_if_ready().expect("block with pool deploy");
    }

    let factory_bytecode = native_dex_factory_bytecode();
    let deploy_factory = Transaction {
        nonce: 1,
        sender: proposer,
        payload: TransactionPayload::ContractDeployWithPurpose {
            bytecode: factory_bytecode,
            purpose_category: "dapp".to_string(),
            description_hash: None,
            create2_salt: None,
        },
        access_list: AccessList::default(),
    };
    let signed_factory = SignedTransaction::new(deploy_factory, &signing_key);
    let hex_factory = format!(
        "0x{}",
        hex::encode(bincode::serialize(&signed_factory).unwrap())
    );
    let v1 = rpc_call(
        &mut app,
        "boing_submitTransaction",
        serde_json::json!([hex_factory]),
    )
    .await;
    assert!(v1.get("error").is_none(), "{v1:?}");
    {
        let mut n = node.write().await;
        n.produce_block_if_ready().expect("block with factory deploy");
    }

    let pool = nonce_derived_contract_address(&proposer, 0);
    let factory = nonce_derived_contract_address(&proposer, 1);
    let token_a = AccountId([0xAA; 32]);
    let token_b = AccountId([0xBB; 32]);

    let al = AccessList::new(vec![proposer, factory], vec![proposer, factory]);
    let reg_calldata = encode_register_pair_calldata(&token_a, &token_b, &pool);
    let reg_tx = Transaction {
        nonce: 2,
        sender: proposer,
        payload: TransactionPayload::ContractCall {
            contract: factory,
            calldata: reg_calldata,
        },
        access_list: al.clone(),
    };
    let signed_reg = SignedTransaction::new(reg_tx, &signing_key);
    let hex_reg = format!(
        "0x{}",
        hex::encode(bincode::serialize(&signed_reg).unwrap())
    );
    let v2 = rpc_call(
        &mut app,
        "boing_submitTransaction",
        serde_json::json!([hex_reg]),
    )
    .await;
    assert!(v2.get("error").is_none(), "{v2:?}");
    {
        let mut n = node.write().await;
        n.produce_block_if_ready().expect("block with register");
    }

    let r_reg = rpc_call(
        &mut app,
        "boing_getTransactionReceipt",
        serde_json::json!([tx_id_hex(&signed_reg.tx)]),
    )
    .await;
    assert_factory_register_log3(&r_reg, &token_a, &token_b, &pool);

    let cnt_calldata = encode_pairs_count_calldata();
    let cnt_tx = Transaction {
        nonce: 3,
        sender: proposer,
        payload: TransactionPayload::ContractCall {
            contract: factory,
            calldata: cnt_calldata,
        },
        access_list: al.clone(),
    };
    let signed_cnt = SignedTransaction::new(cnt_tx, &signing_key);
    let hex_cnt = format!(
        "0x{}",
        hex::encode(bincode::serialize(&signed_cnt).unwrap())
    );
    let v3 = rpc_call(
        &mut app,
        "boing_submitTransaction",
        serde_json::json!([hex_cnt]),
    )
    .await;
    assert!(v3.get("error").is_none(), "{v3:?}");
    {
        let mut n = node.write().await;
        n.produce_block_if_ready().expect("block with pairs_count");
    }
    let r_cnt = rpc_call(
        &mut app,
        "boing_getTransactionReceipt",
        serde_json::json!([tx_id_hex(&signed_cnt.tx)]),
    )
    .await;
    let rd = r_cnt
        .get("result")
        .and_then(|r| r.get("return_data"))
        .and_then(|x| x.as_str())
        .expect("return_data");
    let raw = hex::decode(rd.trim_start_matches("0x")).unwrap();
    assert_eq!(raw.len(), 32);
    assert_eq!(raw[31], 1);

    let get_calldata = encode_get_pair_at_calldata(0);
    let get_tx = Transaction {
        nonce: 4,
        sender: proposer,
        payload: TransactionPayload::ContractCall {
            contract: factory,
            calldata: get_calldata,
        },
        access_list: al,
    };
    let signed_get = SignedTransaction::new(get_tx, &signing_key);
    let hex_get = format!(
        "0x{}",
        hex::encode(bincode::serialize(&signed_get).unwrap())
    );
    let v4 = rpc_call(
        &mut app,
        "boing_submitTransaction",
        serde_json::json!([hex_get]),
    )
    .await;
    assert!(v4.get("error").is_none(), "{v4:?}");
    {
        let mut n = node.write().await;
        n.produce_block_if_ready().expect("block with get_pair_at");
    }
    let r_get = rpc_call(
        &mut app,
        "boing_getTransactionReceipt",
        serde_json::json!([tx_id_hex(&signed_get.tx)]),
    )
    .await;
    let rd2 = r_get
        .get("result")
        .and_then(|r| r.get("return_data"))
        .and_then(|x| x.as_str())
        .expect("return_data");
    let raw2 = hex::decode(rd2.trim_start_matches("0x")).unwrap();
    assert_eq!(raw2.len(), 96);
    assert_eq!(&raw2[0..32], token_a.0.as_slice());
    assert_eq!(&raw2[32..64], token_b.0.as_slice());
    assert_eq!(&raw2[64..96], pool.0.as_slice());
}
