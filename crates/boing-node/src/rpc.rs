//! Simple JSON-RPC over HTTP for Boing node.
//!
//! Supports boing_submitTransaction for submitting signed transactions.
//! Optional global rate limiting when RateLimitConfig.requests_per_sec > 0.
//! Optional testnet faucet (boing_faucetRequest) when --faucet-enable.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::body::Bytes;
use axum::extract::Request as IncomingRequest;
use axum::http::header::{ACCEPT, CONTENT_TYPE};
use axum::http::Method;
use axum::http::Uri;
use axum::http::{HeaderMap, HeaderName, HeaderValue, Request as HttpRequest, StatusCode};
use axum::{
    extract::DefaultBodyLimit,
    extract::State,
    middleware::{self, Next},
    response::IntoResponse,
    response::Response,
    routing::get,
    Json, Router,
};
use governor::{Quota, RateLimiter};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;
use tokio::sync::{broadcast, Mutex as TokioMutex, RwLock};
use tower::ServiceBuilder;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, RequestId, SetRequestIdLayer};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::{DefaultOnFailure, DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tower_http::LatencyUnit;
use tracing::info;
use tracing::Level;

use crate::faucet::{self, testnet_faucet_account_id};
use crate::mempool::MempoolError;
use crate::node::{BoingNode, QaPoolVoteResult};
use crate::security::RateLimitConfig;
use boing_primitives::{
    create2_contract_address, nonce_derived_contract_address, AccessList, AccountId, ExecutionLog,
    ExecutionReceipt, Hash, SignedIntent, SignedTransaction, Transaction, TransactionPayload,
    MAX_EXECUTION_LOG_TOPICS,
};
use boing_qa::pool::{PoolError, QaPoolVote};
use boing_qa::{
    check_contract_deploy_full_with_metadata, qa_pool_config_from_json, rule_registry_from_json,
    QaPoolExpiryPolicy, QaResult, RuleRegistry,
};
use boing_tokenomics::BLOCK_TIME_SECS;

/// Shared node state for RPC and validator loop.
pub type NodeState = Arc<RwLock<BoingNode>>;

/// Per-account cooldown for faucet (testnet only).
pub type FaucetCooldown = Arc<std::sync::Mutex<HashMap<AccountId, Instant>>>;

/// Serializes `boing_faucetRequest` build+submit so concurrent RPCs do not read the same nonce
/// and collide (duplicate tx id or accidental replacement at the same nonce).
/// Tokio mutex so the guard is not held across `.await` with a `std::sync` guard (would make the handler `!Send`).
pub type FaucetSubmitLock = Arc<TokioMutex<()>>;

/// RPC handler state: node + optional global rate limiter + optional testnet faucet.
#[derive(Clone)]
pub struct RpcState {
    pub node: NodeState,
    pub rate_limiter: Option<Arc<governor::DefaultDirectRateLimiter>>,
    /// When set, enables boing_faucetRequest (testnet only).
    pub faucet_signer: Option<Arc<ed25519_dalek::SigningKey>>,
    pub faucet_cooldown: Option<FaucetCooldown>,
    pub faucet_submit_lock: Option<FaucetSubmitLock>,
    /// When set, `boing_qaPoolVote` and `boing_operatorApplyQaPolicy` require header `X-Boing-Operator: <token>`.
    pub operator_rpc_token: Option<Arc<str>>,
    /// Broadcast channel for WebSocket **`/ws`** **newHeads** subscribers (clone per connection).
    pub head_broadcast: Option<broadcast::Sender<serde_json::Value>>,
    /// Concurrent **`GET /ws`** connections (increment on accept, decrement on close). `ws_max_connections == 0` = unlimited.
    pub ws_active: Arc<AtomicUsize>,
    pub ws_max_connections: usize,
    /// **`RateLimitConfig.requests_per_sec`** used to build this router (**0** = HTTP JSON-RPC rate limit off). Exposed in **`boing_health`**.
    pub rate_limit_requests_per_sec: u32,
    /// Cumulative HTTP / JSON-RPC diagnostics (also returned on **`boing_health`**).
    pub rpc_metrics: Arc<RpcHttpMetrics>,
}

/// Monotonic counters for RPC UX / operator dashboards (best-effort; not a billing metric).
#[derive(Default)]
pub struct RpcHttpMetrics {
    pub rate_limited: AtomicU64,
    pub json_parse_errors: AtomicU64,
    pub batch_too_large: AtomicU64,
    pub method_not_found: AtomicU64,
    pub websocket_cap_rejects: AtomicU64,
}

impl RpcState {
    fn operator_authorized(&self, headers: &HeaderMap) -> bool {
        match &self.operator_rpc_token {
            None => true,
            Some(expected) => headers
                .get("x-boing-operator")
                .and_then(|v| v.to_str().ok())
                .is_some_and(|v| v == expected.as_ref()),
        }
    }
}

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: Option<String>,
    id: Option<serde_json::Value>,
    method: String,
    params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
    /// Optional HTTP-level hint for operators (e.g. rate limit + **`x-request-id`** correlation).
    #[serde(skip_serializing_if = "Option::is_none")]
    boing_http: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

fn rpc_error(id: Option<serde_json::Value>, code: i32, message: String) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: None,
        error: Some(JsonRpcError {
            code,
            message,
            data: None,
        }),
        boing_http: None,
    }
}

fn rpc_error_with_data(
    id: Option<serde_json::Value>,
    code: i32,
    message: String,
    data: serde_json::Value,
) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: None,
        error: Some(JsonRpcError {
            code,
            message,
            data: Some(data),
        }),
        boing_http: None,
    }
}

fn rpc_ok(id: Option<serde_json::Value>, result: serde_json::Value) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: Some(result),
        error: None,
        boing_http: None,
    }
}

/// Emit a structured **warn** when the JSON-RPC body carries an `error` object (semantic fields for operators).
fn respond_jsonrpc(
    method: &str,
    status: StatusCode,
    body: JsonRpcResponse,
) -> (StatusCode, Json<JsonRpcResponse>) {
    if let Some(ref err) = body.error {
        boing_telemetry::jsonrpc_error_response(
            method,
            &body.id,
            err.code,
            &err.message,
            err.data.is_some(),
            status.as_u16(),
        );
    }
    (status, Json(body))
}

/// HTTP **429** JSON-RPC error with **`Retry-After: 1`** (quota is per-second).
fn respond_jsonrpc_rate_limited(
    method: &str,
    mut body: JsonRpcResponse,
    request_id: Option<&str>,
) -> Response {
    if let Some(ref err) = body.error {
        boing_telemetry::jsonrpc_error_response(
            method,
            &body.id,
            err.code,
            &err.message,
            err.data.is_some(),
            StatusCode::TOO_MANY_REQUESTS.as_u16(),
        );
    }
    let rid = request_id.map(str::to_string).filter(|s| !s.is_empty());
    body.boing_http = Some(serde_json::json!({
        "code": "rate_limited",
        "message": "HTTP JSON-RPC global rate limit exceeded; honor Retry-After and see boing_health.rpc_surface.http_rate_limit_requests_per_sec.",
        "request_id": rid,
    }));
    (
        StatusCode::TOO_MANY_REQUESTS,
        [(
            HeaderName::from_static("retry-after"),
            HeaderValue::from_static("1"),
        )],
        Json(body),
    )
        .into_response()
}

/// **`GET /`** — browsers and `curl` without `-X POST` hit this; point them at JSON-RPC.
async fn http_root_get() -> impl IntoResponse {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        [(
            HeaderName::from_static("allow"),
            HeaderValue::from_static("GET, HEAD, POST, OPTIONS"),
        )],
        concat!(
            "Boing JSON-RPC: use POST / with Content-Type: application/json (JSON-RPC 2.0).\n",
            "Discovery: GET /openapi.json, GET /.well-known/boing-rpc.\n",
            "Probes: GET /live, GET /ready (JSON: /live.json, /ready.json, or Accept: application/json).\n",
            "WebSocket: GET /ws (newHeads).\n",
            "See boing_getNetworkInfo.developer and docs/RPC-API-SPEC.md\n",
        ),
    )
}

/// **`HEAD /`** — same constraints as **`GET /`** (no body); for cheap proxy checks.
async fn http_head_root() -> impl IntoResponse {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        [(
            HeaderName::from_static("allow"),
            HeaderValue::from_static("GET, HEAD, POST, OPTIONS"),
        )],
    )
}

/// **`OPTIONS /`** — CORS preflight and `Allow` discovery (`curl -X OPTIONS`).
async fn http_options_root() -> impl IntoResponse {
    (
        StatusCode::NO_CONTENT,
        [(
            HeaderName::from_static("allow"),
            HeaderValue::from_static("GET, HEAD, POST, OPTIONS"),
        )],
    )
}

fn execution_logs_to_json(logs: &[ExecutionLog]) -> serde_json::Value {
    serde_json::Value::Array(
        logs.iter()
            .map(|log| {
                serde_json::json!({
                    "topics": log
                        .topics
                        .iter()
                        .map(|t| format!("0x{}", hex::encode(t.as_slice())))
                        .collect::<Vec<_>>(),
                    "data": format!("0x{}", hex::encode(&log.data)),
                })
            })
            .collect(),
    )
}

fn access_list_to_json(al: &AccessList) -> serde_json::Value {
    serde_json::json!({
        "read": al
            .read
            .iter()
            .map(|a| format!("0x{}", hex::encode(a.0)))
            .collect::<Vec<_>>(),
        "write": al
            .write
            .iter()
            .map(|a| format!("0x{}", hex::encode(a.0)))
            .collect::<Vec<_>>(),
    })
}

fn execution_receipt_to_json(r: &ExecutionReceipt) -> serde_json::Value {
    serde_json::json!({
        "tx_id": format!("0x{}", hex::encode(r.tx_id.0)),
        "block_height": r.block_height,
        "tx_index": r.tx_index,
        "success": r.success,
        "gas_used": r.gas_used,
        "return_data": format!("0x{}", hex::encode(&r.return_data)),
        "logs": execution_logs_to_json(&r.logs),
        "error": r.error,
    })
}

fn parse_hash32_hex(s: &str) -> Result<Hash, String> {
    let bytes = hex::decode(s.trim_start_matches("0x")).map_err(|e| e.to_string())?;
    if bytes.len() != 32 {
        return Err("tx_hash must be 32 bytes".into());
    }
    Hash::from_slice(&bytes).ok_or_else(|| "invalid hash".into())
}

