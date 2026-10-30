//! `boing validator register | deregister | unjail | claim-unbond | status` — open validator set (permissionless).
//!
//! Keys: the **owner** (pays the 100 BOING bond, signs the tx) comes from `--secret-hex` /
//! `--secret-file` / `BOING_SECRET_HEX`; the **node** key from `--node-key-hex` /
//! `--node-key-file` / `BOING_VALIDATOR_KEY`. Prefer file/env over argv so seeds stay out of `ps`.
//! If no owner key is given, the node key owns itself. Keys are never printed.

use boing_primitives::{
    sign_validator_registration, AccessList, AccountId, SignedTransaction, Transaction,
    TransactionPayload,
};
use ed25519_dalek::SigningKey;
use serde_json::{json, Value};

use crate::token::{
    account_of, hex_account, nonce_for, parse_account, parse_signing_key, rpc, submit,
};

/// Resolve (owner, node) keys. The node key is optional for deregister/unjail when `--node` is given.
pub fn resolve_keys(
    owner_hex: Option<String>,
    owner_file: Option<String>,
    node_key_hex: Option<String>,
    node_key_file: Option<String>,
) -> anyhow::Result<(Option<SigningKey>, Option<SigningKey>)> {
    let node = optional_signing_key(
        node_key_hex,
        node_key_file,
        "BOING_VALIDATOR_KEY",
        "BOING_VALIDATOR_KEY_FILE",
        "node key",
    )?;
    let owner = optional_signing_key(
        owner_hex,
        owner_file,
        "BOING_SECRET_HEX",
        "BOING_SECRET_FILE",
        "owner secret",
    )?;
    Ok((owner, node))
}

fn optional_signing_key(
    inline: Option<String>,
    file: Option<String>,
    env_inline: &str,
    env_file: &str,
    what: &str,
) -> anyhow::Result<Option<SigningKey>> {
    let has_inline = inline.as_ref().is_some_and(|s| !s.trim().is_empty());
    let has_file = file.as_ref().is_some_and(|s| !s.trim().is_empty())
        || std::env::var(env_file).ok().is_some_and(|s| !s.trim().is_empty());
    let has_env = std::env::var(env_inline)
        .ok()
        .is_some_and(|s| !s.trim().is_empty());
    if !has_inline && !has_file && !has_env {
        return Ok(None);
    }
    let s = crate::secrets::read_secret_string(inline, file, env_inline, env_file, what)?;
    Ok(Some(parse_signing_key(&s)?))
}

fn tx(owner: &SigningKey, nonce: u64, payload: TransactionPayload) -> SignedTransaction {
    let mut tx = Transaction {
        nonce,
        sender: account_of(owner),
        payload,
        access_list: AccessList::default(),
    };
    tx.access_list = tx.suggested_parallel_access_list();
    SignedTransaction::new(tx, owner)
}

/// Node-key proof for `owner`: the chain-bound v2 proof when the signing chain id is known (from the
/// node's `boing_health.tx_signing` or `--chain-id`; security review #14), else the v1 proof.
pub fn registration_proof(owner_id: &AccountId, node: &SigningKey) -> Vec<u8> {
    match boing_primitives::default_signing_chain_id() {
        Some(c) => boing_primitives::sign_validator_registration_v2(owner_id, node, c),
        None => sign_validator_registration(owner_id, node),
    }
}

pub fn build_register_tx(owner: &SigningKey, node: &SigningKey, nonce: u64) -> SignedTransaction {
    let owner_id = account_of(owner);
    tx(
        owner,
        nonce,
        TransactionPayload::ValidatorRegister {
            node_id: account_of(node),
            node_signature: registration_proof(&owner_id, node),
        },
    )
}

pub fn build_deregister_tx(
    owner: &SigningKey,
    node_id: AccountId,
    nonce: u64,
) -> SignedTransaction {
    tx(
        owner,
        nonce,
        TransactionPayload::ValidatorDeregister { node_id },
    )
}

