//! Audit #61 item 41 — phased governance RPCs apply allowlisted fee policy.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::http::Request;
use boing_governance::Governance;
use boing_node::node::BoingNode;
use boing_node::rpc::rpc_router;
use boing_node::security::RateLimitConfig;
use http_body_util::BodyExt;
use tokio::sync::RwLock;
use tower::ServiceExt;

async fn rpc(
    node: Arc<RwLock<BoingNode>>,
    token: &str,
    method: &str,
    params: serde_json::Value,
) -> serde_json::Value {
    let app = rpc_router(
        node,
        &RateLimitConfig::default(),
        None,
        Some(Arc::from(token)),
        None,
    );
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
        .header("x-boing-operator", token)
        .body(Body::from(body.to_string()))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn phased_fee_policy_proposal_applies_on_execute() {
    let node = Arc::new(RwLock::new(BoingNode::new()));
    {
        let mut n = node.write().await;
        // Zero cooling so AdvanceExecution can run immediately in the test.
        n.governance = Governance::new(Duration::from_secs(0), Duration::from_secs(3600));
    }

    let fee_json = serde_json::to_string(&serde_json::json!({
        "fee_validators_bps": 0,
        "fee_treasury_bps": 10000,
        "fee_burn_bps": 0,
        "extra_fixed_fee": "0",
        "transfer_amount_bps": 0,
        "execution_gas_fee_bps": 100,
        "tip_proposer_bps": 10000,
        "tip_treasury_bps": 0,
        "tip_burn_bps": 0
    }))
    .unwrap();

    let proposed = rpc(
        node.clone(),
        "s3cret",
        "boing_governancePropose",
        serde_json::json!(["network_fee_config", fee_json]),
    )
    .await;
    assert_eq!(proposed["result"]["ok"], true, "{proposed:?}");
    let id = proposed["result"]["id"].as_u64().unwrap();

    let cool = rpc(
        node.clone(),
        "s3cret",
        "boing_governanceAdvanceCooling",
        serde_json::json!([id]),
    )
    .await;
    assert_eq!(cool["result"]["phase"], "cooling", "{cool:?}");

    // Wall clock may sit slightly behind proposed_at+0 on some hosts; nudge with library path.
    {
        let mut n = node.write().await;
        let now = SystemTime::now()
            .checked_add(Duration::from_secs(1))
            .unwrap_or(UNIX_EPOCH);
        n.governance_advance_execution(id, now).unwrap();
    }

    let exec = rpc(
        node.clone(),
        "s3cret",
        "boing_governanceExecute",
        serde_json::json!([id]),
    )
    .await;
    assert_eq!(exec["result"]["applied"], true, "{exec:?}");
    assert_eq!(exec["result"]["target_key"], "network_fee_config");

    let got = rpc(
        node.clone(),
        "s3cret",
        "boing_getNetworkFeePolicy",
        serde_json::json!([]),
    )
    .await;
    assert_eq!(got["result"]["execution_gas_fee_bps"], 100, "{got:?}");
}

#[tokio::test]
async fn unknown_target_key_rejected() {
    let node = Arc::new(RwLock::new(BoingNode::new()));
    let v = rpc(
        node,
        "s3cret",
        "boing_governancePropose",
        serde_json::json!(["not_a_key", "{}"]),
    )
    .await;
    assert!(v.get("error").is_some(), "{v:?}");
}
