//! `boing name …` — on-chain name registry (`label.boing`).
//!
//! Reads: `resolve`, `reverse`, `info`, `check`, `owned`. Writes (signed with `--secret-hex` /
//! `BOING_SECRET_HEX`; the key is never printed): `claim` (commit → wait → register [→ set primary]),
//! `renew`, `transfer`, `set-target`, `set-primary`, `release`. `--dry-run` prints the plan, rent and the
//! signed first transaction without submitting anything.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use boing_primitives::{
    display_name, hasher, name_commitment, name_rent, normalize_name, AccessList, AccountId,
    SignedTransaction, Transaction, TransactionPayload,
};
use ed25519_dalek::SigningKey;
use serde_json::{json, Value};

use crate::token::{account_of, hex_account, nonce_for, parse_account, rpc, submit};

fn tx(key: &SigningKey, nonce: u64, payload: TransactionPayload) -> SignedTransaction {
    let mut tx = Transaction {
        nonce,
        sender: account_of(key),
        payload,
        access_list: AccessList::default(),
    };
    tx.access_list = tx.suggested_parallel_access_list();
    SignedTransaction::new(tx, key)
}

/// Unpredictable salt without an RNG dependency: hash of the secret seed, label and the clock.
fn fresh_salt(key: &SigningKey, label: &str) -> [u8; 32] {
    let mut h = hasher();
    h.update(b"BOING_NAME_SALT_V1");
    h.update(&key.to_bytes());
    h.update(label.as_bytes());
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    h.update(&now.as_nanos().to_le_bytes());
    *h.finalize().as_bytes()
}

pub fn build_commit_tx(
    key: &SigningKey,
    nonce: u64,
    label: &str,
    salt: &[u8; 32],
) -> SignedTransaction {
    let c = name_commitment(label, &account_of(key), salt);
    tx(key, nonce, TransactionPayload::NameCommit { commitment: c })
}

pub fn build_register_tx(
    key: &SigningKey,
    nonce: u64,
    label: &str,
    salt: [u8; 32],
    target: AccountId,
    years: u8,
) -> SignedTransaction {
    tx(
        key,
        nonce,
        TransactionPayload::NameRegister {
            name: label.into(),
            salt,
            target,
            years,
        },
    )
}

fn label_of(name: &str) -> anyhow::Result<String> {
    normalize_name(name).map_err(|e| anyhow::anyhow!("invalid name {name:?}: {e}"))
}

async fn wait_for_receipt(
    rpc_url: &str,
    stx: &SignedTransaction,
    timeout: Duration,
) -> anyhow::Result<Value> {
    let id = format!("0x{}", hex::encode(stx.tx.id().0));
    let start = std::time::Instant::now();
    loop {
        if let Ok(r) = rpc(rpc_url, "boing_getTransactionReceipt", json!([id])).await {
            if !r.is_null() {
                if r.get("success").and_then(Value::as_bool) == Some(false) {
                    anyhow::bail!(
                        "transaction {id} failed: {}",
                        r.get("error").cloned().unwrap_or(Value::Null)
                    );
                }
                return Ok(r);
            }
        }
        if start.elapsed() > timeout {
            anyhow::bail!("timed out waiting for {id} to be included");
        }
        tokio::time::sleep(Duration::from_millis(1500)).await;
    }
}

fn print_record(v: &Value) {
    if v.is_null() {
        println!("  (not registered or not active)");
        return;
    }
    for k in [
        "display",
        "target",
        "owner",
        "status",
        "registered_at",
        "expires_at",
    ] {
        if let Some(x) = v.get(k) {
            println!(
                "  {k:<13} {}",
                x.as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| x.to_string())
            );
        }
    }
}

pub async fn resolve(rpc_url: &str, name: &str, raw: bool) -> anyhow::Result<()> {
    let v = rpc(rpc_url, "boing_resolveName", json!([name])).await?;
    if raw {
        println!("{}", serde_json::to_string_pretty(&v)?);
    } else {
        print_record(&v);
    }
    Ok(())
}