pub fn build_unjail_tx(owner: &SigningKey, node_id: AccountId, nonce: u64) -> SignedTransaction {
    tx(
        owner,
        nonce,
        TransactionPayload::ValidatorUnjail { node_id },
    )
}

async fn require_open_mode(rpc_url: &str) -> anyhow::Result<Value> {
    let report = rpc(rpc_url, "boing_getValidators", json!([])).await?;
    if report.get("mode").and_then(Value::as_str) != Some("open") {
        anyhow::bail!(
            "this node is not running the open validator-set mode (mode = {}); registration is not available yet",
            report.get("mode").cloned().unwrap_or(Value::Null)
        );
    }
    Ok(report)
}

pub async fn register(
    rpc_url: &str,
    owner: Option<SigningKey>,
    node: Option<SigningKey>,
    nonce: Option<u64>,
    dry_run: bool,
) -> anyhow::Result<()> {
    let node = node.ok_or_else(|| {
        anyhow::anyhow!("no node key: pass --node-key-hex or set BOING_VALIDATOR_KEY")
    })?;
    let owner = owner.unwrap_or_else(|| node.clone());
    if !dry_run {
        let report = require_open_mode(rpc_url).await?;
        if let Some(open) = report.get("open") {
            println!(
                "Open validator set: bond {} BOING, joins at the next epoch boundary (every {} blocks, next {})",
                open.get("bond_boing").or(open.get("bond")).and_then(Value::as_str).unwrap_or("?"),
                open.get("epoch_len").cloned().unwrap_or(Value::Null),
                open.get("next_epoch_height").cloned().unwrap_or(Value::Null),
            );
        }
    }
    println!(
        "Registering node {} (owner {})",
        hex_account(&account_of(&node)),
        hex_account(&account_of(&owner))
    );
    let n = nonce_for(rpc_url, &owner, nonce).await?;
    submit(rpc_url, &build_register_tx(&owner, &node, n), dry_run).await
}

fn node_id_from(
    node_arg: Option<String>,
    node_key: &Option<SigningKey>,
) -> anyhow::Result<AccountId> {
    match (node_arg, node_key) {
        (Some(s), _) => parse_account(&s),
        (None, Some(k)) => Ok(account_of(k)),
        (None, None) => anyhow::bail!(
            "pass --node <hex id> or a node key (--node-key-hex / BOING_VALIDATOR_KEY)"
        ),
    }
}

pub async fn deregister(
    rpc_url: &str,
    owner: Option<SigningKey>,
    node_key: Option<SigningKey>,
    node_arg: Option<String>,
    nonce: Option<u64>,
    dry_run: bool,
) -> anyhow::Result<()> {
    let node_id = node_id_from(node_arg, &node_key)?;
    let owner = owner.or(node_key).ok_or_else(|| {
        anyhow::anyhow!("no owner key: pass --secret-hex or set BOING_SECRET_HEX")
    })?;
    println!(
        "Deregistering node {} (bond claimable with ClaimUnbond after the delay)",
        hex_account(&node_id)
    );
    let n = nonce_for(rpc_url, &owner, nonce).await?;
    submit(rpc_url, &build_deregister_tx(&owner, node_id, n), dry_run).await
}

pub async fn unjail(
    rpc_url: &str,
    owner: Option<SigningKey>,
    node_key: Option<SigningKey>,
    node_arg: Option<String>,
    nonce: Option<u64>,
    dry_run: bool,
) -> anyhow::Result<()> {
    let node_id = node_id_from(node_arg, &node_key)?;
    let owner = owner.or(node_key).ok_or_else(|| {
        anyhow::anyhow!("no owner key: pass --secret-hex or set BOING_SECRET_HEX")
    })?;
    println!("Unjailing node {}", hex_account(&node_id));
    let n = nonce_for(rpc_url, &owner, nonce).await?;
    submit(rpc_url, &build_unjail_tx(&owner, node_id, n), dry_run).await
}

pub fn build_claim_unbond_tx(owner: &SigningKey, nonce: u64) -> SignedTransaction {
    tx(owner, nonce, TransactionPayload::ClaimUnbond)
}

