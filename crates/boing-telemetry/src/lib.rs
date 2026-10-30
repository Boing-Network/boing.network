//! Shared structured [`tracing`] fields for Boing workspace crates.
//!
//! Use stable keys for log aggregation: `boing_component`, `component_event`, `error_message`.
//! Avoid dotted keys (e.g. `boing.component`) — they break `tracing` macro parsing.

/// Cap for RPC id / error text fields so a hostile client cannot flood logs (audit #61 item 52).
pub const LOG_FIELD_MAX_CHARS: usize = 512;

/// Truncate for log fields: keeps a UTF-8 prefix and marks overflow.
pub fn truncate_for_log(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max_chars).collect();
    out.push('…');
    out
}

/// Warning on an operational / semantic error path.
pub fn component_warn(
    target: &'static str,
    boing_component: &'static str,
    component_event: &'static str,
    error_message: impl std::fmt::Display,
) {
    let msg = truncate_for_log(&error_message.to_string(), LOG_FIELD_MAX_CHARS);
    tracing::warn!(
        target = target,
        boing_component = boing_component,
        component_event = component_event,
        error_message = %msg,
        "Boing component warning"
    );
}

/// Error-level structured log.
pub fn component_error(
    target: &'static str,
    boing_component: &'static str,
    component_event: &'static str,
    error_message: impl std::fmt::Display,
) {
    let msg = truncate_for_log(&error_message.to_string(), LOG_FIELD_MAX_CHARS);
    tracing::error!(
        target = target,
        boing_component = boing_component,
        component_event = component_event,
        error_message = %msg,
        "Boing component error"
    );
}

/// Info-level structured log (lifecycle / milestones).
pub fn component_info(
    target: &'static str,
    boing_component: &'static str,
    component_event: &'static str,
    message: &'static str,
) {
    tracing::info!(
        target = target,
        boing_component = boing_component,
        component_event = component_event,
        message,
    );
}

/// Structured debug (same key names as warnings; enable with `RUST_LOG=...=debug`).
pub fn component_debug(
    target: &'static str,
    boing_component: &'static str,
    component_event: &'static str,
    message: &'static str,
) {
    tracing::debug!(
        target = target,
        boing_component = boing_component,
        component_event = component_event,
        message,
    );
}

/// JSON-RPC HTTP error envelope (node). `jsonrpc_id` is usually `Option<serde_json::Value>` at the call site.
pub fn jsonrpc_error_response(
    method: &str,
    jsonrpc_id: &impl std::fmt::Debug,
    code: i32,
    err_message: &str,
    has_data: bool,
    http_status: u16,
) {
    let id_raw = format!("{jsonrpc_id:?}");
    let id = truncate_for_log(&id_raw, LOG_FIELD_MAX_CHARS);
    // Collapse newlines so line-oriented log shippers stay coherent.
    let msg = truncate_for_log(&err_message.replace(['\n', '\r'], " "), LOG_FIELD_MAX_CHARS);
    tracing::warn!(
        target = "boing_node::rpc",
        boing_component = "jsonrpc",
        component_event = "jsonrpc_error_response",
        rpc_method = %method,
        rpc_jsonrpc_id = %id,
        rpc_error_code = code,
        rpc_error_message = %msg,
        rpc_error_has_data = has_data,
        http_status = http_status,
        "JSON-RPC error response"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_marks_overflow() {
        let s = "a".repeat(LOG_FIELD_MAX_CHARS + 10);
        let t = truncate_for_log(&s, LOG_FIELD_MAX_CHARS);
        assert_eq!(t.chars().count(), LOG_FIELD_MAX_CHARS + 1);
        assert!(t.ends_with('…'));
    }
}