pub async fn reverse(rpc_url: &str, address: &str, raw: bool) -> anyhow::Result<()> {
    let addr = parse_account(address)?;
    let v = rpc(rpc_url, "boing_reverseName", json!([hex_account(&addr)])).await?;
    if raw {
        println!("{}", serde_json::to_string_pretty(&v)?);
    } else if v.is_null() {
        println!("  (no verified primary name)");
    } else {
        println!("  {}", v["display"].as_str().unwrap_or("?"));
    }
    Ok(())
}

pub async fn info(rpc_url: &str, name: &str) -> anyhow::Result<()> {
    let v = rpc(rpc_url, "boing_getNameInfo", json!([name])).await?;
    println!("{}", serde_json::to_string_pretty(&v)?);
    Ok(())
}

pub async fn check(rpc_url: &str, names: &[String]) -> anyhow::Result<()> {
    let v = rpc(rpc_url, "boing_checkNames", json!([names])).await?;
    for r in v["results"].as_array().cloned().unwrap_or_default() {
        let label = r
            .get("display")
            .and_then(Value::as_str)
            .unwrap_or_else(|| r["input"].as_str().unwrap_or("?"));
        if r["valid"] != true {
            println!(
                "  {label:<36} invalid: {}",
                r["error"].as_str().unwrap_or("?")
            );
        } else if r["available"] == true {
            println!(
                "  {label:<36} available, {} BOING/year",
                r["rent_per_year"].as_str().unwrap_or("?")
            );
        } else if r["reserved"] == true {
            println!("  {label:<36} reserved");
        } else {
            println!(
                "  {label:<36} taken ({})",
                r["status"].as_str().unwrap_or("?")
            );
        }
    }
    Ok(())
}