fn parse_account_id_hex(s: &str) -> Result<AccountId, String> {
    let bytes = hex::decode(s.trim_start_matches("0x")).map_err(|e| e.to_string())?;
    if bytes.len() != 32 {
        return Err("account id must be 32 bytes".into());
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(AccountId(arr))
}

/// Max inclusive block span for `boing_getLogs` (prevents unbounded scans).
const GET_LOGS_MAX_BLOCK_RANGE: u64 = 128;
/// Max log entries returned per `boing_getLogs` call.
const GET_LOGS_MAX_RESULTS: usize = 2048;

/// `boing_*` method names implemented by this binary; returned by `boing_rpcSupportedMethods`.
/// Keep sorted and in sync with `rpc_router` match arms for `boing_*` methods.
const BOING_RPC_SUPPORTED_METHODS: &[&str] = &[
    "boing_chainHeight",
    "boing_clientVersion",
    "boing_faucetRequest",
    "boing_getAccount",
    "boing_getAccountProof",
    "boing_getBalance",
    "boing_getBlockByHash",
    "boing_getBlockByHeight",
    "boing_getContractStorage",
    "boing_getLogs",
    "boing_getNetworkInfo",
    "boing_getQaRegistry",
    "boing_getRpcMethodCatalog",
    "boing_getRpcOpenApi",
    "boing_getSyncState",
    "boing_getTransactionReceipt",
    "boing_health",
    "boing_operatorApplyQaPolicy",
    "boing_qaCheck",
    "boing_qaPoolConfig",
    "boing_qaPoolList",
    "boing_qaPoolVote",
    "boing_registerDappMetrics",
    "boing_rpcSupportedMethods",
    "boing_simulateTransaction",
    "boing_submitIntent",
    "boing_submitTransaction",
    "boing_verifyAccountProof",
];

static DEVELOPER_API_DOCUMENT: OnceLock<serde_json::Value> = OnceLock::new();

fn developer_api_document() -> &'static serde_json::Value {
    DEVELOPER_API_DOCUMENT.get_or_init(|| {
        serde_json::from_str(include_str!("../schemas/developer_api.json"))
            .expect("developer_api.json must parse")
    })
}

/// Default for `boing_getNetworkInfo.developer.repository_url` when `BOING_DEVELOPER_REPOSITORY_URL` is unset.
const DEFAULT_DEVELOPER_REPOSITORY_URL: &str = "https://github.com/Boing-Network/boing.network";

fn network_info_developer_hints() -> serde_json::Value {
    let repository_url = std::env::var("BOING_DEVELOPER_REPOSITORY_URL")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_DEVELOPER_REPOSITORY_URL.to_string());
    let base = repository_url.trim_end_matches('/').to_string();
    let rpc_spec_url = std::env::var("BOING_DEVELOPER_RPC_SPEC_URL")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{}/blob/main/docs/RPC-API-SPEC.md", base));
    let dapp_integration_doc_url = std::env::var("BOING_DEVELOPER_DAPP_DOC_URL")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{}/blob/main/docs/BOING-DAPP-INTEGRATION.md", base));
    serde_json::json!({
        "repository_url": repository_url,
        "rpc_spec_url": rpc_spec_url,
        "dapp_integration_doc_url": dapp_integration_doc_url,
        "sdk_npm_package": "boing-sdk",
        "websocket": {
            "path": "/ws",
            "handshake": {"type": "subscribe", "channel": "newHeads"},
            "event_types": ["newHead"]
        },
        "api_discovery_methods": ["boing_getRpcMethodCatalog", "boing_getRpcOpenApi", "boing_rpcSupportedMethods"],
        "http": {
            "live_path": "/live",
            "ready_path": "/ready",
            "live_json_path": "/live.json",
            "ready_json_path": "/ready.json",
            "openapi_http_path": "/openapi.json",
            "well_known_boing_rpc_path": "/.well-known/boing-rpc",
            "jsonrpc_post_path": "/",
            "response_header_rpc_version": "x-boing-rpc-version",
            "request_id_header": "x-request-id",
            "supports_jsonrpc_batch": true,
            "jsonrpc_batch_max_env": "BOING_RPC_MAX_BATCH",
            "jsonrpc_max_body_mb_env": "BOING_RPC_MAX_BODY_MB",
            "websocket_max_connections_env": "BOING_RPC_WS_MAX_CONNECTIONS",
            "ready_min_peers_env": "BOING_RPC_READY_MIN_PEERS"
        }
    })
}

/// Max JSON-RPC POST body size (default **8** MiB). Operators may raise for large deploy calldata.
fn max_rpc_body_bytes() -> usize {
    const DEFAULT_MB: usize = 8;
    const MIN_MB: usize = 1;
    match std::env::var("BOING_RPC_MAX_BODY_MB") {
        Ok(s) => match s.trim().parse::<usize>() {
            Ok(mb) if mb >= MIN_MB => {
                let bytes = mb.saturating_mul(1024 * 1024);
                if mb != DEFAULT_MB {
                    tracing::info!(
                        megabytes = mb,
                        "BOING_RPC_MAX_BODY_MB: max JSON-RPC POST body size"
                    );
                }
                bytes
            }
            Ok(_) => {
                tracing::warn!(
                    "BOING_RPC_MAX_BODY_MB must be >= {} MiB; using default {}",
                    MIN_MB,
                    DEFAULT_MB
                );
                DEFAULT_MB * 1024 * 1024
            }
            Err(_) => {
                tracing::warn!("BOING_RPC_MAX_BODY_MB invalid; using default {}", DEFAULT_MB);
                DEFAULT_MB * 1024 * 1024
            }
        },
        Err(_) => DEFAULT_MB * 1024 * 1024,
    }
}

/// Max JSON-RPC objects per **batch** POST (default **32**, hard cap **256**). Set `BOING_RPC_MAX_BATCH`.
fn max_rpc_batch_len() -> usize {
    const DEFAULT: usize = 32;
    const HARD_CAP: usize = 256;
    match std::env::var("BOING_RPC_MAX_BATCH") {
        Ok(s) => match s.trim().parse::<usize>() {
            Ok(0) => {
                tracing::warn!("BOING_RPC_MAX_BATCH 0 is invalid; using default {}", DEFAULT);
                DEFAULT
            }
            Ok(n) => n.min(HARD_CAP),
            Err(_) => {
                tracing::warn!("BOING_RPC_MAX_BATCH invalid; using default {}", DEFAULT);
                DEFAULT
            }
        },
        Err(_) => DEFAULT,
    }
}

/// Max concurrent **`GET /ws`** connections (default **0** = unlimited). Set **`BOING_RPC_WS_MAX_CONNECTIONS`**.
fn max_ws_connections() -> usize {
    match std::env::var("BOING_RPC_WS_MAX_CONNECTIONS") {
        Ok(s) => match s.trim().parse::<usize>() {
            Ok(n) => {
                if n > 0 {
                    tracing::info!(
                        max = n,
                        "BOING_RPC_WS_MAX_CONNECTIONS: rejecting new WebSocket handshakes when at cap"
                    );
                }
                n
            }
            Err(_) => {
                tracing::warn!("BOING_RPC_WS_MAX_CONNECTIONS invalid; WebSocket connections unlimited");
                0
            }
        },
        Err(_) => 0,
    }
}

/// When set, `GET /ready` returns **503** unless `connected_peers().len() >= N`. Unset = no peer check.
fn ready_min_peers() -> Option<u32> {
    static PARSED: OnceLock<Option<u32>> = OnceLock::new();
    *PARSED.get_or_init(|| {
        match std::env::var("BOING_RPC_READY_MIN_PEERS") {
            Ok(s) => match s.trim().parse::<u32>() {
                Ok(0) => None,
                Ok(n) => {
                    tracing::info!(
                        min_peers = n,
                        "BOING_RPC_READY_MIN_PEERS: /ready requires at least this many P2P peers"
                    );
                    Some(n)
                }
                Err(_) => {
                    tracing::warn!("BOING_RPC_READY_MIN_PEERS invalid; ignoring peer requirement");
                    None
                }
            },
            Err(_) => None,
        }
    })
}

fn max_rpc_body_megabytes() -> u64 {
    (max_rpc_body_bytes() / (1024 * 1024)).max(1) as u64
}

fn accept_requests_json(headers: &HeaderMap) -> bool {
    let Some(raw) = headers.get(ACCEPT).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    for part in raw.split(',') {
        let mut it = part.trim().split(';');
        let mime = it.next().unwrap_or("").trim();
        if mime.eq_ignore_ascii_case("application/json") {
            let mut q = 1.0f32;
            for p in it {
                let p = p.trim();
                if let Some(rest) = p.strip_prefix("q=") {
                    if let Ok(qv) = rest.trim().parse::<f32>() {
                        q = qv;
                    }
                }
            }
            if q > 0.0 {
                return true;
            }
        }
    }
    false
}

fn probe_wants_json_path(uri_path: &str, headers: &HeaderMap) -> bool {
    uri_path.ends_with(".json") || accept_requests_json(headers)
}

fn rpc_surface_capabilities(state: &RpcState) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc_batch_max": max_rpc_batch_len(),
        "websocket_max_connections": max_ws_connections(),
        "http_rate_limit_requests_per_sec": state.rate_limit_requests_per_sec,
        "ready_min_peers": ready_min_peers(),
        "http_max_body_megabytes": max_rpc_body_megabytes(),
        "get_logs_max_block_range": GET_LOGS_MAX_BLOCK_RANGE,
        "get_logs_max_results": GET_LOGS_MAX_RESULTS,
        "max_log_topic_filters": MAX_EXECUTION_LOG_TOPICS,
    })
}

fn rpc_metrics_snapshot(state: &RpcState) -> serde_json::Value {
    serde_json::json!({
        "rate_limited_total": state.rpc_metrics.rate_limited.load(Ordering::Relaxed),
        "json_parse_errors_total": state.rpc_metrics.json_parse_errors.load(Ordering::Relaxed),
        "batch_too_large_total": state.rpc_metrics.batch_too_large.load(Ordering::Relaxed),
        "method_not_found_total": state.rpc_metrics.method_not_found.load(Ordering::Relaxed),
        "websocket_cap_rejects_total": state.rpc_metrics.websocket_cap_rejects.load(Ordering::Relaxed),
    })
}

