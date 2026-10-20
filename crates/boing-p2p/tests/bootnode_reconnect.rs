//! Bootnode redial: a node whose bootnode is killed reconnects on its own once the bootnode is back
//! (new identity, same address, no restart of the follower), and block-sync works over the restored link.

use std::sync::Arc;
use std::time::Duration;

use boing_p2p::{Block, BlockProvider, BlockRequest, P2pEvent, P2pNode, P2pStatus};
use boing_primitives::{AccountId, BlockHeader, Hash};

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

async fn wait_status(
    node: &P2pNode,
    what: &str,
    timeout: Duration,
    ok: impl Fn(&P2pStatus) -> bool,
) {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let s = node.status().await;
        if ok(&s) {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out after {timeout:?} waiting for {what}; last status {s:?}"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn block_at(height: u64) -> Block {
    Block {
        header: BlockHeader {
            parent_hash: Hash([1u8; 32]),
            height,
            timestamp: 1_700_000_000,
            proposer: AccountId([2u8; 32]),
            tx_root: Hash([0u8; 32]),
            receipts_root: Hash([0u8; 32]),
            state_root: Hash([0u8; 32]),
            blobs_root: None,
        },
        transactions: vec![],
        signatures: vec![],
        priority_tips: vec![],
    }
}

struct OneBlock(Block);

impl BlockProvider for OneBlock {
    fn get_block_by_hash(&self, _hash: &Hash) -> Option<Block> {
        None
    }
    fn get_block_by_height(&self, height: u64) -> Option<Block> {
        (height == self.0.header.height).then(|| self.0.clone())
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn follower_redials_bootnode_after_it_is_killed_and_restored() {
    let port = free_port();
    let listen = format!("/ip4/127.0.0.1/tcp/{port}");

    let (bootnode, _boot_rx) = P2pNode::new(&listen, None, 0).expect("bootnode");
    let (follower, mut follower_rx) =
        P2pNode::new("/ip4/127.0.0.1/tcp/0", None, 0).expect("follower");
    // DNS form: resolved again on every (re)dial.
    let accepted = follower
        .set_bootnodes(&[
            format!("/dns4/localhost/tcp/{port}"),
            "not-a-multiaddr".to_string(),
        ])
        .unwrap();
    assert_eq!(accepted, 1, "invalid entries are skipped");

    wait_status(
        &follower,
        "initial bootnode connection",
        Duration::from_secs(20),
        |s| s.connected_peers == 1 && s.bootnodes == 1 && s.bootnodes_connected == 1,
    )
    .await;

    // Kill the bootnode (its swarm task ends when the last handle is dropped).
    drop(bootnode);
    wait_status(
        &follower,
        "drop to zero peers",
        Duration::from_secs(20),
        |s| s.connected_peers == 0 && s.bootnodes_connected == 0,
    )
    .await;

    // Stay down long enough for a few failed redials (backoff grows).
    tokio::time::sleep(Duration::from_secs(4)).await;
    assert_eq!(follower.status().await.connected_peers, 0);

    // Bring it back on the same address with a fresh identity and a block to serve.
    let (restored, _restored_rx) =
        P2pNode::new(&listen, Some(Arc::new(OneBlock(block_at(7)))), 0).expect("restored bootnode");
    wait_status(
        &follower,
        "automatic reconnection",
        Duration::from_secs(40),
        |s| s.connected_peers == 1 && s.bootnodes_connected == 1,
    )
    .await;

    // Catch-up path over the restored link: request a block by height.
    let peer = follower.connected_peers().await[0];
    follower
        .request_block(peer, BlockRequest::ByHeight(7))
        .unwrap();
    let fetched = tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            match follower_rx.recv().await {
                Some(P2pEvent::BlockFetched(b)) => return b,
                Some(_) => continue,
                None => panic!("event channel closed"),
            }
        }
    })
    .await
    .expect("block fetched over restored connection");
    assert_eq!(fetched.header.height, 7);
    drop(restored);
}

#[tokio::test]
async fn inert_node_reports_zero_status() {
    let n = P2pNode::inert();
    assert_eq!(n.status().await, P2pStatus::default());
    assert_eq!(n.set_bootnodes(&["/ip4/127.0.0.1/tcp/1"]).unwrap(), 1);
}
