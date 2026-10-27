//! JSON-RPC helpers for **`boing_listDexPools`**, **`boing_listDexTokens`**, **`boing_getDexToken`**.
//!
//! Discovery is **DEX-derived**: registered pairs from the native pair directory (env
//! **`BOING_CANONICAL_NATIVE_DEX_FACTORY`** or per-request **`factory`**) plus live pool storage reads.
//! Optional receipt / block scans populate **heights** and **deploy metadata** (see
//! `docs/HANDOFF_Boing_Network_Global_Token_Discovery.md`).

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use boing_execution::{
    native_dex_factory_count_key, native_dex_factory_triplet_storage_key,
    reference_fungible_secured_runtime_bytecode, reference_fungible_secured_runtime_bytecode_v2,
    reference_fungible_template_bytecode, reference_fungible_template_v2_bytecode, reserve_a_key,
    reserve_b_key, swap_fee_bps_key, token_a_key, token_b_key, NATIVE_CP_SWAP_FEE_BPS,
    NATIVE_DEX_FACTORY_MAX_PAIRS, NATIVE_DEX_FACTORY_TOPIC_REGISTER,
};
use boing_primitives::{
    contract_deploy_init_body, contract_deploy_uses_init_code, create2_contract_address,
    nonce_derived_contract_address, AccountId, ExecutionLog, ExecutionReceipt, Hash, Transaction,
    TransactionPayload,
};
use boing_state::StateStore;
use serde_json::{json, Value};

use crate::chain::ChainState;

const DEFAULT_PAGE_LIMIT: usize = 100;
const MAX_PAGE_LIMIT: usize = 500;
const DEFAULT_METADATA_SCAN_BLOCKS: u64 = 8192;
const MAX_METADATA_SCAN_BLOCKS: u64 = 500_000;
/// Default cap on receipt rows examined per discovery call (sorted ascending by block, then tx index).
const DEFAULT_MAX_RECEIPT_SCANS: usize = 500_000;

pub type DexDiscoveryRpcError = (i32, String);

fn err_bad(msg: impl Into<String>) -> DexDiscoveryRpcError {
    (-32602, msg.into())
}

fn err_config(msg: impl Into<String>) -> DexDiscoveryRpcError {
    (-32000, msg.into())
}

