//! Gossip hardening: frames larger than libp2p's 64 KiB default are delivered with the default
//! `GossipConfig`, with peer scoring + application-level validation reporting turned on.

use std::time::Duration;

use boing_p2p::{GossipConfig, P2pEvent, P2pNode};
use boing_primitives::{blob_id_of, MediaBlobGossip};

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

fn blob(len: usize, seed: u8) -> MediaBlobGossip {
    let data: Vec<u8> = (0..len).map(|i| (i as u8).wrapping_add(seed)).collect();
    MediaBlobGossip { blob_id: blob_id_of(&data), content_type: "application/octet-stream".into(), data }
}

async fn deliver(cfg: GossipConfig, payload: MediaBlobGossip) -> bool {
    let port = free_port();
    let (sub, mut sub_rx) =
        P2pNode::new_with_gossip(&format!("/ip4/127.0.0.1/tcp/{port}"), None, 0, cfg.clone()).expect("sub");
    let (publisher, _pub_rx) =
        P2pNode::new_with_gossip("/ip4/127.0.0.1/tcp/0", None, 0, cfg).expect("publisher");
    publisher.dial(&format!("/ip4/127.0.0.1/tcp/{port}")).unwrap();
    let _keep = sub;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(40);
    // The gossipsub mesh needs a few heartbeats after connect; keep re-publishing.
    while tokio::time::Instant::now() < deadline {
        let _ = publisher.broadcast_media_blob(&payload);
        if let Ok(Some(ev)) = tokio::time::timeout(Duration::from_millis(1500), sub_rx.recv()).await {
            if let P2pEvent::MediaBlobReceived(b) = ev {
                return b == payload;
            }
        }
    }
    false
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn large_frames_are_delivered_by_default_with_scoring_on() {
    assert!(deliver(GossipConfig::default(), blob(300 * 1024, 7)).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn small_frames_still_delivered_with_scoring_and_limits_disabled() {
    let cfg = GossipConfig { peer_scoring: false, peer_msgs_per_sec: 0, peer_bytes_per_sec: 0, ..GossipConfig::default() };
    assert!(deliver(cfg, blob(1024, 3)).await);
}