fn well_known_boing_rpc_document() -> serde_json::Value {
    serde_json::json!({
        "schema_version": 1,
        "openapi_url": "/openapi.json",
        "jsonrpc_post_path": "/",
        "live_plain_path": "/live",
        "live_json_path": "/live.json",
        "ready_plain_path": "/ready",
        "ready_json_path": "/ready.json",
        "websocket_path": "/ws",
    })
}

/// Optional 32-byte `AccountId` from env for **`boing_getNetworkInfo.end_user`** (lowercase `0x` + 64 hex).
/// Malformed values are logged and ignored so a typo does not break JSON-RPC.
fn env_optional_account_id_hex(var: &'static str) -> Option<String> {
    let Ok(raw) = std::env::var(var) else {
        return None;
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let body = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    if body.len() != 64 || !body.chars().all(|c| c.is_ascii_hexdigit()) {
        tracing::warn!(
            %var,
            value = %trimmed,
            "invalid 32-byte hex AccountId for network hints; expected 64 hex chars (optional 0x prefix)"
        );
        return None;
    }
    Some(format!("0x{}", body.to_ascii_lowercase()))
}

fn end_user_network_hints() -> serde_json::Value {
    let chain_display_name = std::env::var("BOING_CHAIN_DISPLAY_NAME")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let explorer_url = std::env::var("BOING_EXPLORER_URL")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let faucet_url = std::env::var("BOING_FAUCET_URL")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    serde_json::json!({
        "chain_display_name": chain_display_name,
        "explorer_url": explorer_url,
        "faucet_url": faucet_url,
        "canonical_native_cp_pool": env_optional_account_id_hex("BOING_CANONICAL_NATIVE_CP_POOL"),
        "canonical_native_dex_factory": env_optional_account_id_hex("BOING_CANONICAL_NATIVE_DEX_FACTORY"),
        "canonical_native_dex_multihop_swap_router": env_optional_account_id_hex("BOING_CANONICAL_NATIVE_DEX_MULTIHOP_SWAP_ROUTER"),
        "canonical_native_dex_ledger_router_v2": env_optional_account_id_hex("BOING_CANONICAL_NATIVE_DEX_LEDGER_ROUTER_V2"),
        "canonical_native_dex_ledger_router_v3": env_optional_account_id_hex("BOING_CANONICAL_NATIVE_DEX_LEDGER_ROUTER_V3"),
        "canonical_native_amm_lp_vault": env_optional_account_id_hex("BOING_CANONICAL_NATIVE_AMM_LP_VAULT"),
        "canonical_native_lp_share_token": env_optional_account_id_hex("BOING_CANONICAL_NATIVE_LP_SHARE_TOKEN"),
    })
}

/// One-line effective RPC config after env parsing (complements per-var logs from limit helpers).
pub fn log_rpc_config_banner(rpc_listen: &str, rate_limit_rps: u32) {
    info!(
        listen = %rpc_listen,
        jsonrpc_max_batch = max_rpc_batch_len(),
        jsonrpc_max_body_mib = max_rpc_body_megabytes(),
        websocket_max_connections = max_ws_connections(),
        ready_min_peers = ?ready_min_peers(),
        http_rate_limit_rps = rate_limit_rps,
        "Boing RPC: POST / (JSON-RPC); GET /openapi.json; GET /.well-known/boing-rpc; probes /live /ready (+ .json); WS /ws",
    );
}

async fn map_payload_too_large_to_json(req: IncomingRequest, next: Next) -> Response {
    let res = next.run(req).await;
    if res.status() != StatusCode::PAYLOAD_TOO_LARGE {
        return res;
    }
    let (mut parts, _) = res.into_parts();
    let rid = parts
        .headers
        .get(HeaderName::from_static("x-request-id"))
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let mb = max_rpc_body_megabytes();
    let body = serde_json::json!({
        "code": "payload_too_large",
        "message": format!(
            "JSON-RPC POST body exceeds the configured maximum ({} MiB). Raise BOING_RPC_MAX_BODY_MB on the node if legitimate.",
            mb
        ),
        "request_id": rid,
    });
    parts.headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    Response::from_parts(parts, Body::from(body.to_string()))
}

/// **`GET /live`** — plain **`ok\\n`** by default, or JSON when **`Accept: application/json`** or path **`/live.json`**.
async fn http_live(uri: Uri, headers: HeaderMap) -> Response {
    if probe_wants_json_path(uri.path(), &headers) {
        return (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response();
    }
    (StatusCode::OK, "ok\n").into_response()
}

async fn http_head_live() -> StatusCode {
    StatusCode::OK
}

/// **`GET /ready`** — optional peer gate; JSON with **`503`** + **`Retry-After: 5`** when **`BOING_RPC_READY_MIN_PEERS`** not met.
async fn http_ready(State(state): State<RpcState>, uri: Uri, headers: HeaderMap) -> Response {
    let json = probe_wants_json_path(uri.path(), &headers);
    let n = state.node.read().await;
    if let Some(min) = ready_min_peers() {
        let count = n.p2p.connected_peers().await.len() as u32;
        if count < min {
            if json {
                let body = serde_json::json!({
                    "ready": false,
                    "reason": "peers_below_min",
                    "peers": count,
                    "min_peers": min,
                });
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    [(
                        HeaderName::from_static("retry-after"),
                        HeaderValue::from_static("5"),
                    )],
                    Json(body),
                )
                    .into_response();
            }
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                [(
                    HeaderName::from_static("retry-after"),
                    HeaderValue::from_static("5"),
                )],
                format!("not_ready: peers {} < {}\n", count, min),
            )
                .into_response();
        }
    }
    if json {
        return (StatusCode::OK, Json(serde_json::json!({ "ready": true }))).into_response();
    }
    (StatusCode::OK, "ready\n").into_response()
}

async fn http_head_ready(State(state): State<RpcState>) -> impl IntoResponse {
    let n = state.node.read().await;
    if let Some(min) = ready_min_peers() {
        let count = n.p2p.connected_peers().await.len() as u32;
        if count < min {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                [(
                    HeaderName::from_static("retry-after"),
                    HeaderValue::from_static("5"),
                )],
            )
                .into_response();
        }
    }
    StatusCode::OK.into_response()
}

async fn http_openapi_json() -> Json<serde_json::Value> {
    let root = developer_api_document();
    let oa = root
        .get("openapi")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    Json(oa)
}

async fn http_well_known_boing_rpc() -> Json<serde_json::Value> {
    Json(well_known_boing_rpc_document())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetLogsFilterParams {
    from_block: serde_json::Value,
    to_block: serde_json::Value,
    #[serde(default)]
    address: Option<String>,
    #[serde(default)]
    topics: Option<Vec<serde_json::Value>>,
}

fn json_block_number(v: &serde_json::Value) -> Result<u64, String> {
    match v {
        serde_json::Value::Number(n) => n
            .as_u64()
            .ok_or_else(|| "block number must fit in u64".to_string()),
        serde_json::Value::String(s) => {
            let s = s.trim();
            if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
                u64::from_str_radix(hex, 16).map_err(|e| e.to_string())
            } else {
                s.parse::<u64>().map_err(|e| e.to_string())
            }
        }
        _ => Err("block number must be a JSON number or decimal/hex string".into()),
    }
}

fn deployed_contract_address(tx: &Transaction) -> Option<AccountId> {
    match &tx.payload {
        TransactionPayload::ContractDeploy {
            bytecode,
            create2_salt,
        } => Some(match create2_salt {
            Some(salt) => create2_contract_address(&tx.sender, salt, bytecode),
            None => nonce_derived_contract_address(&tx.sender, tx.nonce),
        }),
        TransactionPayload::ContractDeployWithPurpose {
            bytecode,
            create2_salt,
            ..
        } => Some(match create2_salt {
            Some(salt) => create2_contract_address(&tx.sender, salt, bytecode),
            None => nonce_derived_contract_address(&tx.sender, tx.nonce),
        }),
        TransactionPayload::ContractDeployWithPurposeAndMetadata {
            bytecode,
            create2_salt,
            ..
        } => Some(match create2_salt {
            Some(salt) => create2_contract_address(&tx.sender, salt, bytecode),
            None => nonce_derived_contract_address(&tx.sender, tx.nonce),
        }),
        _ => None,
    }
}

/// Contract whose execution produced logs (call target or address created by deploy).
fn log_emitting_account(tx: &Transaction) -> Option<AccountId> {
    match &tx.payload {
        TransactionPayload::ContractCall { contract, .. } => Some(*contract),
        TransactionPayload::ContractDeploy { .. }
        | TransactionPayload::ContractDeployWithPurpose { .. }
        | TransactionPayload::ContractDeployWithPurposeAndMetadata { .. } => {
            deployed_contract_address(tx)
        }
        _ => None,
    }
}

fn parse_topic_word(v: &serde_json::Value) -> Result<Option<[u8; 32]>, String> {
    if v.is_null() {
        return Ok(None);
    }
    let s = v
        .as_str()
        .ok_or_else(|| "each topic filter must be null or a 32-byte hex string".to_string())?;
    let bytes = hex::decode(s.trim_start_matches("0x")).map_err(|e| e.to_string())?;
    if bytes.len() != 32 {
        return Err("topic must be 32 bytes hex".into());
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(Some(arr))
}

fn parse_topics_filter(
    raw: Option<Vec<serde_json::Value>>,
) -> Result<Vec<Option<[u8; 32]>>, String> {
    let Some(vec) = raw else {
        return Ok(Vec::new());
    };
    if vec.len() > boing_primitives::MAX_EXECUTION_LOG_TOPICS {
        return Err(format!(
            "topics filter length must be at most {}",
            boing_primitives::MAX_EXECUTION_LOG_TOPICS
        ));
    }
    let mut out = Vec::with_capacity(vec.len());
    for v in vec {
        out.push(parse_topic_word(&v)?);
    }
    Ok(out)
}

fn log_matches_topic_filter(log: &ExecutionLog, filter: &[Option<[u8; 32]>]) -> bool {
    for (i, want) in filter.iter().enumerate() {
        if let Some(expected) = want {
            match log.topics.get(i) {
                Some(actual) if actual == expected => {}
                _ => return false,
            }
        }
    }
    true
}

fn parse_qa_pool_vote(s: &str) -> Result<QaPoolVote, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "allow" => Ok(QaPoolVote::Allow),
        "reject" => Ok(QaPoolVote::Reject),
        "abstain" => Ok(QaPoolVote::Abstain),
        _ => Err("vote must be allow, reject, or abstain".into()),
    }
}


