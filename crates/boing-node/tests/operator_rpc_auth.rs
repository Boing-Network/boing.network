//! Operator RPC gate: fails closed without a configured token.

use std::sync::Arc;

use axum::body::Body;
use axum::http::Request;
use boing_node::node::BoingNode;
use boing_node::rpc::rpc_router;
use boing_node::security::RateLimitConfig;
use http_body_util::BodyExt;
use tokio::sync::RwLock;
use tower::ServiceExt;

const OPERATOR_METHODS: [&str; 8] = [
    "boing_operatorApplyFeePolicy",
    "boing_operatorApplyQaPolicy",
    "boing_governancePropose",
    "boing_governanceAdvanceCooling",
    "boing_governanceAdvanceExecution",
    "boing_governanceExecute",
    "boing_submitSlashAppeal",
    "boing_resolveSlashAppeal",
];

async fn call(token: Option<&str>, header: Option<&str>, method: &str) -> serde_json::Value {
    let node = Arc::new(RwLock::new(BoingNode::new()));
    let app = rpc_router(
        node,
        &RateLimitConfig::default(),
        None,
        token.map(Arc::from),
        None,
    );
    let mut req = Request::builder()
        .method("POST")
        .uri("/")
        .header("content-type", "application/json");
    if let Some(h) = header {
        req = req.header("x-boing-operator", h);
    }
    let body = serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": []});
    let res = app
        .oneshot(req.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

fn denied(v: &serde_json::Value) -> bool {
    v["error"]["code"] == serde_json::json!(-32057)
}

#[tokio::test]
async fn unset_token_rejects_operator_methods() {
    for m in OPERATOR_METHODS {
        assert!(denied(&call(None, None, m).await), "{m}");
        assert!(denied(&call(None, Some("guess"), m).await), "{m}");
    }
}

#[tokio::test]
async fn wrong_token_rejects_operator_methods() {
    for m in OPERATOR_METHODS {
        assert!(denied(&call(Some("s3cret"), None, m).await), "{m}");
        assert!(denied(&call(Some("s3cret"), Some("wrong"), m).await), "{m}");
    }
}

#[tokio::test]
async fn correct_token_passes_operator_gate() {
    for m in OPERATOR_METHODS {
        let v = call(Some("s3cret"), Some("s3cret"), m).await;
        assert!(!denied(&v), "{m}: {v:?}");
    }
}