fn parse_account_id_hex(s: &str) -> Result<AccountId, DexDiscoveryRpcError> {
    let bytes = hex::decode(s.trim().trim_start_matches("0x"))
        .map_err(|_| err_bad("invalid account hex".to_string()))?;
    if bytes.len() != 32 {
        return Err(err_bad("account id must be 32 bytes"));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(AccountId(arr))
}

fn account_id_hex_lower(id: &AccountId) -> String {
    format!("0x{}", hex::encode(id.0))
}

/// Low **8** bytes BE (factory `pairs_count` layout).
fn word_low_u64_be(word: &[u8; 32]) -> u64 {
    u64::from_be_bytes(word[24..32].try_into().expect("len"))
}

/// Reference-style **u128** in the low **16** bytes (BE).
fn word_low_u128_be(word: &[u8; 32]) -> u128 {
    u128::from_be_bytes(word[16..32].try_into().expect("len"))
}

fn factory_pair_count(state: &StateStore, factory: &AccountId) -> u64 {
    let w = state.get_contract_storage(factory, &native_dex_factory_count_key());
    word_low_u64_be(&w).min(NATIVE_DEX_FACTORY_MAX_PAIRS)
}

fn read_triplet(
    state: &StateStore,
    factory: &AccountId,
    index: u64,
) -> (AccountId, AccountId, AccountId) {
    let ka = native_dex_factory_triplet_storage_key(index, 0);
    let kb = native_dex_factory_triplet_storage_key(index, 1);
    let kp = native_dex_factory_triplet_storage_key(index, 2);
    (
        AccountId(state.get_contract_storage(factory, &ka)),
        AccountId(state.get_contract_storage(factory, &kb)),
        AccountId(state.get_contract_storage(factory, &kp)),
    )
}

fn pool_fee_bps(state: &StateStore, pool: &AccountId) -> u32 {
    let w = state.get_contract_storage(pool, &swap_fee_bps_key());
    let raw = word_low_u128_be(&w);
    if raw == 0 {
        u32::from(NATIVE_CP_SWAP_FEE_BPS)
    } else {
        u32::try_from(raw.min(10_000)).unwrap_or(u32::from(NATIVE_CP_SWAP_FEE_BPS))
    }
}

fn pool_reserves(state: &StateStore, pool: &AccountId) -> (u128, u128) {
    let ra = state.get_contract_storage(pool, &reserve_a_key());
    let rb = state.get_contract_storage(pool, &reserve_b_key());
    (word_low_u128_be(&ra), word_low_u128_be(&rb))
}

fn pool_token_ids(state: &StateStore, pool: &AccountId) -> (AccountId, AccountId) {
    let ta = AccountId(state.get_contract_storage(pool, &token_a_key()));
    let tb = AccountId(state.get_contract_storage(pool, &token_b_key()));
    (ta, tb)
}

fn dex_object_params(
    params: &Option<Value>,
) -> Result<&serde_json::Map<String, Value>, DexDiscoveryRpcError> {
    match params {
        Some(Value::Array(a)) => match a.first() {
            Some(Value::Object(m)) => Ok(m),
            _ => Err(err_bad("params: expected [object] with cursor/limit")),
        },
        Some(Value::Object(m)) => Ok(m),
        _ => Err(err_bad("params: expected object or [object]")),
    }
}

fn param_limit(m: &serde_json::Map<String, Value>) -> usize {
    let raw = m
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(DEFAULT_PAGE_LIMIT as u64) as usize;
    raw.clamp(1, MAX_PAGE_LIMIT)
}

fn param_light(m: &serde_json::Map<String, Value>) -> bool {
    m.get("light")
        .and_then(|v| v.as_bool())
        .or_else(|| m.get("enrich").and_then(|v| v.as_bool()).map(|b| !b))
        .unwrap_or(false)
}

fn param_include_diagnostics(m: &serde_json::Map<String, Value>) -> bool {
    m.get("includeDiagnostics")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

fn metadata_scan_blocks_limit() -> u64 {
    match std::env::var("BOING_DEX_TOKEN_METADATA_SCAN_BLOCKS") {
        Ok(s) => match s.trim().parse::<u64>() {
            Ok(0) => DEFAULT_METADATA_SCAN_BLOCKS,
            Ok(n) => n.min(MAX_METADATA_SCAN_BLOCKS),
            Err(_) => DEFAULT_METADATA_SCAN_BLOCKS,
        },
        Err(_) => DEFAULT_METADATA_SCAN_BLOCKS,
    }
}

/// `None` = unlimited receipt scan (operator override via **`BOING_DEX_DISCOVERY_MAX_RECEIPT_SCANS=0`**).
fn max_receipt_scans_limit() -> Option<usize> {
    match std::env::var("BOING_DEX_DISCOVERY_MAX_RECEIPT_SCANS") {
        Ok(s) => match s.trim().parse::<usize>() {
            Ok(0) => None,
            Ok(n) => Some(n),
            Err(_) => Some(DEFAULT_MAX_RECEIPT_SCANS),
        },
        Err(_) => Some(DEFAULT_MAX_RECEIPT_SCANS),
    }
}

/// Optional JSON object: **`"0x" + 64 hex` → decimals** (0–255), the operator override for
/// **`boing_listDexPools`** leg fields, **`boing_listDexTokens`** and **`boing_getDexToken`**.
fn parse_token_decimals_json() -> HashMap<String, u8> {
    match std::env::var("BOING_DEX_TOKEN_DECIMALS_JSON") {
        Ok(raw) => parse_token_decimals_map(&raw),
        Err(_) => HashMap::new(),
    }
}

/// Parse the decimals override map. Entries with a bad id or a non-integer / out-of-range value are
/// skipped (never coerced to a guessed default).
fn parse_token_decimals_map(raw: &str) -> HashMap<String, u8> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return HashMap::new();
    }
    let Ok(v) = serde_json::from_str::<Value>(trimmed) else {
        return HashMap::new();
    };
    let Some(obj) = v.as_object() else {
        return HashMap::new();
    };
    let mut m = HashMap::with_capacity(obj.len());
    for (k, val) in obj {
        let Ok(id) = parse_account_id_hex(k) else {
            continue;
        };
        let n = match val {
            Value::Number(num) => num.as_u64(),
            Value::String(s) => s.trim().parse::<u64>().ok(),
            _ => None,
        };
        let Some(dec) = n.and_then(|n| u8::try_from(n).ok()) else {
            continue;
        };
        m.insert(account_id_hex_lower(&id), dec);
    }
    m
}