async fn handle_rpc(
    State(state): State<RpcState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let hdr_request_id = headers
        .get(HeaderName::from_static("x-request-id"))
        .and_then(|v| v.to_str().ok());

    if let Some(ref limiter) = state.rate_limiter {
        if limiter.check().is_err() {
            state.rpc_metrics.rate_limited.fetch_add(1, Ordering::Relaxed);
            return respond_jsonrpc_rate_limited(
                "jsonrpc",
                rpc_error(
                    None,
                    -32016,
                    "Rate limit exceeded. Try again later.".into(),
                ),
                hdr_request_id,
            );
        }
    }

    let v: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            state
                .rpc_metrics
                .json_parse_errors
                .fetch_add(1, Ordering::Relaxed);
            return respond_jsonrpc(
                "jsonrpc",
                StatusCode::OK,
                rpc_error(
                    None,
                    -32700,
                    format!("Parse error: {}", e),
                ),
            )
            .into_response();
        }
    };

    match v {
        serde_json::Value::Array(items) => {
            let max_batch = max_rpc_batch_len();
            if items.len() > max_batch {
                state
                    .rpc_metrics
                    .batch_too_large
                    .fetch_add(1, Ordering::Relaxed);
                return respond_jsonrpc(
                    "jsonrpc",
                    StatusCode::OK,
                    rpc_error(
                        None,
                        -32600,
                        format!(
                            "Invalid Request: batch exceeds max length {} (BOING_RPC_MAX_BATCH)",
                            max_batch
                        ),
                    ),
                )
                .into_response();
            }
            if items.is_empty() {
                return (StatusCode::OK, Json(Vec::<JsonRpcResponse>::new())).into_response();
            }
            let mut out: Vec<JsonRpcResponse> = Vec::with_capacity(items.len());
            for item in items {
                let req = match serde_json::from_value::<JsonRpcRequest>(item) {
                    Ok(r) => r,
                    Err(_) => {
                        out.push(rpc_error(
                            None,
                            -32600,
                            "Invalid Request".into(),
                        ));
                        continue;
                    }
                };
                if req.id.is_none() {
                    // JSON-RPC notification: no response object in batch output
                    let _ = dispatch_jsonrpc_request(&state, &headers, req).await;
                    continue;
                }
                let (_st, resp) = dispatch_jsonrpc_request(&state, &headers, req).await;
                out.push(resp);
            }
            if out.is_empty() {
                // JSON-RPC 2.0: batch of notifications only → no response body
                return StatusCode::NO_CONTENT.into_response();
            }
            (StatusCode::OK, Json(out)).into_response()
        }
        serde_json::Value::Object(_) => {
            let req: JsonRpcRequest = match serde_json::from_value(v) {
                Ok(r) => r,
                Err(e) => {
                    return respond_jsonrpc(
                        "jsonrpc",
                        StatusCode::OK,
                        rpc_error(
                            None,
                            -32600,
                            format!("Invalid Request: {}", e),
                        ),
                    )
                    .into_response();
                }
            };
            if req.id.is_none() {
                let _ = dispatch_jsonrpc_request(&state, &headers, req).await;
                return StatusCode::NO_CONTENT.into_response();
            }
            let method = req.method.clone();
            let (st, resp) = dispatch_jsonrpc_request(&state, &headers, req).await;
            respond_jsonrpc(method.as_str(), st, resp).into_response()
        }
        _ => respond_jsonrpc(
            "jsonrpc",
            StatusCode::OK,
            rpc_error(
                None,
                -32600,
                "Invalid Request: top-level JSON must be an object or array".into(),
            ),
        )
        .into_response(),
    }
}

