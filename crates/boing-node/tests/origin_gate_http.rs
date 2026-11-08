//! Companion origin gate (`BOING_ORIGIN_GATE_TOKEN` / `X-Boing-Origin-Gate`).

use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use boing_node::node::BoingNode;
use boing_node::rpc::rpc_router;
use boing_node::security::RateLimitConfig;
use http_body_util::BodyExt;
use tokio::sync::RwLock;
use tower::ServiceExt;

/// Process-wide lock: `BOING_ORIGIN_GATE_TOKEN` is read at router build time from the environment.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn with_gate_env<T>(token: Option<&str>, f: impl FnOnce() -> T) -> T {
    const KEY: &str = "BOING_ORIGIN_GATE_TOKEN";
    let _guard = ENV_LOCK.lock().expect("origin gate env lock");
    let prev = std::env::var(KEY).ok();
    match token {
        Some(t) => std::env::set_var(KEY, t),
        None => std::env::remove_var(KEY),
    }
    let out = f();
    match prev {
        Some(v) => std::env::set_var(KEY, v),
        None => std::env::remove_var(KEY),
    }
    out
}

async fn live_status(token_env: Option<&str>, gate_header: Option<&str>) -> StatusCode {
    let node = Arc::new(RwLock::new(BoingNode::new()));
    let app = with_gate_env(token_env, || {
        rpc_router(node, &RateLimitConfig::default(), None, None, None)
    });
    let mut req = Request::builder().method("GET").uri("/live");
    if let Some(h) = gate_header {
        req = req.header("x-boing-origin-gate", h);
    }
    let res = app.oneshot(req.body(Body::empty()).unwrap()).await.unwrap();
    res.status()
}

async fn rpc_status(token_env: Option<&str>, gate_header: Option<&str>) -> StatusCode {
    let node = Arc::new(RwLock::new(BoingNode::new()));
    let app = with_gate_env(token_env, || {
        rpc_router(node, &RateLimitConfig::default(), None, None, None)
    });
    let mut req = Request::builder()
        .method("POST")
        .uri("/")
        .header("content-type", "application/json");
    if let Some(h) = gate_header {
        req = req.header("x-boing-origin-gate", h);
    }
    let body = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"boing_health","params":[]});
    let res = app
        .oneshot(req.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    res.status()
}

#[tokio::test]
async fn gate_off_allows_rpc_without_header() {
    assert_eq!(rpc_status(None, None).await, StatusCode::OK);
    assert_eq!(live_status(None, None).await, StatusCode::OK);
}

#[tokio::test]
async fn gate_on_blocks_rpc_without_header() {
    assert_eq!(
        rpc_status(Some("gate-secret"), None).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        rpc_status(Some("gate-secret"), Some("wrong")).await,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn gate_on_allows_rpc_with_matching_header() {
    assert_eq!(
        rpc_status(Some("gate-secret"), Some("gate-secret")).await,
        StatusCode::OK
    );
}

#[tokio::test]
async fn gate_on_still_allows_live_without_header() {
    // Fly http_service.checks hit GET /live with no gate header.
    assert_eq!(live_status(Some("gate-secret"), None).await, StatusCode::OK);
}

#[tokio::test]
async fn gate_reject_body_is_json() {
    let node = Arc::new(RwLock::new(BoingNode::new()));
    let app = with_gate_env(Some("gate-secret"), || {
        rpc_router(node, &RateLimitConfig::default(), None, None, None)
    });
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"jsonrpc":"2.0","id":1,"method":"boing_health","params":[]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["error"], "origin_gate");
}