/// Deployed code of the reference fungible templates. None of them has a decimals slot: balances are
/// whole units, so a token running one of these has **0** decimals (v2 is what boing.finance deploys
/// and what the v8 / v9 custody pools hold). Name and symbol live in the deploy metadata, not in storage.
fn whole_unit_token_codes() -> &'static [Vec<u8>] {
    static CODES: OnceLock<Vec<Vec<u8>>> = OnceLock::new();
    CODES.get_or_init(|| {
        vec![
            reference_fungible_template_v2_bytecode(),
            reference_fungible_template_bytecode(),
            reference_fungible_secured_runtime_bytecode_v2(),
            reference_fungible_secured_runtime_bytecode(),
        ]
    })
}

/// Deployed code of wrapped BOING (native BOING 1:1 in base units, **18** decimals).
fn wboing_code() -> &'static Vec<u8> {
    static CODE: OnceLock<Vec<u8>> = OnceLock::new();
    CODE.get_or_init(boing_execution::wrapped_boing_bytecode)
}

/// Token decimals and where they came from:
/// 1. **`env`**: the operator map (**`BOING_DEX_TOKEN_DECIMALS_JSON`**);
/// 2. **`template`**: the token's deployed code is a reference fungible template (whole units, **0**)
///    or wrapped BOING (**18**);
/// 3. otherwise unknown (**`None`**, JSON **`null`**); the node does not guess.
fn resolve_token_decimals(
    state: &StateStore,
    map: &HashMap<String, u8>,
    id: &AccountId,
) -> (Option<u8>, Option<&'static str>) {
    if let Some(d) = map.get(&account_id_hex_lower(id)) {
        return (Some(*d), Some("env"));
    }
    if let Some(code) = state.get_contract_code(id) {
        if whole_unit_token_codes().iter().any(|t| t == code) {
            return (Some(0), Some("template"));
        }
        if *code == *wboing_code() {
            return (Some(boing_execution::WBOING_DECIMALS), Some("template"));
        }
    }
    (None, None)
}

fn decimals_json(d: Option<u8>) -> Value {
    d.map(Value::from).unwrap_or(Value::Null)
}

fn source_json(s: Option<&'static str>) -> Value {
    s.map(Value::from).unwrap_or(Value::Null)
}

fn parse_u128_opt(m: &serde_json::Map<String, Value>, key: &str) -> Option<u128> {
    let v = m.get(key)?;
    let s = match v {
        Value::String(s) => s.as_str(),
        Value::Number(n) => return n.as_u64().map(u128::from),
        _ => return None,
    };
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    if !t.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    t.parse().ok()
}

fn resolve_factory_account(
    m: &serde_json::Map<String, Value>,
) -> Result<AccountId, DexDiscoveryRpcError> {
    if let Some(v) = m.get("factory") {
        if let Some(s) = v.as_str() {
            if !s.trim().is_empty() {
                return parse_account_id_hex(s);
            }
        }
    }
    let Ok(raw) = std::env::var("BOING_CANONICAL_NATIVE_DEX_FACTORY") else {
        return Err(err_config(
            "Set params.factory (32-byte hex) or BOING_CANONICAL_NATIVE_DEX_FACTORY; see boing_getNetworkInfo.end_user.canonical_native_dex_factory",
        ));
    };
    parse_account_id_hex(raw.trim())
}

fn parse_pool_cursor(cursor: Option<&str>) -> u64 {
    let Some(c) = cursor.map(str::trim).filter(|s| !s.is_empty()) else {
        return 0;
    };
    c.strip_prefix('i')
        .and_then(|rest| rest.parse::<u64>().ok())
        .unwrap_or(0)
}

fn pool_cursor_next(start: u64, returned: usize) -> String {
    format!("i{}", start.saturating_add(returned as u64))
}

fn abbrev_token_label(id: &AccountId) -> (String, String) {
    let h = account_id_hex_lower(id);
    let sym = format!("{}…{}", &h[..10], &h[h.len().saturating_sub(6)..]);
    let name = format!("Pool token {}", &h[..12]);
    (sym, name)
}

