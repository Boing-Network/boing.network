//! Permission-gated `GET /v1/media/blob?id=` raw download.

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use boing_node::media_blob_store::{BlobMetaRecord, MediaBlobStore};
use boing_node::node::BoingNode;
use boing_node::rpc::rpc_router;
use boing_node::security::RateLimitConfig;
use boing_primitives::{blob_id_of, Account, AccountId, AccountState, BlobRetention};
use ed25519_dalek::SigningKey;
use http_body_util::BodyExt;
use tokio::sync::RwLock;
use tower::ServiceExt;

fn key(b: u8) -> SigningKey {
    SigningKey::from_bytes(&[b; 32])
}

fn tmp_dir(name: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!(
        "boing-blob-dl-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn node_with_blob(data_dir: &std::path::Path, payload: &[u8]) -> (Arc<RwLock<BoingNode>>, [u8; 32]) {
    let sk = key(7);
    let id = AccountId(sk.verifying_key().to_bytes());
    let mut n = BoingNode::with_validators(vec![id], id, Some(sk));
    n.state.insert(Account {
        id,
        state: AccountState {
            balance: 1_000_000 * boing_primitives::BASE_UNITS_PER_BOING,
            nonce: 0,
            stake: 0,
            ..Default::default()
        },
    });
    let store = MediaBlobStore::open(data_dir).unwrap();
    let blob_id = blob_id_of(payload);
    store
        .put(
            BlobMetaRecord {
                blob_id,
                size: payload.len() as u32,
                content_type: "text/plain".into(),
                retention: BlobRetention::Forever,
                publisher: id.0,
                included_height: 1,
                tx_id: [9u8; 32],
                pin_until_height: None,
            },
            payload,
        )
        .unwrap();
    n.attach_media_blob_store(store);
    (Arc::new(RwLock::new(n)), blob_id)
}

#[tokio::test]
async fn media_blob_download_open_without_tokens() {
    let dir = tmp_dir("open");
    let payload = b"hello-blob-download";
    let (node, blob_id) = node_with_blob(&dir, payload);
    let app = rpc_router(node, &RateLimitConfig::default(), None, None, None);
    let id_hex = hex::encode(blob_id);
    let req = Request::builder()
        .uri(format!("/v1/media/blob?id=0x{id_hex}"))
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.headers().get("content-type").and_then(|v| v.to_str().ok()),
        Some("text/plain")
    );
    let body = res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], payload);
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn media_blob_download_requires_operator_when_set() {
    let dir = tmp_dir("auth");
    let payload = b"secret-bytes";
    let (node, blob_id) = node_with_blob(&dir, payload);
    let op: std::sync::Arc<str> = "test-operator-secret".into();
    let app = rpc_router(
        node,
        &RateLimitConfig::default(),
        None,
        Some(op.clone()),
        None,
    );
    let id_hex = hex::encode(blob_id);

    let denied = Request::builder()
        .uri(format!("/v1/media/blob?id=0x{id_hex}"))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(denied).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    let allowed = Request::builder()
        .uri(format!("/v1/media/blob?id=0x{id_hex}"))
        .header("x-boing-operator", op.as_ref())
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(allowed).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], payload);
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn media_blob_download_accepts_bearer() {
    let dir = tmp_dir("bearer");
    let payload = b"bearer-payload";
    let (node, blob_id) = node_with_blob(&dir, payload);
    let op: std::sync::Arc<str> = "bearer-op-token".into();
    let app = rpc_router(node, &RateLimitConfig::default(), None, Some(op.clone()), None);
    let id_hex = hex::encode(blob_id);
    let req = Request::builder()
        .uri(format!("/v1/media/blob?id={id_hex}"))
        .header("authorization", format!("Bearer {}", op.as_ref()))
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], payload);
    let _ = std::fs::remove_dir_all(dir);
}