async fn dispatch_jsonrpc_request(
    state: &RpcState,
    headers: &HeaderMap,
    req: JsonRpcRequest,
) -> (StatusCode, JsonRpcResponse) {
    let node = &state.node;
    let id = req.id;

    let result = match req.method.as_str() {
        "boing_submitTransaction" => {
            let params = req
                .params
                .and_then(|p| serde_json::from_value::<Vec<String>>(p).ok());
            let hex_tx = match params {
                Some(v) if !v.is_empty() => v[0].clone(),
                _ => {
                    return (StatusCode::OK, rpc_error(
                            id,
                            -32602,
                            "Invalid params: expected [hex_signed_tx]".into(),
                        ),
                    )
                }
            };
            match hex::decode(hex_tx.trim_start_matches("0x")) {
                Ok(bytes) => match bincode::deserialize::<SignedTransaction>(&bytes) {
                    Ok(signed) => {
                        let gossip_copy = signed.clone();
                        let n = node.read().await;
                        match n.submit_transaction(signed) {
                            Ok(()) => {
                                let _ = n.p2p.broadcast_signed_transaction(&gossip_copy);
                                info!("RPC: transaction submitted");
                                rpc_ok(id, serde_json::json!({"tx_hash": "ok"}))
                            }
                            Err(MempoolError::QaRejected(r)) => {
                                let mut data = serde_json::json!({ "rule_id": r.rule_id.0, "message": r.message });
                                if let Some(ref u) = r.doc_url {
                                    data["doc_url"] = serde_json::Value::String(u.clone());
                                }
                                rpc_error_with_data(id, -32050, format!("Deployment rejected by QA: {}", r.message), data)
                            }
                            Err(MempoolError::QaPendingPool(tx_hash)) => rpc_error_with_data(
                                id,
                                -32051,
                                "Deployment referred to community QA pool.".into(),
                                serde_json::json!({
                                    "tx_hash": format!("0x{}", hex::encode(tx_hash.0)),
                                }),
                            ),
                            Err(MempoolError::QaPoolEnqueue(msg)) => {
                                rpc_error(id, -32000, format!("QA pool enqueue failed: {}", msg))
                            }
                            Err(MempoolError::QaPoolDisabled) => rpc_error(
                                id,
                                -32054,
                                "QA pool is disabled by governance (configure administrators in qa_pool_config).".into(),
                            ),
                            Err(MempoolError::QaPoolFull) => rpc_error_with_data(
                                id,
                                -32055,
                                "QA pool is at capacity (max_pending_items).".into(),
                                serde_json::json!({ "reason": "pool_full" }),
                            ),
                            Err(MempoolError::QaPoolDeployerCap) => rpc_error_with_data(
                                id,
                                -32056,
                                "QA pool deployer pending limit exceeded.".into(),
                                serde_json::json!({ "reason": "deployer_cap" }),
                            ),
                            Err(e) => rpc_error(id, -32000, format!("{}", e)),
                        }
                    }
                    Err(e) => rpc_error(id, -32602, format!("Invalid transaction: {}", e)),
                },
                Err(e) => rpc_error(id, -32602, format!("Invalid hex: {}", e)),
            }
        }
        "boing_qaPoolList" => {
            let n = node.read().await;
            rpc_ok(
                id,
                serde_json::json!({ "items": n.qa_pool.list_summaries() }),
            )
        }
        "boing_qaPoolConfig" => {
            let n = node.read().await;
            let cfg = n.qa_pool.governance_config();
            let expiry = match cfg.default_on_expiry {
                QaPoolExpiryPolicy::Reject => "reject",
                QaPoolExpiryPolicy::Allow => "allow",
            };
            rpc_ok(
                id,
                serde_json::json!({
                    "max_pending_items": cfg.max_pending_items,
                    "max_pending_per_deployer": cfg.max_pending_per_deployer,
                    "review_window_secs": cfg.review_window_secs,
                    "quorum_fraction": cfg.quorum_fraction,
                    "allow_threshold_fraction": cfg.allow_threshold_fraction,
                    "reject_threshold_fraction": cfg.reject_threshold_fraction,
                    "default_on_expiry": expiry,
                    "dev_open_voting": cfg.dev_open_voting,
                    "administrator_count": cfg.administrator_accounts().len(),
                    "accepts_new_pending": cfg.accepts_new_pending(),
                    "pending_count": n.qa_pool.pending_len(),
                }),
            )
        }
        "boing_getQaRegistry" => {
            let n = node.read().await;
            let reg = n.mempool.qa_registry();
            match serde_json::to_value(reg) {
                Ok(v) => rpc_ok(id, v),
                Err(e) => rpc_error(
                    id,
                    -32000,
                    format!("Failed to serialize QA registry: {}", e),
                ),
            }
        }
        "boing_qaPoolVote" => {
            if !state.operator_authorized(headers) {
                return (StatusCode::OK, rpc_error(
                        id,
                        -32057,
                        "Operator authentication required: set X-Boing-Operator to match the node's BOING_OPERATOR_RPC_TOKEN."
                            .into(),
                    ),
                );
            }
            let params = req
                .params
                .and_then(|p| serde_json::from_value::<Vec<String>>(p).ok());
            let (tx_hex, voter_hex, vote_s) = match params {
                Some(v) if v.len() >= 3 => (v[0].clone(), v[1].clone(), v[2].clone()),
                _ => {
                    return (StatusCode::OK, rpc_error(
                            id,
                            -32602,
                            "Invalid params: expected [tx_hash_hex, voter_hex, allow|reject|abstain]".into(),
                        ),
                    );
                }
            };
            let tx_hash = match parse_hash32_hex(&tx_hex) {
                Ok(h) => h,
                Err(e) => return (StatusCode::OK, rpc_error(id, -32602, e)),
            };
            let voter = match parse_account_id_hex(&voter_hex) {
                Ok(a) => a,
                Err(e) => return (StatusCode::OK, rpc_error(id, -32602, e)),
            };
            let vote = match parse_qa_pool_vote(&vote_s) {
                Ok(v) => v,
                Err(e) => return (StatusCode::OK, rpc_error(id, -32602, e)),
            };
            let n = node.read().await;
            match n.qa_pool_vote(tx_hash, voter, vote) {
                Ok(QaPoolVoteResult::Pending) => {
                    rpc_ok(id, serde_json::json!({ "outcome": "pending" }))
                }
                Ok(QaPoolVoteResult::Rejected) => {
                    rpc_ok(id, serde_json::json!({ "outcome": "reject" }))
                }
                Ok(QaPoolVoteResult::AllowedAdmitted) => rpc_ok(
                    id,
                    serde_json::json!({ "outcome": "allow", "mempool": true }),
                ),
                Ok(QaPoolVoteResult::AllowedAlreadyInMempool) => rpc_ok(
                    id,
                    serde_json::json!({ "outcome": "allow", "mempool": false, "duplicate": true }),
                ),
                Ok(QaPoolVoteResult::AllowedMempoolFailed(msg)) => rpc_ok(
                    id,
                    serde_json::json!({ "outcome": "allow", "mempool": false, "error": msg }),
                ),
                Err(PoolError::NotFound) => rpc_error(
                    id,
                    -32052,
                    "No pending QA pool item for that tx_hash.".into(),
                ),
                Err(PoolError::NotAdministrator) => rpc_error(
                    id,
                    -32053,
                    "Voter is not a governance QA pool administrator.".into(),
                ),
                Err(e) => rpc_error(id, -32000, e.to_string()),
            }
        }
        "boing_operatorApplyQaPolicy" => {
            if !state.operator_authorized(headers) {
                return (StatusCode::OK, rpc_error(
                        id,
                        -32057,
                        "Operator authentication required: set X-Boing-Operator to match the node's BOING_OPERATOR_RPC_TOKEN."
                            .into(),
                    ),
                );
            }
            let params = req
                .params
                .and_then(|p| serde_json::from_value::<Vec<String>>(p).ok());
            let (reg_json, pool_json) = match params {
                Some(v) if v.len() >= 2 => (v[0].clone(), v[1].clone()),
                _ => {
                    return (StatusCode::OK, rpc_error(
                            id,
                            -32602,
                            "Invalid params: expected [qa_registry_json, qa_pool_config_json] (two JSON strings).".into(),
                        ),
                    );
                }
            };
            let registry = match rule_registry_from_json(reg_json.as_bytes()) {
                Ok(r) => r,
                Err(e) => {
                    return (StatusCode::OK, rpc_error(
                            id,
                            -32602,
                            format!("Invalid qa_registry JSON: {}", e),
                        ),
                    );
                }
            };
            let pool_cfg = match qa_pool_config_from_json(pool_json.as_bytes()) {
                Ok(c) => c,
                Err(e) => {
                    return (StatusCode::OK, rpc_error(
                            id,
                            -32602,
                            format!("Invalid qa_pool_config JSON: {}", e),
                        ),
                    );
                }
            };
            let mut n = node.write().await;
            n.set_qa_policy(registry, pool_cfg);
            info!("RPC: operator applied QA policy (registry + pool config)");
            rpc_ok(id, serde_json::json!({ "ok": true }))
        }
        "boing_chainHeight" => {
            let n = node.read().await;
            let height = n.chain.height();
            rpc_ok(id, serde_json::json!(height))
        }
        "boing_health" => {
            let n = node.read().await;
            let chain_id = std::env::var("BOING_CHAIN_ID")
                .ok()
                .and_then(|s| s.trim().parse::<u64>().ok());
            let chain_name = std::env::var("BOING_CHAIN_NAME")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            rpc_ok(
                id,
                serde_json::json!({
                    "ok": true,
                    "client_version": format!("boing-node/{}", env!("CARGO_PKG_VERSION")),
                    "chain_id": chain_id,
                    "chain_name": chain_name,
                    "head_height": n.chain.height(),
                    "rpc_surface": rpc_surface_capabilities(state),
                    "rpc_metrics": rpc_metrics_snapshot(state),
                }),
            )
        }
        "boing_clientVersion" => rpc_ok(
            id,
            serde_json::Value::String(format!("boing-node/{}", env!("CARGO_PKG_VERSION"))),
        ),
        "boing_rpcSupportedMethods" => rpc_ok(
            id,
            serde_json::Value::Array(
                BOING_RPC_SUPPORTED_METHODS
                    .iter()
                    .map(|s| serde_json::Value::String((*s).to_string()))
                    .collect(),
            ),
        ),
        "boing_getSyncState" => {
            let n = node.read().await;
            let head_height = n.chain.height();
            let latest_block_hash = n.chain.latest_hash();
            rpc_ok(
                id,
                serde_json::json!({
                    "head_height": head_height,
                    "finalized_height": head_height,
                    "latest_block_hash": format!("0x{}", hex::encode(latest_block_hash.0)),
                }),
            )
        }
        "boing_getNetworkInfo" => {
            let n = node.read().await;
            let head_height = n.chain.height();
            let latest_block_hash = n.chain.latest_hash();
            let chain_id = std::env::var("BOING_CHAIN_ID")
                .ok()
                .and_then(|s| s.trim().parse::<u64>().ok());
            let chain_name = std::env::var("BOING_CHAIN_NAME")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            let agg = &n.native_aggregates;
            rpc_ok(
                id,
                serde_json::json!({
                    "chain_id": chain_id,
                    "chain_name": chain_name,
                    "head_height": head_height,
                    "finalized_height": head_height,
                    "latest_block_hash": format!("0x{}", hex::encode(latest_block_hash.0)),
                    "target_block_time_secs": BLOCK_TIME_SECS,
                    "client_version": format!("boing-node/{}", env!("CARGO_PKG_VERSION")),
                    "consensus": {
                        "validator_count": n.consensus.num_validators(),
                        "model": "hotstuff_bft"
                    },
                    "native_currency": {
                        "symbol": "BOING",
                        "decimals": 18
                    },
                    "chain_native": {
                        "account_count": agg.account_count,
                        "total_balance": agg.total_balance.to_string(),
                        "total_stake": agg.total_stake.to_string(),
                        "total_native_held": agg.total_native_held.to_string(),
                        "as_of_height": head_height,
                    },
                    "developer": network_info_developer_hints(),
                    "rpc_surface": rpc_surface_capabilities(state),
                    "end_user": end_user_network_hints(),
                    "rpc": {
                        "not_available": ["staking_apy"],
                        "not_available_note": "This JSON-RPC surface does not expose staking APY. Chain-wide sums of committed account balances and stakes are in chain_native (not circulating supply or treasury totals). Per-account balance and stake are available via boing_getAccount. For APY or custom supply definitions, use protocol metrics or indexers."
                    }
                }),
            )
        }
        "boing_getRpcMethodCatalog" => {
            let root = developer_api_document();
            let cat = root
                .get("method_catalog")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            rpc_ok(id, cat)
        }
        "boing_getRpcOpenApi" => {
            let root = developer_api_document();
            let oa = root
                .get("openapi")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            rpc_ok(id, oa)
        }
        "boing_getBalance" => {
            let params = req
                .params
                .and_then(|p| serde_json::from_value::<Vec<String>>(p).ok());
            let hex_account = match params {
                Some(v) if !v.is_empty() => v[0].clone(),
                _ => {
                    return (StatusCode::OK, rpc_error(
                            id,
                            -32602,
                            "Invalid params: expected [hex_account_id]".into(),
                        ),
                    )
                }
            };
            match hex::decode(hex_account.trim_start_matches("0x")) {
                Ok(bytes) if bytes.len() == 32 => {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&bytes);
                    let account_id = boing_primitives::AccountId(arr);
                    let n = node.read().await;
                    let balance = n.state.get(&account_id).map(|s| s.balance).unwrap_or(0);
                    rpc_ok(id, serde_json::json!({ "balance": balance.to_string() }))
                }
                _ => rpc_error(
                    id,
                    -32602,
                    "Invalid account id: expected 32 bytes hex".into(),
                ),
            }
        }
        "boing_getAccount" => {
            let params = req
                .params
                .and_then(|p| serde_json::from_value::<Vec<String>>(p).ok());
            let hex_account = match params {
                Some(v) if !v.is_empty() => v[0].clone(),
                _ => {
                    return (StatusCode::OK, rpc_error(
                            id,
                            -32602,
                            "Invalid params: expected [hex_account_id]".into(),
                        ),
                    )
                }
            };
            match hex::decode(hex_account.trim_start_matches("0x")) {
                Ok(bytes) if bytes.len() == 32 => {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&bytes);
                    let account_id = boing_primitives::AccountId(arr);
                    let n = node.read().await;
                    match n.state.get(&account_id) {
                        Some(s) => rpc_ok(
                            id,
                            serde_json::json!({
                                "balance": s.balance.to_string(),
                                "nonce": s.nonce,
                                "stake": s.stake.to_string()
                            }),
                        ),
                        None => rpc_ok(
                            id,
                            serde_json::json!({
                                "balance": "0",
                                "nonce": 0,
                                "stake": "0"
                            }),
                        ),
                    }
                }
                _ => rpc_error(
                    id,
                    -32602,
                    "Invalid account id: expected 32 bytes hex".into(),
                ),
            }
        }
        "boing_getContractStorage" => {
            let params = req
                .params
                .and_then(|p| serde_json::from_value::<Vec<String>>(p).ok());
            let (hex_contract, hex_key) = match params.as_ref() {
                Some(v) if v.len() >= 2 => (v[0].clone(), v[1].clone()),
                _ => {
                    return (StatusCode::OK, rpc_error(
                            id,
                            -32602,
                            "Invalid params: expected [hex_contract_id, hex_storage_key]".into(),
                        ),
                    );
                }
            };
            let contract_bytes = match hex::decode(hex_contract.trim_start_matches("0x")) {
                Ok(b) if b.len() == 32 => b,
                _ => {
                    return (StatusCode::OK, rpc_error(
                            id,
                            -32602,
                            "Invalid contract id: expected 32 bytes hex".into(),
                        ),
                    )
                }
            };
            let key_bytes = match hex::decode(hex_key.trim_start_matches("0x")) {
                Ok(b) if b.len() == 32 => b,
                _ => {
                    return (StatusCode::OK, rpc_error(
                            id,
                            -32602,
                            "Invalid storage key: expected 32 bytes hex".into(),
                        ),
                    )
                }
            };
            let mut contract_arr = [0u8; 32];
            contract_arr.copy_from_slice(&contract_bytes);
            let mut key_arr = [0u8; 32];
            key_arr.copy_from_slice(&key_bytes);
            let contract_id = boing_primitives::AccountId(contract_arr);
            let n = node.read().await;
            let word = n.state.get_contract_storage(&contract_id, &key_arr);
            rpc_ok(
                id,
                serde_json::json!({
                    "value": format!("0x{}", hex::encode(word)),
                }),
            )
        }
        "boing_getAccountProof" => {
            let params = req
                .params
                .and_then(|p| serde_json::from_value::<Vec<String>>(p).ok());
            let hex_account = match params {
                Some(v) if !v.is_empty() => v[0].clone(),
                _ => {
                    return (StatusCode::OK, rpc_error(
                            id,
                            -32602,
                            "Invalid params: expected [hex_account_id]".into(),
                        ),
                    )
                }
            };
            match hex::decode(hex_account.trim_start_matches("0x")) {
                Ok(bytes) if bytes.len() == 32 => {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&bytes);
                    let account_id = boing_primitives::AccountId(arr);
                    let mut n = node.write().await;
                    if let Some(proof) = n.state.prove_account(&account_id) {
                        match bincode::serialize(&proof) {
                            Ok(ser) => rpc_ok(
                                id,
                                serde_json::json!({
                                    "proof": hex::encode(ser),
                                    "root": hex::encode(proof.root.0),
                                    "value_hash": hex::encode(proof.value_hash.0)
                                }),
                            ),
                            Err(e) => rpc_error(id, -32000, format!("Serialization error: {}", e)),
                        }
                    } else {
                        rpc_error(id, -32000, "Account not found".into())
                    }
                }
                _ => rpc_error(
                    id,
                    -32602,
                    "Invalid account id: expected 32 bytes hex".into(),
                ),
            }
        }
        "boing_verifyAccountProof" => {
            let params = req
                .params
                .and_then(|p| serde_json::from_value::<Vec<String>>(p).ok());
            let (hex_proof, hex_root) = match params {
                Some(v) if v.len() >= 2 => (v[0].clone(), v[1].clone()),
                _ => {
                    return (StatusCode::OK, rpc_error(
                            id,
                            -32602,
                            "Invalid params: expected [hex_proof, hex_state_root]".into(),
                        ),
                    )
                }
            };
            match hex::decode(hex_proof.trim_start_matches("0x")) {
                Ok(bytes) => match bincode::deserialize::<boing_state::MerkleProof>(&bytes) {
                    Ok(proof) => {
                        let root_bytes = hex::decode(hex_root.trim_start_matches("0x")).ok();
                        let expected_root = root_bytes
                            .as_ref()
                            .and_then(|b| boing_primitives::Hash::from_slice(b));
                        let valid = expected_root
                            .map(|r| proof.verify() && proof.root == r)
                            .unwrap_or(proof.verify());
                        rpc_ok(id, serde_json::json!({ "valid": valid }))
                    }
                    Err(e) => rpc_error(id, -32602, format!("Invalid proof: {}", e)),
                },
                Err(e) => rpc_error(id, -32602, format!("Invalid hex: {}", e)),
            }
        }
        "boing_simulateTransaction" => {
            let params = req
                .params
                .and_then(|p| serde_json::from_value::<Vec<String>>(p).ok());
            let hex_tx = match params {
                Some(v) if !v.is_empty() => v[0].clone(),
                _ => {
                    return (StatusCode::OK, rpc_error(
                            id,
                            -32602,
                            "Invalid params: expected [hex_signed_tx]".into(),
                        ),
                    )
                }
            };
            match hex::decode(hex_tx.trim_start_matches("0x")) {
                Ok(bytes) => match bincode::deserialize::<SignedTransaction>(&bytes) {
                    Ok(signed) => {
                        let (mut state_copy, vm, exec_ctx) = {
                            let n = node.read().await;
                            let h = n.chain.height();
                            let ts = n
                                .chain
                                .get_block_by_height(h)
                                .map(|b| b.header.timestamp)
                                .unwrap_or(0);
                            (
                                n.state.snapshot(),
                                boing_execution::Vm::with_qa_registry(
                                    n.mempool.qa_registry().clone(),
                                ),
                                boing_execution::VmExecutionContext {
                                    block_height: h,
                                    block_timestamp: ts,
                                },
                            )
                        };
                        let sug = signed.tx.suggested_parallel_access_list();
                        let covers = signed.tx.access_list_covers_parallel_suggestion();
                        match vm.execute_with_context(&signed.tx, &mut state_copy, exec_ctx) {
                            Ok(out) => rpc_ok(
                                id,
                                serde_json::json!({
                                    "gas_used": out.gas_used,
                                    "success": true,
                                    "return_data": format!("0x{}", hex::encode(&out.return_data)),
                                    "logs": execution_logs_to_json(&out.logs),
                                    "suggested_access_list": access_list_to_json(&sug),
                                    "access_list_covers_suggestion": covers,
                                }),
                            ),
                            Err(e) => rpc_ok(
                                id,
                                serde_json::json!({
                                    "gas_used": 0,
                                    "success": false,
                                    "error": format!("{}", e),
                                    "return_data": "0x",
                                    "logs": serde_json::json!([]),
                                    "suggested_access_list": access_list_to_json(&sug),
                                    "access_list_covers_suggestion": covers,
                                }),
                            ),
                        }
                    }
                    Err(e) => rpc_error(id, -32602, format!("Invalid transaction: {}", e)),
                },
                Err(e) => rpc_error(id, -32602, format!("Invalid hex: {}", e)),
            }
        }
        "boing_getBlockByHeight" => {
            let params = req
                .params
                .and_then(|p| serde_json::from_value::<Vec<serde_json::Value>>(p).ok());
            let height = match params.as_ref() {
                Some(v) if !v.is_empty() => v[0].as_u64(),
                _ => {
                    return (StatusCode::OK, rpc_error(
                            id,
                            -32602,
                            "Invalid params: expected [height] or [height, include_receipts]"
                                .into(),
                        ),
                    );
                }
            };
            let height = match height {
                Some(h) => h,
                None => {
                    return (StatusCode::OK, rpc_error(id, -32602, "Invalid height: expected u64".into()),
                    );
                }
            };
            let include_receipts = params
                .as_ref()
                .and_then(|v| v.get(1))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let n = node.read().await;
            match n.chain.get_block_by_height(height) {
                Some(block) => {
                    let hash = block.hash();
                    let block_json =
                        serde_json::to_value(&block).unwrap_or(serde_json::Value::Null);
                    let mut obj = block_json.as_object().cloned().unwrap_or_default();
                    obj.insert("hash".to_string(), serde_json::json!(hex::encode(hash.0)));
                    if include_receipts {
                        let arr: Vec<serde_json::Value> = block
                            .transactions
                            .iter()
                            .map(|tx| {
                                let tid = tx.id();
                                n.receipts
                                    .get(&tid)
                                    .map(execution_receipt_to_json)
                                    .unwrap_or(serde_json::Value::Null)
                            })
                            .collect();
                        obj.insert("receipts".to_string(), serde_json::Value::Array(arr));
                    }
                    rpc_ok(id, serde_json::Value::Object(obj))
                }
                None => rpc_ok(id, serde_json::Value::Null),
            }
        }
        "boing_getTransactionReceipt" => {
            let params = req
                .params
                .and_then(|p| serde_json::from_value::<Vec<String>>(p).ok());
            let hex_id = match params {
                Some(v) if !v.is_empty() => v[0].clone(),
                _ => {
                    return (StatusCode::OK, rpc_error(
                            id,
                            -32602,
                            "Invalid params: expected [hex_tx_id]".into(),
                        ),
                    );
                }
            };
            match parse_hash32_hex(&hex_id) {
                Ok(tx_id) => {
                    let n = node.read().await;
                    match n.receipts.get(&tx_id) {
                        Some(r) => rpc_ok(id, execution_receipt_to_json(r)),
                        None => rpc_ok(id, serde_json::Value::Null),
                    }
                }
                Err(e) => rpc_error(id, -32602, e),
            }
        }
        "boing_getLogs" => {
            let params = req
                .params
                .and_then(|p| serde_json::from_value::<Vec<serde_json::Value>>(p).ok());
            let filter_val = match params.as_ref().and_then(|v| v.first()) {
                Some(v) => v.clone(),
                None => {
                    return (StatusCode::OK, rpc_error(
                            id,
                            -32602,
                            "Invalid params: expected [filter_object] with fromBlock and toBlock"
                                .into(),
                        ),
                    );
                }
            };
            let filter: GetLogsFilterParams = match serde_json::from_value(filter_val) {
                Ok(f) => f,
                Err(e) => {
                    return (StatusCode::OK, rpc_error(
                            id,
                            -32602,
                            format!("Invalid getLogs filter: {}", e),
                        ),
                    );
                }
            };
            let from_block = match json_block_number(&filter.from_block) {
                Ok(h) => h,
                Err(e) => return (StatusCode::OK, rpc_error(id, -32602, e)),
            };
            let to_block = match json_block_number(&filter.to_block) {
                Ok(h) => h,
                Err(e) => return (StatusCode::OK, rpc_error(id, -32602, e)),
            };
            if to_block < from_block {
                return (StatusCode::OK, rpc_error(id, -32602, "toBlock must be >= fromBlock".into()),
                );
            }
            let span = to_block.saturating_sub(from_block).saturating_add(1);
            if span > GET_LOGS_MAX_BLOCK_RANGE {
                return (StatusCode::OK, rpc_error(
                        id,
                        -32602,
                        format!(
                            "block range too large (max {} inclusive blocks)",
                            GET_LOGS_MAX_BLOCK_RANGE
                        ),
                    ),
                );
            }
            let addr_filter = match filter.address.as_deref() {
                None | Some("") => None,
                Some(s) => match parse_account_id_hex(s) {
                    Ok(a) => Some(a),
                    Err(e) => return (StatusCode::OK, rpc_error(id, -32602, e)),
                },
            };
            let topic_filter = match parse_topics_filter(filter.topics) {
                Ok(t) => t,
                Err(e) => return (StatusCode::OK, rpc_error(id, -32602, e)),
            };

            let n = node.read().await;
            let mut out: Vec<serde_json::Value> = Vec::new();
            for height in from_block..=to_block {
                let Some(block) = n.chain.get_block_by_height(height) else {
                    continue;
                };
                for tx in block.transactions.iter() {
                    let tid = tx.id();
                    let Some(receipt) = n.receipts.get(&tid) else {
                        continue;
                    };
                    if receipt.logs.is_empty() {
                        continue;
                    }
                    let emit_addr = log_emitting_account(tx);
                    if let Some(want) = addr_filter {
                        if emit_addr != Some(want) {
                            continue;
                        }
                    }
                    for (log_index, log) in receipt.logs.iter().enumerate() {
                        if !log_matches_topic_filter(log, &topic_filter) {
                            continue;
                        }
                        if out.len() >= GET_LOGS_MAX_RESULTS {
                            return (StatusCode::OK, rpc_error(
                                    id,
                                    -32603,
                                    format!(
                                        "log result limit exceeded (max {}); narrow filters or block range",
                                        GET_LOGS_MAX_RESULTS
                                    ),
                                ),
                            );
                        }
                        let addr_hex = emit_addr.map(|a| format!("0x{}", hex::encode(a.0)));
                        out.push(serde_json::json!({
                            "block_height": receipt.block_height,
                            "tx_index": receipt.tx_index,
                            "tx_id": format!("0x{}", hex::encode(receipt.tx_id.0)),
                            "log_index": log_index as u32,
                            "address": addr_hex,
                            "topics": log.topics.iter().map(|t| format!("0x{}", hex::encode(t.as_slice()))).collect::<Vec<_>>(),
                            "data": format!("0x{}", hex::encode(&log.data)),
                        }));
                    }
                }
            }
            rpc_ok(id, serde_json::Value::Array(out))
        }
        "boing_getBlockByHash" => {
            let params = req
                .params
                .and_then(|p| serde_json::from_value::<Vec<serde_json::Value>>(p).ok());
            let hex_hash = match params.as_ref() {
                Some(v) if !v.is_empty() => match v[0].as_str() {
                    Some(s) => s.to_string(),
                    None => {
                        return (StatusCode::OK, rpc_error(
                                id,
                                -32602,
                                "Invalid params: expected [hex_block_hash] or [hex_block_hash, include_receipts]"
                                    .into(),
                            ),
                        );
                    }
                },
                _ => {
                    return (StatusCode::OK, rpc_error(
                            id,
                            -32602,
                            "Invalid params: expected [hex_block_hash] or [hex_block_hash, include_receipts]"
                                .into(),
                        ),
                    );
                }
            };
            let include_receipts = params
                .as_ref()
                .and_then(|v| v.get(1))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let bytes = match hex::decode(hex_hash.trim_start_matches("0x")) {
                Ok(b) if b.len() == 32 => b,
                Ok(_) => {
                    return (StatusCode::OK, rpc_error(id, -32602, "Hash must be 32 bytes".into()),
                    );
                }
                Err(e) => {
                    return (StatusCode::OK, rpc_error(id, -32602, format!("Invalid hex: {}", e)),
                    );
                }
            };
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&bytes);
            let hash = boing_primitives::Hash(arr);
            let n = node.read().await;
            match n.chain.get_block_by_hash(&hash) {
                Some(block) => {
                    let block_json =
                        serde_json::to_value(&block).unwrap_or(serde_json::Value::Null);
                    let mut obj = block_json.as_object().cloned().unwrap_or_default();
                    obj.insert(
                        "hash".to_string(),
                        serde_json::json!(hex::encode(block.hash().0)),
                    );
                    if include_receipts {
                        let arr_r: Vec<serde_json::Value> = block
                            .transactions
                            .iter()
                            .map(|tx| {
                                let tid = tx.id();
                                n.receipts
                                    .get(&tid)
                                    .map(execution_receipt_to_json)
                                    .unwrap_or(serde_json::Value::Null)
                            })
                            .collect();
                        obj.insert("receipts".to_string(), serde_json::Value::Array(arr_r));
                    }
                    rpc_ok(id, serde_json::Value::Object(obj))
                }
                None => rpc_ok(id, serde_json::Value::Null),
            }
        }
        "boing_registerDappMetrics" => {
            let params = req
                .params
                .and_then(|p| serde_json::from_value::<Vec<String>>(p).ok());
            let (hex_contract, hex_owner) = match params {
                Some(v) if v.len() >= 2 => (v[0].clone(), v[1].clone()),
                _ => {
                    return (StatusCode::OK, rpc_error(
                            id,
                            -32602,
                            "Invalid params: expected [hex_contract, hex_owner]".into(),
                        ),
                    )
                }
            };
            let parse_account = |hex_s: &str| -> Result<boing_primitives::AccountId, String> {
                let bytes = hex::decode(hex_s.trim_start_matches("0x"))
                    .map_err(|e| format!("Invalid hex: {}", e))?;
                if bytes.len() != 32 {
                    return Err("Account ID must be 32 bytes".into());
                }
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&bytes);
                Ok(boing_primitives::AccountId(arr))
            };
            match (parse_account(&hex_contract), parse_account(&hex_owner)) {
                (Ok(contract), Ok(owner)) => {
                    let n = node.write().await;
                    n.dapp_registry.register(contract, owner);
                    info!(
                        "RPC: dApp registered contract={} owner={}",
                        hex::encode(contract.0),
                        hex::encode(owner.0)
                    );
                    rpc_ok(
                        id,
                        serde_json::json!({"registered": true, "contract": hex::encode(contract.0), "owner": hex::encode(owner.0)}),
                    )
                }
                (Err(e), _) | (_, Err(e)) => rpc_error(id, -32602, e),
            }
        }
        "boing_qaCheck" => {
            let params = req
                .params
                .and_then(|p| serde_json::from_value::<Vec<serde_json::Value>>(p).ok());
            let hex_bytecode = match params.as_ref().and_then(|v| v.first()) {
                Some(serde_json::Value::String(s)) => s.clone(),
                _ => return (StatusCode::OK, rpc_error(id, -32602, "Invalid params: expected [hex_bytecode] or [hex_bytecode, purpose_category?, description_hash?, asset_name?, asset_symbol?]".into())),
            };
            let bytecode = match hex::decode(hex_bytecode.trim_start_matches("0x")) {
                Ok(b) => b,
                Err(e) => {
                    return (StatusCode::OK, rpc_error(
                            id,
                            -32602,
                            format!("Invalid hex bytecode: {}", e),
                        ),
                    )
                }
            };
            let purpose = params
                .as_ref()
                .and_then(|v| v.get(1))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let desc_hash = params
                .as_ref()
                .and_then(|v| v.get(2))
                .and_then(|v| v.as_str())
                .and_then(|s| hex::decode(s.trim_start_matches("0x")).ok())
                .filter(|b| b.len() == 32);
            let asset_name = params
                .as_ref()
                .and_then(|v| v.get(3))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let asset_symbol = params
                .as_ref()
                .and_then(|v| v.get(4))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let registry = RuleRegistry::new();
            let result = check_contract_deploy_full_with_metadata(
                &bytecode,
                purpose.as_deref(),
                desc_hash.as_deref(),
                asset_name.as_deref(),
                asset_symbol.as_deref(),
                &registry,
            );
            let (result_str, rule_id, message, doc_url) = match result {
                QaResult::Allow => ("allow".to_string(), None, None, None),
                QaResult::Reject(r) => (
                    "reject".to_string(),
                    Some(r.rule_id.0),
                    Some(r.message),
                    r.doc_url,
                ),
                QaResult::Unsure => (
                    "unsure".to_string(),
                    None,
                    Some("Deployment referred to community QA pool".into()),
                    None,
                ),
            };
            let mut obj = serde_json::Map::new();
            obj.insert("result".to_string(), serde_json::Value::String(result_str));
            if let Some(rid) = rule_id {
                obj.insert("rule_id".to_string(), serde_json::Value::String(rid));
            }
            if let Some(msg) = message {
                obj.insert("message".to_string(), serde_json::Value::String(msg));
            }
            if let Some(u) = doc_url {
                obj.insert("doc_url".to_string(), serde_json::Value::String(u));
            }
            rpc_ok(id, serde_json::Value::Object(obj))
        }
        "boing_submitIntent" => {
            let params = req
                .params
                .and_then(|p| serde_json::from_value::<Vec<String>>(p).ok());
            let hex_intent = match params {
                Some(v) if !v.is_empty() => v[0].clone(),
                _ => {
                    return (StatusCode::OK, rpc_error(
                            id,
                            -32602,
                            "Invalid params: expected [hex_signed_intent]".into(),
                        ),
                    )
                }
            };
            match hex::decode(hex_intent.trim_start_matches("0x")) {
                Ok(bytes) => match bincode::deserialize::<SignedIntent>(&bytes) {
                    Ok(signed) => {
                        let n = node.read().await;
                        match n.submit_intent(signed) {
                            Ok(intent_id) => {
                                info!("RPC: intent submitted");
                                rpc_ok(
                                    id,
                                    serde_json::json!({"intent_id": hex::encode(intent_id.0)}),
                                )
                            }
                            Err(e) => rpc_error(id, -32000, format!("{}", e)),
                        }
                    }
                    Err(e) => rpc_error(id, -32602, format!("Invalid intent: {}", e)),
                },
                Err(e) => rpc_error(id, -32602, format!("Invalid hex: {}", e)),
            }
        }
        "boing_faucetRequest" => {
            let Some(ref faucet_signer) = state.faucet_signer else {
                return (StatusCode::OK, rpc_error(
                        id,
                        -32601,
                        "Faucet not enabled on this node.".into(),
                    ),
                );
            };
            let Some(ref cooldown) = state.faucet_cooldown else {
                return (StatusCode::OK, rpc_error(
                        id,
                        -32601,
                        "Faucet not enabled on this node.".into(),
                    ),
                );
            };
            let Some(ref submit_lock) = state.faucet_submit_lock else {
                return (StatusCode::OK, rpc_error(
                        id,
                        -32601,
                        "Faucet not enabled on this node.".into(),
                    ),
                );
            };
            let params = req
                .params
                .and_then(|p| serde_json::from_value::<Vec<String>>(p).ok());
            let hex_account = match params {
                Some(v) if !v.is_empty() => v[0].clone(),
                _ => {
                    return (StatusCode::OK, rpc_error(
                            id,
                            -32602,
                            "Invalid params: expected [hex_account_id] (32 bytes hex)".into(),
                        ),
                    )
                }
            };
            let to_bytes = match hex::decode(hex_account.trim_start_matches("0x")) {
                Ok(b) if b.len() == 32 => b,
                Ok(_) => {
                    return (StatusCode::OK, rpc_error(
                            id,
                            -32602,
                            "Account ID must be 32 bytes hex.".into(),
                        ),
                    )
                }
                Err(e) => {
                    return (StatusCode::OK, rpc_error(id, -32602, format!("Invalid hex: {}", e)),
                    );
                }
            };
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&to_bytes);
            let to_id = AccountId(arr);

            const COOLDOWN: Duration = Duration::from_secs(60);
            {
                let map = cooldown.lock().unwrap();
                if let Some(&last) = map.get(&to_id) {
                    if last.elapsed() < COOLDOWN {
                        return (StatusCode::OK, rpc_error(
                                id,
                                -32016,
                                format!(
                                    "Faucet cooldown: try again in {} seconds.",
                                    (COOLDOWN.as_secs()).saturating_sub(last.elapsed().as_secs())
                                ),
                            ),
                        );
                    }
                }
            }

            let faucet_id = testnet_faucet_account_id();
            let _faucet_gate = submit_lock.lock().await;

            let n = node.write().await;
            let (chain_nonce, balance_ok) = match n.state.get(&faucet_id) {
                Some(s) => (s.nonce, s.balance >= faucet::FAUCET_DISPENSE_AMOUNT),
                None => {
                    return (StatusCode::OK, rpc_error(
                            id,
                            -32000,
                            "Faucet account not initialized.".into(),
                        ),
                    );
                }
            };
            if !balance_ok {
                return (StatusCode::OK, rpc_error(id, -32000, "Faucet balance too low.".into()),
                );
            }
            let nonce = n
                .mempool
                .suggested_next_nonce(faucet_id, chain_nonce);
            let tx = Transaction {
                nonce,
                sender: faucet_id,
                payload: TransactionPayload::Transfer {
                    to: to_id,
                    amount: faucet::FAUCET_DISPENSE_AMOUNT,
                },
                access_list: AccessList::new(vec![faucet_id, to_id], vec![faucet_id, to_id]),
            };
            let signed = SignedTransaction::new(tx, faucet_signer.as_ref());
            let gossip_copy = signed.clone();
            drop(n);
            let n = node.write().await;
            match n.submit_transaction(signed) {
                Ok(()) => {
                    let _ = n.p2p.broadcast_signed_transaction(&gossip_copy);
                    if let Ok(mut map) = cooldown.lock() {
                        map.insert(to_id, Instant::now());
                    }
                    info!(
                        "RPC: faucet sent {} to {}",
                        faucet::FAUCET_DISPENSE_AMOUNT,
                        hex::encode(to_id.0)
                    );
                    rpc_ok(
                        id,
                        serde_json::json!({
                            "ok": true,
                            "amount": faucet::FAUCET_DISPENSE_AMOUNT,
                            "to": hex::encode(to_id.0),
                            "message": "Check your wallet; tx is in the mempool."
                        }),
                    )
                }
                Err(e) => rpc_error(id, -32000, format!("Faucet submit failed: {}", e)),
            }
        }
        _ => {
            state
                .rpc_metrics
                .method_not_found
                .fetch_add(1, Ordering::Relaxed);
            rpc_error_with_data(
                id,
                -32601,
                format!("Method not found: {}", req.method),
                serde_json::json!({ "method": req.method }),
            )
        }
    };

    (StatusCode::OK, result)
}