fn reserve_product(a: u128, b: u128) -> u128 {
    a.checked_mul(b).unwrap_or(u128::MAX)
}

fn pool_passes_filters(
    ra: u128,
    rb: u128,
    min_product: Option<u128>,
    min_liq: Option<u128>,
) -> bool {
    if let Some(th) = min_product {
        if reserve_product(ra, rb) < th {
            return false;
        }
    }
    if let Some(m) = min_liq {
        if ra < m || rb < m {
            return false;
        }
    }
    true
}

/// `pool` -> `(token_a, token_b)` as stored in the directory.
fn factory_directory(
    state: &StateStore,
    factory: &AccountId,
) -> HashMap<AccountId, (AccountId, AccountId)> {
    let n = factory_pair_count(state, factory);
    let mut m = HashMap::with_capacity(n as usize);
    for idx in 0..n {
        let (ta, tb, pool) = read_triplet(state, factory, idx);
        m.insert(pool, (ta, tb));
    }
    m
}

fn log_pool_account_id(log: &ExecutionLog) -> Option<AccountId> {
    if log.data.len() < 32 {
        return None;
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&log.data[log.data.len() - 32..]);
    Some(AccountId(arr))
}

/// Minimum block height per pool from validated **`Log3`** register events matching directory triplets.
/// Receipts are sorted ascending by **`(block_height, tx_index)`** so the first hit is the earliest height.
/// Returns **`(heights, receipts_examined, capped)`** — **`capped`** when the scan stopped before every pool was matched.
fn scan_pool_register_heights(
    directory: &HashMap<AccountId, (AccountId, AccountId)>,
    receipts: &HashMap<Hash, ExecutionReceipt>,
) -> (HashMap<AccountId, u64>, usize, bool) {
    let mut out: HashMap<AccountId, u64> = HashMap::new();
    if directory.is_empty() {
        return (out, 0, false);
    }
    let mut pending: HashSet<AccountId> = directory.keys().copied().collect();
    let mut sorted: Vec<&ExecutionReceipt> = receipts.values().collect();
    sorted.sort_by_key(|r| (r.block_height, r.tx_index));
    let limit = max_receipt_scans_limit();
    let mut scanned = 0usize;
    let mut capped = false;
    for r in sorted {
        if pending.is_empty() {
            break;
        }
        if let Some(max) = limit {
            if scanned >= max {
                capped = true;
                break;
            }
        }
        scanned += 1;
        if !r.success {
            continue;
        }
        for log in &r.logs {
            if log.topics.len() != 3 {
                continue;
            }
            if log.topics[0] != NATIVE_DEX_FACTORY_TOPIC_REGISTER {
                continue;
            }
            let Some(pool) = log_pool_account_id(log) else {
                continue;
            };
            let Some(&(d_ta, d_tb)) = directory.get(&pool) else {
                continue;
            };
            let t1 = AccountId(log.topics[1]);
            let t2 = AccountId(log.topics[2]);
            if t1 == d_ta && t2 == d_tb {
                let h = r.block_height;
                out.entry(pool).or_insert(h);
                pending.remove(&pool);
            }
        }
    }
    (out, scanned, capped)
}

fn predicted_deploy_addresses(tx: &Transaction) -> Vec<AccountId> {
    let Some((bytecode, _, _, _, _)) = tx.payload.as_contract_deploy() else {
        return Vec::new();
    };
    let mut v = Vec::new();
    if let Some(salt) = tx.payload.deploy_create2_salt() {
        v.push(create2_contract_address(&tx.sender, &salt, bytecode));
        if contract_deploy_uses_init_code(bytecode) {
            let body = contract_deploy_init_body(bytecode);
            v.push(create2_contract_address(&tx.sender, &salt, body));
        }
    } else {
        v.push(nonce_derived_contract_address(&tx.sender, tx.nonce));
    }
    v
}

