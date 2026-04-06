//! Native AMM: deploy CP pool → add liquidity → swap → remove over JSON-RPC (checklist **A4.3** node-side).
//!
//! Asserts **`boing_getTransactionReceipt`** `logs` (`Log2` topic0 + data words) and **`boing_getLogs`** with pool `address` + swap **topic0** filter.
//! Boing Express + browser origin is manual; this test is the same protocol path wallets use.

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use boing_execution::{
    constant_product_amount_out_after_fee, constant_product_pool_bytecode, decode_add_liquidity_return_lp_minted,
    encode_add_liquidity_calldata, encode_remove_liquidity_calldata, encode_swap_calldata, reserve_a_key,
    reserve_b_key, total_lp_supply_key, NATIVE_AMM_TOPIC_ADD_LIQUIDITY, NATIVE_AMM_TOPIC_REMOVE_LIQUIDITY,
    NATIVE_AMM_TOPIC_SWAP, NATIVE_CP_POOL_CREATE2_SALT_V1,
};
use boing_execution::reference_token::amount_word;
use boing_node::rpc::rpc_router;
use boing_node::security::RateLimitConfig;
use boing_primitives::{
    create2_contract_address, nonce_derived_contract_address, AccessList, Account, AccountId,
    AccountState, SignedTransaction, Transaction, TransactionPayload,
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

fn hex_key32(k: &[u8; 32]) -> String {
    format!("0x{}", hex::encode(k))
}

fn tx_id_hex(tx: &Transaction) -> String {
    format!("0x{}", hex::encode(tx.id().0))
}

fn receipt_logs(rpc: &serde_json::Value) -> &Vec<serde_json::Value> {
    rpc.get("result")
        .and_then(|r| r.get("logs"))
        .and_then(|l| l.as_array())
        .expect("transaction receipt logs")
}

fn assert_native_amm_log2(
    rpc: &serde_json::Value,
    topic0: &[u8; 32],
    caller: &AccountId,
    word0: u128,
    word1: u128,
    word2: u128,
) {
    let logs = receipt_logs(rpc);
    assert_eq!(logs.len(), 1, "expected exactly one execution log");
    let topics = logs[0]
        .get("topics")
        .and_then(|t| t.as_array())
        .expect("log topics");
    assert_eq!(topics.len(), 2);
    assert_eq!(topics[0].as_str().unwrap(), hex_key32(topic0));
    assert_eq!(topics[1].as_str().unwrap(), hex_account(caller));
    let data = logs[0]
        .get("data")
        .and_then(|d| d.as_str())
        .expect("log data");
    let raw = hex::decode(data.trim_start_matches("0x")).unwrap();
    assert_eq!(raw.len(), 96);
    assert_eq!(raw[16..32], amount_word(word0)[16..32]);
    assert_eq!(raw[48..64], amount_word(word1)[16..32]);
    assert_eq!(raw[80..96], amount_word(word2)[16..32]);
}

fn assert_receipt_add_liquidity_return_lp(rpc: &serde_json::Value, expected_lp_minted: u128) {
    let rd = rpc
        .get("result")
        .and_then(|r| r.get("return_data"))
        .and_then(|x| x.as_str())
        .expect("transaction receipt return_data");
    let bytes = hex::decode(rd.trim_start_matches("0x")).expect("return_data hex");
    assert_eq!(
        decode_add_liquidity_return_lp_minted(&bytes),
        Some(expected_lp_minted),
        "add_liquidity return_data should be 32-byte LP minted word"
    );
}

#[tokio::test]
async fn native_amm_deploy_add_liquidity_swap_via_rpc() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let proposer = AccountId(signing_key.verifying_key().to_bytes());
    let node = Arc::new(RwLock::new(node_with_proposer_key(&signing_key, 10_000_000)));
    let mut app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None, None);

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

    let r_add = rpc_call(
        &mut app,
        "boing_getTransactionReceipt",
        serde_json::json!([tx_id_hex(&signed_add.tx)]),
    )
    .await;
    assert_native_amm_log2(
        &r_add,
        &NATIVE_AMM_TOPIC_ADD_LIQUIDITY,
        &proposer,
        1_000,
        2_000,
        1_000,
    );
    assert_receipt_add_liquidity_return_lp(&r_add, 1_000);

    let dx: u128 = 100;
    let dy = u128::from(constant_product_amount_out_after_fee(1_000u64, 2_000u64, dx as u64));
    let swap_calldata = encode_swap_calldata(0, dx, dy);
    let swap_tx = Transaction {
        nonce: 2,
        sender: proposer,
        payload: TransactionPayload::ContractCall {
            contract,
            calldata: swap_calldata,
        },
        access_list: al.clone(),
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

    let r_swap = rpc_call(
        &mut app,
        "boing_getTransactionReceipt",
        serde_json::json!([tx_id_hex(&signed_swap.tx)]),
    )
    .await;
    assert_native_amm_log2(
        &r_swap,
        &NATIVE_AMM_TOPIC_SWAP,
        &proposer,
        0,
        dx,
        dy,
    );

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

    let da_out = ra_u / 1_000u128;
    let db_out = rb_u / 1_000u128;

    let remove_calldata = encode_remove_liquidity_calldata(1, 0, 0);
    let remove_tx = Transaction {
        nonce: 3,
        sender: proposer,
        payload: TransactionPayload::ContractCall {
            contract,
            calldata: remove_calldata,
        },
        access_list: al,
    };
    let signed_remove = SignedTransaction::new(remove_tx, &signing_key);
    let hex_remove = format!(
        "0x{}",
        hex::encode(bincode::serialize(&signed_remove).unwrap())
    );
    let v4 = rpc_call(
        &mut app,
        "boing_submitTransaction",
        serde_json::json!([hex_remove]),
    )
    .await;
    assert!(v4.get("error").is_none(), "{v4:?}");
    {
        let mut n = node.write().await;
        n.produce_block_if_ready().expect("block with remove_liquidity");
    }

    let r_rm = rpc_call(
        &mut app,
        "boing_getTransactionReceipt",
        serde_json::json!([tx_id_hex(&signed_remove.tx)]),
    )
    .await;
    assert_native_amm_log2(
        &r_rm,
        &NATIVE_AMM_TOPIC_REMOVE_LIQUIDITY,
        &proposer,
        1,
        da_out,
        db_out,
    );

    let ra2 = rpc_call(
        &mut app,
        "boing_getContractStorage",
        serde_json::json!([hex_account(&contract), hex_key32(&reserve_a_key())]),
    )
    .await;
    let ra2_v = ra2
        .get("result")
        .and_then(|r| r.get("value"))
        .and_then(|x| x.as_str())
        .expect("reserve A after remove");
    let rb2 = rpc_call(
        &mut app,
        "boing_getContractStorage",
        serde_json::json!([hex_account(&contract), hex_key32(&reserve_b_key())]),
    )
    .await;
    let rb2_v = rb2
        .get("result")
        .and_then(|r| r.get("value"))
        .and_then(|x| x.as_str())
        .expect("reserve B after remove");
    let ra2_word = hex::decode(ra2_v.trim_start_matches("0x")).unwrap();
    let rb2_word = hex::decode(rb2_v.trim_start_matches("0x")).unwrap();
    let ra2_u = u128::from_be_bytes(ra2_word[16..32].try_into().unwrap());
    let rb2_u = u128::from_be_bytes(rb2_word[16..32].try_into().unwrap());
    assert_eq!(ra2_u, ra_u - da_out);
    assert_eq!(rb2_u, rb_u - db_out);

    let t_st = rpc_call(
        &mut app,
        "boing_getContractStorage",
        serde_json::json!([hex_account(&contract), hex_key32(&total_lp_supply_key())]),
    )
    .await;
    let t_v = t_st
        .get("result")
        .and_then(|r| r.get("value"))
        .and_then(|x| x.as_str())
        .expect("total LP");
    let t_word = hex::decode(t_v.trim_start_matches("0x")).unwrap();
    let t_u = u128::from_be_bytes(t_word[16..32].try_into().unwrap());
    assert_eq!(t_u, 1_000u128 - 1);

    let h = rpc_call(&mut app, "boing_chainHeight", serde_json::json!([]))
        .await
        .get("result")
        .and_then(|x| x.as_u64())
        .expect("chain height");
    let logs_swap = rpc_call(
        &mut app,
        "boing_getLogs",
        serde_json::json!([{
            "fromBlock": 0,
            "toBlock": h,
            "address": hex_account(&contract),
            "topics": [hex_key32(&NATIVE_AMM_TOPIC_SWAP)],
        }]),
    )
    .await;
    let arr = logs_swap
        .get("result")
        .and_then(|r| r.as_array())
        .expect("getLogs result array");
    assert_eq!(arr.len(), 1);
    assert_eq!(
        arr[0].get("tx_id").and_then(|x| x.as_str()),
        Some(tx_id_hex(&signed_swap.tx).as_str())
    );
}