/// Per-request span for the HTTP RPC stack (after [`SetRequestIdLayer`]); includes **`x-request-id`** for log correlation.
fn rpc_http_make_span<B>(req: &HttpRequest<B>) -> tracing::Span {
    let request_id = req
        .extensions()
        .get::<RequestId>()
        .and_then(|id| id.header_value().to_str().ok())
        .filter(|s| !s.is_empty())
        .unwrap_or("-");
    tracing::info_span!(
        "rpc_http",
        method = %req.method(),
        path = %req.uri().path(),
        request_id = %request_id,
    )
}

fn rpc_http_trace_layer() -> TraceLayer<
    tower_http::classify::SharedClassifier<tower_http::classify::ServerErrorsAsFailures>,
    fn(&HttpRequest<axum::body::Body>) -> tracing::Span,
    DefaultOnRequest,
    DefaultOnResponse,
    tower_http::trace::DefaultOnBodyChunk,
    tower_http::trace::DefaultOnEos,
    DefaultOnFailure,
> {
    TraceLayer::new_for_http()
        .make_span_with(rpc_http_make_span as fn(&HttpRequest<axum::body::Body>) -> tracing::Span)
        .on_request(DefaultOnRequest::new().level(Level::DEBUG))
        .on_response(
            DefaultOnResponse::new()
                .level(Level::DEBUG)
                .latency_unit(LatencyUnit::Millis),
        )
        .on_failure(DefaultOnFailure::new().level(Level::WARN))
}

