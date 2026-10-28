//! `boing qa apply` — push QA registry and pool config via `boing_operatorApplyQaPolicy`.

use std::path::Path;

use serde_json::json;

pub async fn run(
    rpc_url: &str,
    registry_path: &Path,
    pool_path: &Path,
    operator_token: Option<&str>,
) -> anyhow::Result<()> {
    let registry_json = std::fs::read_to_string(registry_path).map_err(|e| {
        boing_telemetry::component_warn("boing_cli::qa_apply", "cli", "read_registry_failed", &e);
        anyhow::anyhow!("read --registry: {}", e)
    })?;
    let pool_json = std::fs::read_to_string(pool_path).map_err(|e| {
        boing_telemetry::component_warn("boing_cli::qa_apply", "cli", "read_pool_failed", &e);
        anyhow::anyhow!("read --pool: {}", e)
    })?;

    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "boing_operatorApplyQaPolicy",
        "params": [registry_json, pool_json]
    });

    let client = reqwest::Client::new();
    let mut req = client.post(rpc_url).json(&body);
    if let Some(t) = operator_token {
        req = req.header("X-Boing-Operator", t);
    }

    let resp = req.send().await.map_err(|e| {
        boing_telemetry::component_warn(
            "boing_cli::qa_apply",
            "cli",
            "rpc_connect_failed",
            format!("{rpc_url}: {e}"),
        );
        anyhow::anyhow!("Cannot connect to {}: {}. Is the node running?", rpc_url, e)
    })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body_text = resp.text().await.unwrap_or_default();
        boing_telemetry::component_warn(
            "boing_cli::qa_apply",
            "cli",
            "rpc_http_error",
            format!("{status} {body_text}"),
        );
        anyhow::bail!("RPC returned {}: {}", status, body_text);
    }

    let text = resp.text().await?;
    let parsed: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
        boing_telemetry::component_warn(
            "boing_cli::qa_apply",
            "cli",
            "rpc_json_parse_failed",
            format!("{e}; body_len={}", text.len()),
        );
        anyhow::Error::from(e)
    })?;

    if let Some(err) = parsed.get("error") {
        let code = err.get("code").and_then(|v| v.as_i64()).unwrap_or(0);
        let msg = err
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown error");
        boing_telemetry::component_warn(
            "boing_cli::qa_apply",
            "cli",
            "rpc_jsonrpc_error",
            format!("code={code} message={msg}"),
        );
        anyhow::bail!("RPC error {}: {}", code, msg);
    }

    let ok = parsed
        .get("result")
        .and_then(|r| r.get("ok"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if ok {
        println!("✓ Applied QA policy on node (registry + pool config)");
    } else {
        boing_telemetry::component_warn(
            "boing_cli::qa_apply",
            "cli",
            "unexpected_rpc_result",
            format!("body_len={}", text.len()),
        );
        println!("Unexpected result; check node logs. Body: {}", text);
    }

    Ok(())
}