pub async fn owned(rpc_url: &str, address: &str) -> anyhow::Result<()> {
    let addr = parse_account(address)?;
    let v = rpc(rpc_url, "boing_namesOwnedBy", json!([hex_account(&addr)])).await?;
    let list = v.as_array().cloned().unwrap_or_default();
    if list.is_empty() {
        println!("  (no names)");
    }
    for r in list {
        println!(
            "  {:<36} → {} ({}, expires at {})",
            r["display"].as_str().unwrap_or("?"),
            r["target"].as_str().unwrap_or("?"),
            r["status"].as_str().unwrap_or("?"),
            r["expires_at"]
        );
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn claim(
    rpc_url: &str,
    key: &SigningKey,
    name: &str,
    target: Option<String>,
    years: u8,
    primary: bool,
    nonce: Option<u64>,
    dry_run: bool,
) -> anyhow::Result<()> {
    let label = label_of(name)?;
    let me = account_of(key);
    let target = match target {
        Some(t) => parse_account(&t)?,
        None => me,
    };
    let rent = name_rent(&label, years);
    let info = rpc(rpc_url, "boing_getNameInfo", json!([label])).await?;
    println!(
        "Claim {} for {} year(s): rent {} BOING (burned) + network fees",
        display_name(&label),
        years,
        rent
    );
    println!("  owner  {}", hex_account(&me));
    println!("  target {}", hex_account(&target));
    if info["registry"]["enabled"] != true {
        anyhow::bail!("the name registry is not enabled on this node");
    }
    if info["registry"]["active"] != true {
        println!(
            "  note: the registry activates at height {} (head {})",
            info["registry"]["activation_height"], info["head_height"]
        );
    }
    if info["valid"] != true {
        anyhow::bail!("invalid name: {}", info["error"].as_str().unwrap_or("?"));
    }
    if info["available"] != true {
        anyhow::bail!(
            "{} is not available{}",
            display_name(&label),
            if info["reserved"] == true {
                " (reserved)"
            } else {
                ""
            }
        );
    }
    let bal = rpc(rpc_url, "boing_getBalance", json!([hex_account(&me)])).await?;
    let balance: u128 = bal["balance"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    println!("  balance {balance} BOING");
    if balance < rent + 3 {
        anyhow::bail!(
            "insufficient balance: need about {} BOING (rent + fees)",
            rent + 3
        );
    }
    let salt = fresh_salt(key, &label);
    let n0 = nonce_for(rpc_url, key, nonce).await?;
    let commit = build_commit_tx(key, n0, &label, &salt);
    if dry_run {
        println!(
            "Dry run: step 1/{} NameCommit (nothing submitted)",
            if primary { 3 } else { 2 }
        );
        submit(rpc_url, &commit, true).await?;
        println!(
            "  then NameRegister (nonce {}) after the commit is included{}",
            n0 + 1,
            if primary { ", then NameSetPrimary" } else { "" }
        );
        return Ok(());
    }
    println!("Step 1: commit");
    submit(rpc_url, &commit, false).await?;
    wait_for_receipt(rpc_url, &commit, Duration::from_secs(180)).await?;
    println!("Step 2: register");
    let reg = build_register_tx(key, n0 + 1, &label, salt, target, years);
    // The reveal must land at least `commit_min_age` blocks after the commit; retry briefly if early.
    let mut tries = 0;
    loop {
        match submit(rpc_url, &reg, false).await {
            Ok(()) => break,
            Err(e) if e.to_string().contains("too new") && tries < 20 => {
                tries += 1;
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(e) => return Err(e),
        }
    }
    wait_for_receipt(rpc_url, &reg, Duration::from_secs(180)).await?;
    if primary {
        if target != me {
            println!("  skipping primary: the name targets another address");
        } else {
            println!("Step 3: set primary name");
            let sp = tx(
                key,
                n0 + 2,
                TransactionPayload::NameSetPrimary {
                    name: Some(label.clone()),
                },
            );
            submit(rpc_url, &sp, false).await?;
            wait_for_receipt(rpc_url, &sp, Duration::from_secs(180)).await?;
        }
    }
    println!("Registered {}", display_name(&label));
    let v = rpc(rpc_url, "boing_resolveName", json!([label])).await?;
    print_record(&v);
    Ok(())
}

pub async fn simple(
    rpc_url: &str,
    key: &SigningKey,
    payload: TransactionPayload,
    nonce: Option<u64>,
    dry_run: bool,
) -> anyhow::Result<()> {
    let n = nonce_for(rpc_url, key, nonce).await?;
    let stx = tx(key, n, payload);
    println!("{}", stx.tx.display_for_signing());
    submit(rpc_url, &stx, dry_run).await?;
    if !dry_run {
        wait_for_receipt(rpc_url, &stx, Duration::from_secs(180)).await?;
        println!("  included");
    }
    Ok(())
}

pub fn payload_renew(name: &str, years: u8) -> anyhow::Result<TransactionPayload> {
    Ok(TransactionPayload::NameRenew {
        name: label_of(name)?,
        years,
    })
}
pub fn payload_transfer(name: &str, to: &str) -> anyhow::Result<TransactionPayload> {
    Ok(TransactionPayload::NameTransfer {
        name: label_of(name)?,
        new_owner: parse_account(to)?,
    })
}
pub fn payload_set_target(name: &str, target: &str) -> anyhow::Result<TransactionPayload> {
    Ok(TransactionPayload::NameSetTarget {
        name: label_of(name)?,
        target: parse_account(target)?,
    })
}
pub fn payload_set_primary(name: Option<&str>) -> anyhow::Result<TransactionPayload> {
    Ok(TransactionPayload::NameSetPrimary {
        name: name.map(label_of).transpose()?,
    })
}
pub fn payload_release(name: &str) -> anyhow::Result<TransactionPayload> {
    Ok(TransactionPayload::NameRelease {
        name: label_of(name)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_matches_register_reveal() {
        let k = SigningKey::from_bytes(&[5; 32]);
        let salt = fresh_salt(&k, "alice");
        let c = build_commit_tx(&k, 0, "alice", &salt);
        let TransactionPayload::NameCommit { commitment } = c.tx.payload else {
            panic!()
        };
        assert_eq!(commitment, name_commitment("alice", &account_of(&k), &salt));
        let r = build_register_tx(&k, 1, "alice", salt, account_of(&k), 1);
        assert!(r.verify().is_ok());
        assert_ne!(fresh_salt(&k, "alice"), [0; 32]);
    }

    #[test]
    fn payload_builders_normalize() {
        let TransactionPayload::NameRenew { name, .. } = payload_renew("Alice.BOING", 1).unwrap()
        else {
            panic!()
        };
        assert_eq!(name, "alice");
        assert!(payload_release("-bad").is_err());
        let TransactionPayload::NameSetPrimary { name } = payload_set_primary(None).unwrap() else {
            panic!()
        };
        assert!(name.is_none());
    }
}