fn build_rpc_cors_layer() -> CorsLayer {
    let mut origins = vec![
        HeaderValue::from_static("https://boing.observer"),
        HeaderValue::from_static("https://boing.express"),
        HeaderValue::from_static("https://boing.network"),
        HeaderValue::from_static("https://www.boing.network"),
        HeaderValue::from_static("https://boing.finance"),
        HeaderValue::from_static("https://www.boing.finance"),
        HeaderValue::from_static("https://bootnode2.boing.network"),
        HeaderValue::from_static("https://testnet-rpc-2.boing.network"),
        HeaderValue::from_static("http://localhost:3000"),
        HeaderValue::from_static("http://localhost:4321"),
        HeaderValue::from_static("http://127.0.0.1:3000"),
        HeaderValue::from_static("http://127.0.0.1:4321"),
        HeaderValue::from_static("http://localhost:5173"),
        HeaderValue::from_static("http://127.0.0.1:5173"),
        HeaderValue::from_static("http://localhost:5174"),
        HeaderValue::from_static("http://127.0.0.1:5174"),
        HeaderValue::from_static("http://localhost:4173"),
        HeaderValue::from_static("http://127.0.0.1:4173"),
        HeaderValue::from_static("http://localhost:8080"),
        HeaderValue::from_static("http://127.0.0.1:8080"),
    ];
    let mut cors_extra_merged: usize = 0;
    if let Ok(raw) = std::env::var("BOING_RPC_CORS_EXTRA_ORIGINS") {
        for part in raw.split(',') {
            let t = part.trim();
            if t.is_empty() {
                continue;
            }
            match HeaderValue::from_str(t) {
                Ok(v) => {
                    origins.push(v);
                    cors_extra_merged = cors_extra_merged.saturating_add(1);
                }
                Err(_) => tracing::warn!(
                    origin = %t,
                    "BOING_RPC_CORS_EXTRA_ORIGINS: skipped invalid origin"
                ),
            }
        }
    }
    if cors_extra_merged > 0 {
        tracing::info!(
            merged = cors_extra_merged,
            "BOING_RPC_CORS_EXTRA_ORIGINS: merged extra browser origins for JSON-RPC POST / and WebSocket GET /ws"
        );
    }
    CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([Method::GET, Method::HEAD, Method::POST, Method::OPTIONS])
        .allow_headers([
            CONTENT_TYPE,
            HeaderName::from_static("x-boing-operator"),
            HeaderName::from_static("x-request-id"),
            HeaderName::from_static("sec-websocket-protocol"),
            HeaderName::from_static("sec-websocket-version"),
            HeaderName::from_static("sec-websocket-key"),
            HeaderName::from_static("upgrade"),
            HeaderName::from_static("connection"),
        ])
        .expose_headers([
            HeaderName::from_static("x-boing-rpc-version"),
            HeaderName::from_static("x-request-id"),
            HeaderName::from_static("retry-after"),
        ])
}

