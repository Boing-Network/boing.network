//! Name registry JSON-RPC (read-only views over committed state at the current head).
//!
//! - `boing_resolveName [name]` → active record or `null`
//! - `boing_reverseName [address]` → verified primary name or `null`
//! - `boing_getNameInfo [name]` → validity, availability, rent, record (any status), registry params
//! - `boing_checkNames [[names…]]` (or `[name, name, …]`, ≤25) → availability + rent per name
//! - `boing_namesOwnedBy [address]` → names owned (active or in grace)

use boing_primitives::{
    check_registrable, display_name, is_reserved, name_rent_per_year, normalize_name, AccountId,
};
use boing_state::{NameRecord, NameRegistry, NameRegistryParams, NameStatus};
use serde_json::{json, Value};

use crate::node::BoingNode;

pub const NAME_RPC_METHODS: &[&str] = &[
    "boing_checkNames",
    "boing_getNameInfo",
    "boing_namesOwnedBy",
    "boing_resolveName",
    "boing_reverseName",
];

const MAX_CHECK: usize = 25;

fn hex0x(a: &AccountId) -> String {
    format!("0x{}", hex::encode(a.0))
}

fn bad(msg: &str) -> (i32, String) {
    (-32602, format!("Invalid params: {msg}"))
}

fn first_string(params: &Value) -> Option<String> {
    match params {
        Value::Array(v) => v.first().and_then(|x| x.as_str()).map(str::to_string),
        Value::String(s) => Some(s.clone()),
        Value::Object(o) => o
            .get("name")
            .or_else(|| o.get("address"))
            .and_then(|x| x.as_str())
            .map(str::to_string),
        _ => None,
    }
}

fn parse_addr(s: &str) -> Option<AccountId> {
    let b = hex::decode(s.trim().trim_start_matches("0x")).ok()?;
    let arr: [u8; 32] = b.try_into().ok()?;
    Some(AccountId(arr))
}

fn record_json(label: &str, r: &NameRecord, status: NameStatus) -> Value {
    json!({
        "name": label,
        "display": display_name(label),
        "owner": hex0x(&r.owner),
        "target": hex0x(&r.target),
        "registered_at": r.registered_at,
        "expires_at": r.expires_at,
        "status": status.as_str(),
    })
}

fn params_json(p: Option<&NameRegistryParams>, head: u64) -> Value {
    match p {
        None => json!({ "enabled": false }),
        Some(p) => json!({
            "enabled": true,
            "active": head.saturating_add(1) >= p.activation_height,
            "activation_height": p.activation_height,
            "blocks_per_year": p.blocks_per_year,
            "grace_blocks": p.grace_blocks,
            "commit_min_age": p.commit_min_age,
            "commit_max_age": p.commit_max_age,
            "rent_per_year": { "3": "10", "4": "5", "5": "2", "6+": "1" },
            "max_years": boing_primitives::NAME_MAX_YEARS,
        }),
    }
}

/// Availability / validity for one raw input.
fn check_one(reg: &NameRegistry, p: Option<&NameRegistryParams>, head: u64, raw: &str) -> Value {
    let label = match normalize_name(raw) {
        Ok(l) => l,
        Err(e) => {
            return json!({ "input": raw, "valid": false, "error": e.to_string(), "available": false })
        }
    };
    let reserved = is_reserved(&label);
    let registrable = check_registrable(&label).is_ok();
    // A name is judged at the next block height (where a register tx would execute).
    let at = head.saturating_add(1);
    let status = p.and_then(|p| reg.status(&label, p, at));
    let free = match p {
        Some(p) => reg.is_available(&label, p, at),
        None => true,
    };
    json!({
        "input": raw,
        "name": label,
        "display": display_name(&label),
        "valid": true,
        "reserved": reserved,
        "available": registrable && free,
        "status": status.map(|s| s.as_str()),
        "rent_per_year": name_rent_per_year(&label).to_string(),
    })
}

pub fn dispatch(node: &BoingNode, method: &str, params: &Value) -> Result<Value, (i32, String)> {
    let head = node.chain.height();
    let st = &node.state;
    let reg = st.names();
    let p = st.name_registry_params();
    // Reads are evaluated at the next block height, like a tx submitted now.
    let at = head.saturating_add(1);
    match method {
        "boing_resolveName" => {
            let raw = first_string(params).ok_or_else(|| bad("expected [name]"))?;
            let Ok(label) = normalize_name(&raw) else {
                return Ok(Value::Null);
            };
            let Some(p) = p else { return Ok(Value::Null) };
            Ok(reg
                .resolve(&label, p, at)
                .map(|r| record_json(&label, r, NameStatus::Active))
                .unwrap_or(Value::Null))
        }
        "boing_reverseName" => {
            let raw = first_string(params).ok_or_else(|| bad("expected [address]"))?;
            let addr = parse_addr(&raw).ok_or_else(|| bad("address must be 32 bytes hex"))?;
            let Some(p) = p else { return Ok(Value::Null) };
            Ok(match reg.reverse_verified(&addr, p, at) {
                Some(l) => {
                    json!({ "name": l, "display": display_name(l), "address": hex0x(&addr), "verified": true })
                }
                None => Value::Null,
            })
        }
        "boing_getNameInfo" => {
            let raw = first_string(params).ok_or_else(|| bad("expected [name]"))?;
            let mut out = check_one(reg, p, head, &raw);
            if let (Some(label), Some(pp)) = (
                out.get("name").and_then(|v| v.as_str()).map(str::to_string),
                p,
            ) {
                if let Some(r) = reg.names.get(&label) {
                    out["record"] = record_json(&label, r, NameRegistry::status_of(r, pp, at));
                }
            }
            out["registry"] = params_json(p, head);
            out["head_height"] = json!(head);
            Ok(out)
        }
        "boing_checkNames" => {
            let names: Vec<String> = match params {
                Value::Array(v) if v.len() == 1 && v[0].is_array() => v[0]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect(),
                Value::Array(v) => v
                    .iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect(),
                _ => return Err(bad("expected [[name, …]]")),
            };
            if names.is_empty() || names.len() > MAX_CHECK {
                return Err(bad("1-25 names"));
            }
            Ok(json!({
                "registry": params_json(p, head),
                "results": names.iter().map(|n| check_one(reg, p, head, n)).collect::<Vec<_>>(),
            }))
        }
        "boing_namesOwnedBy" => {
            let raw = first_string(params).ok_or_else(|| bad("expected [address]"))?;
            let addr = parse_addr(&raw).ok_or_else(|| bad("address must be 32 bytes hex"))?;
            let Some(p) = p else { return Ok(json!([])) };
            Ok(Value::Array(
                reg.owned_by(&addr, p, at)
                    .into_iter()
                    .map(|(l, r)| record_json(l, r, NameRegistry::status_of(r, p, at)))
                    .collect(),
            ))
        }
        _ => Err((-32601, format!("Method not found: {method}"))),
    }
}
