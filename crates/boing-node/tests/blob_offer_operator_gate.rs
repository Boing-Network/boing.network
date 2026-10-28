//! `boing_putBlobSampleOffer` writes the node-local offer book, so it requires the operator token
//! (and is rejected when `BOING_OPERATOR_RPC_TOKEN` is unset).

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use boing_node::media_blob_store::MediaBlobStore;
use boing_node::node::BoingNode;
use boing_node::rpc::rpc_router;
use boing_node::security::RateLimitConfig;
use boing_primitives::AccountId;
use ed25519_dalek::SigningKey;
use http_body_util::BodyExt;
use tokio::sync::RwLock;
use tower::ServiceExt;

fn node_with_store(tag: &str) -> (Arc<RwLock<BoingNode>>, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("boing-offer-gate-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let sk = SigningKey::from_bytes(&[7u8; 32]);
    let id = AccountId(sk.verifying_key().to_bytes());
    let mut n = BoingNode::with_validators(vec![id], id, Some(sk));
    n.attach_media_blob_store(MediaBlobStore::open(&dir).unwrap());
    (Arc::new(RwLock::new(n)), dir)
}

fn put_offer(op_header: Option<&str>) -> Request<Body> {
    let body = serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "boing_putBlobSampleOffer",
        "params": [{
            "blobId": format!("0x{}", "11".repeat(32)),
            "chunkIndex": 0, "chunkCount": 4,
            "provider": format!("0x{}", "22".repeat(32)),
            "askFee": "1",
        }]
    });
    let mut b = Request::builder()
        .method("POST")
        .uri("/")
        .header("content-type", "application/json");
    if let Some(h) = op_header {
        b = b.header("x-boing-operator", h);
    }
    b.body(Body::from(body.to_string())).unwrap()
}

async fn json_of(res: axum::response::Response) -> serde_json::Value {
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn put_sample_offer_requires_operator_token_when_configured() {
    let (node, dir) = node_with_store("gated");
    let op: Arc<str> = "offer-gate-secret".into();
    let app = rpc_router(
        node,
        &RateLimitConfig::default(),
        None,
        Some(op.clone()),
        None,
    );

    let denied = json_of(app.clone().oneshot(put_offer(None)).await.unwrap()).await;
    assert_eq!(denied["error"]["code"], -32057, "{denied}");
    let wrong = json_of(app.clone().oneshot(put_offer(Some("nope"))).await.unwrap()).await;
    assert_eq!(wrong["error"]["code"], -32057, "{wrong}");

    let ok = json_of(app.oneshot(put_offer(Some(op.as_ref()))).await.unwrap()).await;
    assert_eq!(ok["result"]["ok"], true, "{ok}");
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn put_sample_offer_rejected_without_operator_token() {
    // Fail closed: with no `BOING_OPERATOR_RPC_TOKEN` configured the operator gate rejects every call.
    let (node, dir) = node_with_store("unset");
    let app = rpc_router(node, &RateLimitConfig::default(), None, None, None);
    let denied = json_of(app.clone().oneshot(put_offer(None)).await.unwrap()).await;
    assert_eq!(denied["error"]["code"], -32057, "{denied}");
    let guess = json_of(app.oneshot(put_offer(Some("guess"))).await.unwrap()).await;
    assert_eq!(guess["error"]["code"], -32057, "{guess}");
    let _ = std::fs::remove_dir_all(dir);
}
