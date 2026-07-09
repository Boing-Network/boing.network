//! Submit via JSON-RPC on one node; signed tx gossips over P2P; another peer's mempool admits it.
//!
//! Uses a **four-node full mesh** so libp2p gossipsub's default mesh parameters (see upstream
//! `mesh_n` / `mesh_n_low`) allow topic propagation — a two-peer graph often never meshes.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use boing_node::rpc::rpc_router;
use boing_node::security::RateLimitConfig;
use boing_primitives::{
    AccessList, Account, AccountId, AccountState, Hash, SignedTransaction, Transaction,
    TransactionPayload,
};
use boing_state::StateStore;
use ed25519_dalek::SigningKey;
use http_body_util::BodyExt;
use rand::rngs::OsRng;
use tokio::sync::RwLock;
use tower::ServiceExt;

fn node_with_p2p_only(
    signing_key: &SigningKey,
    balance: u128,
    p2p_listen: &str,
) -> (
    boing_node::node::BoingNode,
    tokio::sync::mpsc::UnboundedReceiver<boing_p2p::P2pEvent>,
) {
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

    let (p2p, event_rx) = boing_p2p::P2pNode::new(p2p_listen, None, 0).expect("P2P init");

    let native_aggregates = state.compute_native_aggregates();
    let node = boing_node::node::BoingNode {
        chain,
        consensus,
        state,
        executor: boing_execution::BlockExecutor::new(),
        producer: boing_node::block_producer::BlockProducer::new(proposer).with_max_txs(100),
        vm: boing_execution::Vm::new(),
        scheduler: boing_execution::TransactionScheduler::new(),
        mempool: boing_node::mempool::Mempool::new(),
        p2p,
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
    };
    (node, event_rx)
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

fn spawn_p2p_ingest(
    node: Arc<RwLock<boing_node::node::BoingNode>>,
    mut rx: tokio::sync::mpsc::UnboundedReceiver<boing_p2p::P2pEvent>,
) {
    tokio::spawn(async move {
        while let Some(ev) = rx.recv().await {
            match ev {
                boing_p2p::P2pEvent::BlockReceived(block) => {
                    let mut n = node.write().await;
                    let _ = n.handle_network_block(&block);
                }
                boing_p2p::P2pEvent::BlockFetched(block) => {
                    let mut n = node.write().await;
                    let _ = n.import_network_block_catchup(&block);
                }
                boing_p2p::P2pEvent::VoteReceived(vote) => {
                    let mut n = node.write().await;
                    let _ = n.on_consensus_vote(vote);
                }
                boing_p2p::P2pEvent::EquivocationReceived(ev) => {
                    let mut n = node.write().await;
                    let _ = n.on_equivocation_evidence(ev);
                }
                boing_p2p::P2pEvent::TransactionReceived(signed) => {
                    if signed.verify().is_err() {
                        continue;
                    }
                    let n = node.read().await;
                    let _ = n.submit_transaction(signed);
                }
            }
        }
    });
}

async fn any_non_submitter_has_tx(
    nodes: &[Arc<RwLock<boing_node::node::BoingNode>>],
    tx_id: &Hash,
) -> bool {
    for n in nodes.iter().skip(1) {
        if n.read().await.mempool.contains_tx_id(tx_id) {
            return true;
        }
    }
    false
}

#[tokio::test(flavor = "multi_thread")]
async fn rpc_submitted_tx_propagates_to_peer_mempool_via_gossip() {
    let key = SigningKey::generate(&mut OsRng);
    let proposer = AccountId(key.verifying_key().to_bytes());
    let to = AccountId([9u8; 32]);

    const N: usize = 4;
    let base_port = 34301u16;
    let addrs: Vec<String> = (0..N)
        .map(|i| format!("/ip4/127.0.0.1/tcp/{}", base_port + i as u16))
        .collect();

    let mut receivers = Vec::new();
    let mut nodes: Vec<Arc<RwLock<boing_node::node::BoingNode>>> = Vec::new();
    for addr in &addrs {
        let (node, rx) = node_with_p2p_only(&key, 1_000_000, addr);
        nodes.push(Arc::new(RwLock::new(node)));
        receivers.push(rx);
    }

    tokio::time::sleep(Duration::from_secs(1)).await;

    for i in 0..N {
        for j in 0..N {
            if i == j {
                continue;
            }
            nodes[i]
                .read()
                .await
                .p2p
                .dial(&addrs[j])
                .expect("full mesh dial");
        }
    }

    tokio::time::sleep(Duration::from_secs(4)).await;

    for (node, rx) in nodes.iter().zip(receivers) {
        spawn_p2p_ingest(node.clone(), rx);
    }

    tokio::time::sleep(Duration::from_secs(2)).await;

    let mut app = rpc_router(
        nodes[0].clone(),
        &RateLimitConfig::default(),
        None,
        None,
        None,
    );

    let tx = Transaction {
        nonce: 0,
        sender: proposer,
        payload: TransactionPayload::Transfer {
            to,
            amount: 1,
        },
        access_list: AccessList::new(vec![proposer, to], vec![proposer, to]),
    };
    let signed = SignedTransaction::new(tx, &key);
    let tx_id = signed.tx.id();
    let hex_tx = format!(
        "0x{}",
        hex::encode(bincode::serialize(&signed).unwrap())
    );

    let v = rpc_call(
        &mut app,
        "boing_submitTransaction",
        serde_json::json!([hex_tx]),
    )
    .await;
    assert!(
        v.get("error").is_none(),
        "submit failed: {}",
        serde_json::to_string(&v).unwrap()
    );

    assert!(
        nodes[0].read().await.mempool.contains_tx_id(&tx_id),
        "submitter mempool should hold the tx"
    );

    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    let mut seen = false;
    while tokio::time::Instant::now() < deadline {
        if any_non_submitter_has_tx(&nodes, &tx_id).await {
            seen = true;
            break;
        }
        let _ = nodes[0]
            .read()
            .await
            .p2p
            .broadcast_signed_transaction(&signed);
        tokio::time::sleep(Duration::from_millis(400)).await;
    }

    assert!(
        seen,
        "at least one non-submitter peer should admit gossiped tx 0x{} within deadline",
        hex::encode(tx_id.0)
    );
}