/// `ClaimUnbond`: move a matured pending unbond (validator bond after deregister, or a staking unbond)
/// back to the owner's spendable balance. Without `--dry-run` the account is checked first so an
/// immature claim fails locally instead of burning a fee.
pub async fn claim_unbond(
    rpc_url: &str,
    owner: Option<SigningKey>,
    node_key: Option<SigningKey>,
    nonce: Option<u64>,
    dry_run: bool,
) -> anyhow::Result<()> {
    let owner = owner.or(node_key).ok_or_else(|| {
        anyhow::anyhow!("no owner key: pass --secret-hex or set BOING_SECRET_HEX")
    })?;
    let id = account_of(&owner);
    if !dry_run {
        let acct = rpc(rpc_url, "boing_getAccount", json!([hex_account(&id)])).await?;
        let pending = acct
            .get("pending_unbond")
            .and_then(Value::as_str)
            .unwrap_or("0")
            .to_string();
        // Display in BOING (the node formats base units; older nodes return whole BOING only).
        let pending_shown = acct
            .get("pending_unbond_boing")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| pending.clone());
        let unlock = acct
            .get("unbond_unlock_height")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let height = rpc(rpc_url, "boing_chainHeight", json!([]))
            .await?
            .as_u64()
            .unwrap_or(0);
        if pending == "0" {
            anyhow::bail!(
                "nothing to claim: {} has no pending unbond",
                hex_account(&id)
            );
        }
        // The claim executes in the next block (height + 1).
        if height + 1 < unlock {
            anyhow::bail!(
                "unbond of {pending_shown} BOING unlocks at height {unlock} (chain is at {height})"
            );
        }
        println!("Claiming {pending_shown} BOING unbond for {} (unlocked at {unlock}, chain at {height})", hex_account(&id));
    } else {
        println!("Claiming unbond for {}", hex_account(&id));
    }
    let n = nonce_for(rpc_url, &owner, nonce).await?;
    submit(rpc_url, &build_claim_unbond_tx(&owner, n), dry_run).await
}

/// Render `boing_getValidators` (optionally one node).
pub fn format_status(report: &Value, only: Option<&str>) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "mode={} height={} validators={} quorum={}\n",
        report["mode"].as_str().unwrap_or("?"),
        report["height"],
        report["validator_count"],
        report["quorum"]
    ));
    if let Some(open) = report.get("open").filter(|v| !v.is_null()) {
        out.push_str(&format!(
            "open: activation_height={} activated_at={} bond={} epoch_len={} next_epoch={} max_open={} window={} threshold={}bps cooldown={}\n",
            open["activation_height"], open["activated_at"],
            open["bond_boing"].as_str().or(open["bond"].as_str()).unwrap_or("?"),
            open["epoch_len"], open["next_epoch_height"], open["max_open_validators"],
            open["liveness_window"], open["miss_threshold_bps"], open["jail_cooldown"]
        ));
    }
    out.push_str("id                                                                  kind    status   in_set  bond  uptime   missed/scheduled\n");
    for v in report["validators"].as_array().into_iter().flatten() {
        let id = v["id"].as_str().unwrap_or("");
        if let Some(f) = only {
            if !id.eq_ignore_ascii_case(f) {
                continue;
            }
        }
        let mut status = v["status"].as_str().unwrap_or("?").to_string();
        if let Some(u) = v["jailed_until"].as_u64() {
            status = format!("{status}(until {u})");
        }
        out.push_str(&format!(
            "{id:<68}  {:<6}  {:<8} {:<6}  {:<4}  {:>6.1}%  {}/{}\n",
            v["kind"].as_str().unwrap_or("?"),
            status,
            v["in_set"].as_bool().unwrap_or(false),
            v["bond_boing"]
                .as_str()
                .or(v["bond"].as_str())
                .unwrap_or("0"),
            v["uptime_pct"].as_f64().unwrap_or(0.0),
            v["missed_proposals"],
            v["scheduled_proposals"],
        ));
        if v["status"] == "jailed" {
            let lj = &v["last_jail"];
            if lj.is_object() {
                out.push_str(&format!(
                    "    at jailing (block {}): uptime {:.1}%, missed {}/{} (current window reset)\n",
                    lj["jailed_at"],
                    lj["uptime_pct"].as_f64().unwrap_or(0.0),
                    lj["missed_proposals"],
                    lj["scheduled_proposals"]
                ));
            } else {
                out.push_str("    at jailing: not recorded by this node (jailed before it tracked jail stats)\n");
            }
        }
    }
    out
}