/// Build the RPC router.
/// When `rate_limit` has `requests_per_sec > 0`, applies global rate limiting.
/// When `faucet_signer` is Some, enables boing_faucetRequest (testnet only).
/// When `head_broadcast` is Some, WebSocket **`GET /ws`** streams **newHeads** (see `network_info_developer_hints`).
pub fn rpc_router(
    node: NodeState,
    rate_limit: &RateLimitConfig,
    faucet_signer: Option<Arc<ed25519_dalek::SigningKey>>,
    operator_rpc_token: Option<Arc<str>>,
    head_broadcast: Option<broadcast::Sender<serde_json::Value>>,
) -> Router {
    let rate_limiter = if rate_limit.requests_per_sec > 0 {
        let rps = NonZeroU32::new(rate_limit.requests_per_sec.max(1))
            .unwrap_or_else(|| NonZeroU32::new(1).unwrap());
        let burst =
            NonZeroU32::new(rate_limit.requests_per_sec.saturating_mul(2).max(10)).unwrap_or(rps);
        let quota = Quota::per_second(rps).allow_burst(burst);
        Some(Arc::new(RateLimiter::direct(quota)))
    } else {
        None
    };

    let (faucet_signer, faucet_cooldown, faucet_submit_lock) = if faucet_signer.is_some() {
        (
            faucet_signer,
            Some(Arc::new(std::sync::Mutex::new(HashMap::new()))),
            Some(Arc::new(TokioMutex::new(()))),
        )
    } else {
        (None, None, None)
    };

    let cors = build_rpc_cors_layer();

    let rpc_version_hdr = HeaderValue::try_from(format!("boing-node/{}", env!("CARGO_PKG_VERSION")))
        .expect("CARGO_PKG_VERSION produces valid header value");

    let ws_max_connections = max_ws_connections();
    let ws_active = Arc::new(AtomicUsize::new(0));

    let rate_limit_requests_per_sec = rate_limit.requests_per_sec;
    let rpc_metrics = Arc::new(RpcHttpMetrics::default());

    Router::new()
        .route(
            "/",
            get(http_root_get)
                .head(http_head_root)
                .post(handle_rpc)
                .options(http_options_root),
        )
        .route("/openapi.json", get(http_openapi_json))
        .route("/.well-known/boing-rpc", get(http_well_known_boing_rpc))
        .route("/live", get(http_live).head(http_head_live))
        .route("/live.json", get(http_live).head(http_head_live))
        .route("/ready", get(http_ready).head(http_head_ready))
        .route("/ready.json", get(http_ready).head(http_head_ready))
        .route(
            "/ws",
            get(crate::rpc_ws::ws_new_heads_upgrade),
        )
        .layer(
            ServiceBuilder::new()
                .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
                .layer(rpc_http_trace_layer())
                .layer(PropagateRequestIdLayer::x_request_id()),
        )
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-boing-rpc-version"),
            rpc_version_hdr,
        ))
        // Large signed-tx hex payloads (e.g. contract deploy); override with BOING_RPC_MAX_BODY_MB.
        .layer(DefaultBodyLimit::max(max_rpc_body_bytes()))
        .layer(cors)
        .layer(middleware::from_fn(map_payload_too_large_to_json))
        .with_state(RpcState {
            node,
            rate_limiter,
            faucet_signer,
            faucet_cooldown,
            faucet_submit_lock,
            operator_rpc_token,
            head_broadcast,
            ws_active,
            ws_max_connections,
            rate_limit_requests_per_sec,
            rpc_metrics,
        })
}
