//! Simultaneous mutual dials must still produce a full mesh.
//!
//! Regression for the flaky `four_validators_quorum_commit_via_votes` (boing-node): every node dialed
//! every other node at the same moment. With libp2p's default `PortUse::Reuse`, an outbound TCP socket
//! is bound to the dialer's *listen* port, so A->B and B->A share one 4-tuple. The OS then either
//! rejects one connect (`EADDRINUSE`, WSA 10048, also while an earlier connection on that tuple sits in
//! TIME_WAIT) or TCP simultaneous-open merges both SYNs into a single socket on which *both* ends run
//! the dialer side of the TLS/Noise upgrade (`InappropriateHandshakeMessage`). Pairs where both
//! directions lost the race had no link at all, leaving some nodes with zero peers.

use std::time::Duration;

use boing_p2p::P2pNode;

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

async fn full_mesh_round(n: usize, round: usize) {
    let addrs: Vec<String> = (0..n)
        .map(|_| format!("/ip4/127.0.0.1/tcp/{}", free_port()))
        .collect();
    let nodes: Vec<_> = addrs
        .iter()
        .map(|a| P2pNode::new(a, None, 0).expect("p2p node"))
        .collect();
    // Everyone dials everyone at once (both directions of every pair race).
    for (i, (node, _)) in nodes.iter().enumerate() {
        for (j, addr) in addrs.iter().enumerate() {
            if i != j {
                node.dial(addr).unwrap();
            }
        }
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let mut counts = Vec::with_capacity(n);
        for (node, _) in &nodes {
            counts.push(node.status().await.connected_peers);
        }
        if counts.iter().all(|&c| c == n - 1) {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "round {round}: mutual dial did not form a full mesh; connected peers per node = {counts:?}"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn simultaneous_mutual_dials_form_full_mesh() {
    for round in 0..5 {
        full_mesh_round(4, round).await;
    }
}