pub async fn status(rpc_url: &str, node: Option<String>, json_out: bool) -> anyhow::Result<()> {
    let report = rpc(rpc_url, "boing_getValidators", json!([])).await?;
    if json_out {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }
    let only = node
        .map(|n| parse_account(&n).map(|a| hex_account(&a)))
        .transpose()?;
    print!("{}", format_status(&report, only.as_deref()));
    Ok(())
}

#[cfg(test)]
mod claim_tests {
    use super::*;
    #[test]
    fn claim_unbond_tx_is_signed_by_owner() {
        let owner = SigningKey::from_bytes(&[9u8; 32]);
        let t = build_claim_unbond_tx(&owner, 7);
        assert_eq!(t.tx.nonce, 7);
        assert_eq!(t.tx.sender, account_of(&owner));
        assert!(matches!(t.tx.payload, TransactionPayload::ClaimUnbond));
        assert!(t.verify().is_ok());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_tx_is_bound_and_verifies() {
        let owner = SigningKey::from_bytes(&[9u8; 32]);
        let node = SigningKey::from_bytes(&[7u8; 32]);
        let stx = build_register_tx(&owner, &node, 4);
        assert!(stx.verify().is_ok());
        assert_eq!(stx.tx.sender, account_of(&owner));
        match &stx.tx.payload {
            TransactionPayload::ValidatorRegister {
                node_id,
                node_signature,
            } => {
                assert_eq!(*node_id, account_of(&node));
                assert!(boing_primitives::verify_validator_registration(
                    &account_of(&owner),
                    node_id,
                    node_signature
                ));
            }
            _ => panic!("wrong payload"),
        }
        let d = build_deregister_tx(&owner, account_of(&node), 5);
        assert!(matches!(
            d.tx.payload,
            TransactionPayload::ValidatorDeregister { .. }
        ));
        let u = build_unjail_tx(&owner, account_of(&node), 6);
        assert!(matches!(
            u.tx.payload,
            TransactionPayload::ValidatorUnjail { .. }
        ));
    }

    #[test]
    fn status_formatting() {
        let report = json!({
            "mode": "open", "height": 12, "validator_count": 2, "quorum": 1,
            "open": {"activation_height": 5, "activated_at": 10, "bond": "100", "epoch_len": 5,
                     "next_epoch_height": 15, "max_open_validators": 20, "liveness_window": 1000,
                     "miss_threshold_bps": 1000, "jail_cooldown": 500},
            "validators": [
                {"id": "0xaa", "kind": "anchor", "status": "active", "in_set": true, "bond": "0",
                 "uptime_pct": 100.0, "missed_proposals": 0, "scheduled_proposals": 3, "jailed_until": null},
                {"id": "0xbb", "kind": "open", "status": "jailed", "in_set": false,
                 "bond": "100000000000000000000", "bond_boing": "100",
                 "uptime_pct": 100.0, "missed_proposals": 0, "scheduled_proposals": 0, "jailed_until": 25,
                 "last_jail": {"jailed_at": 15, "uptime_pct": 0.0, "missed_proposals": 2, "scheduled_proposals": 2}}
            ]
        });
        let s = format_status(&report, None);
        assert!(s.contains("mode=open"));
        assert!(s.contains("jailed(until 25)"));
        assert!(
            !s.contains("100000000000000000000"),
            "bond shown in BOING, not base units: {s}"
        );
        assert!(
            s.contains("at jailing (block 15): uptime 0.0%, missed 2/2"),
            "{s}"
        );
        let one = format_status(&report, Some("0xAA"));
        assert!(one.contains("0xaa") && !one.contains("0xbb"));
    }
}