/// `token_id` -> `(symbol, name, metadata_source)` from recent **`ContractDeployWithPurposeAndMetadata`** txs.
/// Scans **newest-first** and stops when every wanted id is resolved. Returns
/// **`(map, blocks_considered, matched_count, unmatched_want)`**.
fn scan_deploy_token_metadata(
    chain: &ChainState,
    want: &HashSet<AccountId>,
    max_blocks: u64,
) -> (
    HashMap<AccountId, (String, String, &'static str)>,
    u64,
    usize,
    usize,
) {
    let tip = chain.height();
    let low = tip.saturating_sub(max_blocks);
    let mut out: HashMap<AccountId, (String, String, &'static str)> = HashMap::new();
    if want.is_empty() {
        return (out, 0, 0, 0);
    }
    let mut remaining: HashSet<AccountId> = want.iter().copied().collect();
    let mut blocks_considered = 0u64;
    for h in (low..=tip).rev() {
        if remaining.is_empty() {
            break;
        }
        blocks_considered += 1;
        let Some(block) = chain.get_block_by_height(h) else {
            continue;
        };
        for tx in &block.transactions {
            let TransactionPayload::ContractDeployWithPurposeAndMetadata {
                asset_name,
                asset_symbol,
                ..
            } = &tx.payload
            else {
                continue;
            };
            let sym_raw = asset_symbol.as_deref().unwrap_or("").trim();
            let name_raw = asset_name.as_deref().unwrap_or("").trim();
            if sym_raw.is_empty() && name_raw.is_empty() {
                continue;
            }
            for addr in predicted_deploy_addresses(tx) {
                if !remaining.contains(&addr) || out.contains_key(&addr) {
                    continue;
                }
                let sym = sym_raw.chars().take(32).collect::<String>();
                let name = name_raw.chars().take(256).collect::<String>();
                out.insert(addr, (sym, name, "deploy"));
                remaining.remove(&addr);
            }
        }
    }
    let matched = out.len();
    let unmatched = remaining.len();
    (out, blocks_considered, matched, unmatched)
}

fn token_first_seen_height(
    token: &AccountId,
    directory: &HashMap<AccountId, (AccountId, AccountId)>,
    pool_heights: &HashMap<AccountId, u64>,
) -> Option<u64> {
    let mut m: Option<u64> = None;
    for (pool, (ta, tb)) in directory {
        if *ta != *token && *tb != *token {
            continue;
        }
        if let Some(&h) = pool_heights.get(pool) {
            m = Some(m.map_or(h, |e| e.min(h)));
        }
    }
    m
}

fn merge_token_display(
    id: &AccountId,
    meta: &HashMap<AccountId, (String, String, &'static str)>,
) -> (String, String, &'static str) {
    if let Some((s, n, src)) = meta.get(id) {
        let (fallback_sym, fallback_name) = abbrev_token_label(id);
        let sym = if s.is_empty() {
            fallback_sym
        } else {
            s.clone()
        };
        let name = if n.is_empty() {
            fallback_name
        } else {
            n.clone()
        };
        (sym, name, src)
    } else {
        let (a, b) = abbrev_token_label(id);
        (a, b, "abbrev")
    }
}

/// `boing_listDexPools`
pub fn list_dex_pools(
    state: &StateStore,
    receipts: &HashMap<Hash, ExecutionReceipt>,
    _chain: &ChainState,
    params: &Option<Value>,
) -> Result<Value, DexDiscoveryRpcError> {
    let m = dex_object_params(params)?;
    let factory = resolve_factory_account(m)?;
    let light = param_light(m);
    let diag = param_include_diagnostics(m);
    let limit = param_limit(m);
    let start = parse_pool_cursor(m.get("cursor").and_then(|v| v.as_str()));

    let total = factory_pair_count(state, &factory);
    if start >= total {
        if diag {
            return Ok(json!({
                "pools": [],
                "nextCursor": null,
                "diagnostics": {
                    "receiptScans": 0,
                    "receiptScanCapped": false,
                }
            }));
        }
        return Ok(json!({ "pools": [], "nextCursor": null }));
    }

    let directory = factory_directory(state, &factory);
    let (pool_heights, receipt_scans, receipt_capped) = if light {
        (HashMap::new(), 0usize, false)
    } else {
        scan_pool_register_heights(&directory, receipts)
    };
    let decimals_map = parse_token_decimals_json();

    let end = (start as usize + limit).min(total as usize);
    let mut pools = Vec::new();
    for idx in start..(end as u64) {
        let (reg_ta, reg_tb, pool) = read_triplet(state, &factory, idx);
        let (slot_ta, slot_tb) = pool_token_ids(state, &pool);
        let token_a = if slot_ta.0 == [0u8; 32] {
            reg_ta
        } else {
            slot_ta
        };
        let token_b = if slot_tb.0 == [0u8; 32] {
            reg_tb
        } else {
            slot_tb
        };
        let (ra, rb) = pool_reserves(state, &pool);
        let fee_bps = pool_fee_bps(state, &pool);
        let created = pool_heights.get(&pool).copied();
        let (dec_a, _) = resolve_token_decimals(state, &decimals_map, &token_a);
        let (dec_b, _) = resolve_token_decimals(state, &decimals_map, &token_b);
        pools.push(json!({
            "poolHex": account_id_hex_lower(&pool),
            "tokenAHex": account_id_hex_lower(&token_a),
            "tokenBHex": account_id_hex_lower(&token_b),
            "tokenADecimals": decimals_json(dec_a),
            "tokenBDecimals": decimals_json(dec_b),
            "feeBps": fee_bps,
            "reserveA": ra.to_string(),
            "reserveB": rb.to_string(),
            "createdAtHeight": created.map(Value::from).unwrap_or(Value::Null),
        }));
    }

    let next = if (end as u64) < total {
        Some(pool_cursor_next(start, pools.len()))
    } else {
        None
    };
    if diag {
        return Ok(json!({
            "pools": pools,
            "nextCursor": next,
            "diagnostics": {
                "receiptScans": receipt_scans,
                "receiptScanCapped": receipt_capped,
            }
        }));
    }
    Ok(json!({ "pools": pools, "nextCursor": next }))
}

#[derive(Clone, Default)]
struct TokenAgg {
    pool_count: u32,
}

/// Build the DEX-derived token universe (keys are unique `AccountId`s).
fn collect_dex_tokens(
    state: &StateStore,
    factory: &AccountId,
    min_product: Option<u128>,
    min_liq: Option<u128>,
) -> HashMap<AccountId, TokenAgg> {
    let total = factory_pair_count(state, factory);
    let mut map: HashMap<AccountId, TokenAgg> = HashMap::new();
    for idx in 0..total {
        let (reg_ta, reg_tb, pool) = read_triplet(state, factory, idx);
        let (slot_ta, slot_tb) = pool_token_ids(state, &pool);
        let token_a = if slot_ta.0 == [0u8; 32] {
            reg_ta
        } else {
            slot_ta
        };
        let token_b = if slot_tb.0 == [0u8; 32] {
            reg_tb
        } else {
            slot_tb
        };
        let (ra, rb) = pool_reserves(state, &pool);
        if !pool_passes_filters(ra, rb, min_product, min_liq) {
            continue;
        }
        for t in [token_a, token_b] {
            if t.0 == [0u8; 32] {
                continue;
            }
            let e = map.entry(t).or_default();
            e.pool_count = e.pool_count.saturating_add(1);
        }
    }
    map
}

fn parse_token_cursor(cursor: Option<&str>) -> Result<Option<AccountId>, DexDiscoveryRpcError> {
    let Some(c) = cursor.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    parse_account_id_hex(c).map(Some)
}

/// `boing_listDexTokens`
pub fn list_dex_tokens(
    state: &StateStore,
    receipts: &HashMap<Hash, ExecutionReceipt>,
    chain: &ChainState,
    params: &Option<Value>,
) -> Result<Value, DexDiscoveryRpcError> {
    let m = dex_object_params(params)?;
    let factory = resolve_factory_account(m)?;
    let light = param_light(m);
    let diag = param_include_diagnostics(m);
    let limit = param_limit(m);
    let min_product = parse_u128_opt(m, "minReserveProduct");
    let min_liq = parse_u128_opt(m, "minLiquidityWei");
    let after = parse_token_cursor(m.get("cursor").and_then(|v| v.as_str()))?;
    let decimals_map = parse_token_decimals_json();

    let directory = factory_directory(state, &factory);
    let (pool_heights, receipt_scans, receipt_capped) = if light {
        (HashMap::new(), 0usize, false)
    } else {
        scan_pool_register_heights(&directory, receipts)
    };

    let map = collect_dex_tokens(state, &factory, min_product, min_liq);
    let mut rows: Vec<(AccountId, TokenAgg)> = map.into_iter().collect();
    rows.sort_by(|(a, _), (b, _)| a.0.cmp(&b.0));

    if let Some(after_id) = after {
        rows.retain(|(id, _)| id.0 > after_id.0);
    }

    let want_ids: HashSet<AccountId> = rows.iter().map(|(id, _)| *id).collect();
    let (meta, deploy_blocks, deploy_matched, deploy_unmatched) = if light {
        (HashMap::new(), 0u64, 0usize, 0usize)
    } else {
        scan_deploy_token_metadata(chain, &want_ids, metadata_scan_blocks_limit())
    };

    let has_more = rows.len() > limit;
    let page: Vec<Value> = rows
        .into_iter()
        .take(limit)
        .map(|(id, agg)| {
            let (sym, name, m_src) = merge_token_display(&id, &meta);
            let first = token_first_seen_height(&id, &directory, &pool_heights);
            let (dec, dec_src) = resolve_token_decimals(state, &decimals_map, &id);
            json!({
                "id": account_id_hex_lower(&id),
                "symbol": sym,
                "name": name,
                "decimals": decimals_json(dec),
                "decimalsSource": source_json(dec_src),
                "poolCount": agg.pool_count,
                "firstSeenHeight": first.map(Value::from).unwrap_or(Value::Null),
                "metadataSource": m_src,
            })
        })
        .collect();

    let next = if has_more {
        page.last()
            .and_then(|row| row.get("id"))
            .and_then(|v| v.as_str())
            .map(str::to_string)
    } else {
        None
    };

    if diag {
        return Ok(json!({
            "tokens": page,
            "nextCursor": next,
            "diagnostics": {
                "receiptScans": receipt_scans,
                "receiptScanCapped": receipt_capped,
                "deployBlocksScanned": deploy_blocks,
                "deployMetadataMatched": deploy_matched,
                "deployMetadataUnmatchedWant": deploy_unmatched,
            }
        }));
    }
    Ok(json!({ "tokens": page, "nextCursor": next }))
}

/// `boing_getDexToken`
pub fn get_dex_token(
    state: &StateStore,
    receipts: &HashMap<Hash, ExecutionReceipt>,
    chain: &ChainState,
    params: &Option<Value>,
) -> Result<Value, DexDiscoveryRpcError> {
    let m = dex_object_params(params)?;
    let factory = resolve_factory_account(m)?;
    let light = param_light(m);
    let diag = param_include_diagnostics(m);
    let id_hex = m
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| err_bad("params.id: expected hex string".to_string()))?;
    let want = parse_account_id_hex(id_hex)?;
    let decimals_map = parse_token_decimals_json();

    let map = collect_dex_tokens(state, &factory, None, None);
    let Some(agg) = map.get(&want) else {
        return Ok(Value::Null);
    };

    let directory = factory_directory(state, &factory);
    let (pool_heights, receipt_scans, receipt_capped) = if light {
        (HashMap::new(), 0usize, false)
    } else {
        scan_pool_register_heights(&directory, receipts)
    };
    let mut want_one = HashSet::new();
    want_one.insert(want);
    let (meta, deploy_blocks, deploy_matched, deploy_unmatched) = if light {
        (HashMap::new(), 0u64, 0usize, 0usize)
    } else {
        scan_deploy_token_metadata(chain, &want_one, metadata_scan_blocks_limit())
    };
    let (sym, name, m_src) = merge_token_display(&want, &meta);
    let first = token_first_seen_height(&want, &directory, &pool_heights);
    let (dec, dec_src) = resolve_token_decimals(state, &decimals_map, &want);

    if diag {
        return Ok(json!({
            "id": account_id_hex_lower(&want),
            "symbol": sym,
            "name": name,
            "decimals": decimals_json(dec),
            "decimalsSource": source_json(dec_src),
            "poolCount": agg.pool_count,
            "firstSeenHeight": first.map(Value::from).unwrap_or(Value::Null),
            "metadataSource": m_src,
            "diagnostics": {
                "receiptScans": receipt_scans,
                "receiptScanCapped": receipt_capped,
                "deployBlocksScanned": deploy_blocks,
                "deployMetadataMatched": deploy_matched,
                "deployMetadataUnmatchedWant": deploy_unmatched,
            }
        }));
    }
    Ok(json!({
        "id": account_id_hex_lower(&want),
        "symbol": sym,
        "name": name,
        "decimals": decimals_json(dec),
        "decimalsSource": source_json(dec_src),
        "poolCount": agg.pool_count,
        "firstSeenHeight": first.map(Value::from).unwrap_or(Value::Null),
        "metadataSource": m_src,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const V2_TOKEN: AccountId = AccountId([0x31; 32]);
    const V1_TOKEN: AccountId = AccountId([0x32; 32]);
    const SECURED_TOKEN: AccountId = AccountId([0x33; 32]);
    const OTHER_CONTRACT: AccountId = AccountId([0x34; 32]);
    const NO_CODE: AccountId = AccountId([0x35; 32]);

    fn state() -> StateStore {
        let mut s = StateStore::new();
        s.set_contract_code(V2_TOKEN, reference_fungible_template_v2_bytecode());
        s.set_contract_code(V1_TOKEN, reference_fungible_template_bytecode());
        s.set_contract_code(
            SECURED_TOKEN,
            reference_fungible_secured_runtime_bytecode_v2(),
        );
        let mut other = reference_fungible_template_v2_bytecode();
        other.push(0x00);
        s.set_contract_code(OTHER_CONTRACT, other);
        s
    }

    #[test]
    fn reference_template_tokens_are_whole_units() {
        let s = state();
        let none = HashMap::new();
        assert_eq!(
            resolve_token_decimals(&s, &none, &V2_TOKEN),
            (Some(0), Some("template"))
        );
        assert_eq!(
            resolve_token_decimals(&s, &none, &V1_TOKEN),
            (Some(0), Some("template"))
        );
        assert_eq!(
            resolve_token_decimals(&s, &none, &SECURED_TOKEN),
            (Some(0), Some("template"))
        );
    }

    #[test]
    fn wrapped_boing_reports_18_decimals() {
        let mut st = StateStore::new();
        let id = AccountId([0x5e; 32]);
        st.set_contract_code(id, boing_execution::wrapped_boing_bytecode());
        assert_eq!(
            resolve_token_decimals(&st, &HashMap::new(), &id),
            (Some(18), Some("template"))
        );
    }

    #[test]
    fn unknown_tokens_are_null_not_18() {
        let s = state();
        let none = HashMap::new();
        assert_eq!(
            resolve_token_decimals(&s, &none, &OTHER_CONTRACT),
            (None, None)
        );
        assert_eq!(resolve_token_decimals(&s, &none, &NO_CODE), (None, None));
        assert_eq!(decimals_json(None), Value::Null);
        assert_eq!(decimals_json(Some(0)), json!(0));
    }

    #[test]
    fn operator_map_wins_and_keeps_listed_tokens() {
        let s = state();
        let raw = format!(
            r#"{{"{}":0,"{}":6,"{}":"9"}}"#,
            account_id_hex_lower(&V2_TOKEN),
            account_id_hex_lower(&NO_CODE),
            account_id_hex_lower(&OTHER_CONTRACT)
        );
        let m = parse_token_decimals_map(&raw);
        assert_eq!(
            resolve_token_decimals(&s, &m, &V2_TOKEN),
            (Some(0), Some("env"))
        );
        assert_eq!(
            resolve_token_decimals(&s, &m, &NO_CODE),
            (Some(6), Some("env"))
        );
        assert_eq!(
            resolve_token_decimals(&s, &m, &OTHER_CONTRACT),
            (Some(9), Some("env"))
        );
        // Template detection still applies to tokens the map does not list.
        assert_eq!(
            resolve_token_decimals(&s, &m, &V1_TOKEN),
            (Some(0), Some("template"))
        );
    }

    #[test]
    fn bad_map_entries_are_skipped_not_defaulted() {
        let a = account_id_hex_lower(&V2_TOKEN);
        let b = account_id_hex_lower(&NO_CODE);
        let c = account_id_hex_lower(&OTHER_CONTRACT);
        let raw = format!(r#"{{"{a}":"x","{b}":256,"{c}":-1,"0x12":3}}"#);
        assert!(parse_token_decimals_map(&raw).is_empty());
        assert!(parse_token_decimals_map("").is_empty());
        assert!(parse_token_decimals_map("[1,2]").is_empty());
        let upper = format!(r#"{{"0x{}":2}}"#, a.trim_start_matches("0x").to_uppercase());
        assert_eq!(parse_token_decimals_map(&upper).get(&a), Some(&2));
    }
}
