//! `boing metrics register` — Register contract for dApp incentive tracking via RPC.

use serde_json::json;

pub async fn run(rpc_url: &str, contract: &str, owner: &str) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "boing_registerDappMetrics",
        "params": [contract, owner]
    });

    let resp = client.post(rpc_url).json(&body).send().await.map_err(|e| {
        boing_telemetry::component_warn(
            "boing_cli::metrics_register",
            "cli",
            "rpc_connect_failed",
            format!("{rpc_url}: {e}"),
        );
        anyhow::anyhow!(
            "Cannot connect to {}: {}. Run `boing dev` first.",
            rpc_url,
            e
        )
    })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body_text = resp.text().await.unwrap_or_default();
        boing_telemetry::component_warn(
            "boing_cli::metrics_register",
            "cli",
            "rpc_http_error",
            format!("{status} {body_text}"),
        );
        anyhow::bail!("RPC returned {}: {}", status, body_text);
    }

    let text = resp.text().await?;
    let parsed: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
        boing_telemetry::component_warn(
            "boing_cli::metrics_register",
            "cli",
            "rpc_json_parse_failed",
            format!("{e}; body_len={}", text.len()),
        );
        anyhow::Error::from(e)
    })?;

    if let Some(err) = parsed.get("error") {
        let msg = err
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown error");
        boing_telemetry::component_warn(
            "boing_cli::metrics_register",
            "cli",
            "rpc_jsonrpc_error",
            msg,
        );
        anyhow::bail!("Registration failed: {}", msg);
    }

    let result = parsed.get("result").and_then(|r| r.get("registered"));
    if result.and_then(|v| v.as_bool()).unwrap_or(false) {
        println!("✓ Contract registered for dApp incentive tracking");
        println!("  contract: {}", contract);
        println!("  owner: {}", owner);
    } else {
        boing_telemetry::component_warn(
            "boing_cli::metrics_register",
            "cli",
            "unexpected_rpc_result",
            format!("body_len={}", text.len()),
        );
        println!("Registration may have succeeded; check node logs.");
    }

    Ok(())
}