/// CREATE2 deploy with `NATIVE_CP_POOL_CREATE2_SALT_V1` → deterministic pool id; add liquidity + swap.
#[tokio::test]
async fn native_amm_create2_deploy_add_swap_via_rpc() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let proposer = AccountId(signing_key.verifying_key().to_bytes());
    let node = Arc::new(RwLock::new(node_with_proposer_key(&signing_key, 10_000_000)));
    let mut app = rpc_router(node.clone(), &RateLimitConfig::default(), None, None, None);

    let bytecode = constant_product_pool_bytecode();
    let salt = NATIVE_CP_POOL_CREATE2_SALT_V1;
    let pool = create2_contract_address(&proposer, &salt, &bytecode);

    let deploy_tx = Transaction {
        nonce: 0,
        sender: proposer,
        payload: TransactionPayload::ContractDeployWithPurpose {
            bytecode: bytecode.clone(),
            purpose_category: "dapp".to_string(),
            description_hash: None,
            create2_salt: Some(salt),
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
        n.produce_block_if_ready().expect("block with create2 deploy");
    }

    let al = AccessList::new(vec![proposer, pool], vec![proposer, pool]);
    let add_calldata = encode_add_liquidity_calldata(500, 800, 0);
    let add_tx = Transaction {
        nonce: 1,
        sender: proposer,
        payload: TransactionPayload::ContractCall {
            contract: pool,
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
    assert!(v2.get("error").is_none(), "{v:?}");
    {
        let mut n = node.write().await;
        n.produce_block_if_ready().expect("block with add_liquidity");
    }

    let r_add = rpc_call(
        &mut app,
        "boing_getTransactionReceipt",
        serde_json::json!([tx_id_hex(&signed_add.tx)]),
    )
    .await;
    assert_receipt_add_liquidity_return_lp(&r_add, 500);

    let dx: u128 = 50;
    let dy = u128::from(constant_product_amount_out_after_fee(500u64, 800u64, dx as u64));
    let swap_calldata = encode_swap_calldata(0, dx, dy);
    let swap_tx = Transaction {
        nonce: 2,
        sender: proposer,
        payload: TransactionPayload::ContractCall {
            contract: pool,
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
    assert!(v3.get("error").is_none(), "{v:?}");
    {
        let mut n = node.write().await;
        n.produce_block_if_ready().expect("block with swap");
    }

    let r_swap = rpc_call(
        &mut app,
        "boing_getTransactionReceipt",
        serde_json::json!([tx_id_hex(&signed_swap.tx)]),
    )
    .await;
    assert_native_amm_log2(
        &r_swap,
        &NATIVE_AMM_TOPIC_SWAP,
        &proposer,
        0,
        dx,
        dy,
    );
}
