//! `boing iface`: on-chain interface registry (`boing_execution::interface_registry`).
//!
//! A contract's deployer claims the contract in the registry and records the BLAKE3 hash and URI
//! of an interface JSON (`{"functions":[{selector,name,params,kind,...}]}`). Explorers fetch the
//! URI, check the hash, and check that the claimer is the contract's deployer.
//!
//! Signer: `--secret-hex` or env `BOING_SECRET_HEX` (never printed). `--dry-run` prints the plan
//! and signed tx hex without submitting.

use crate::pool::{build_pool_call_tx, Signer};
use crate::token::{
    account_of, hex_account, nonce_for, parse_account, resolve_signing_key, rpc, submit,
};
use boing_execution::{
    interface_blob_store_bytecode, INTERFACE_BLOB_STORE_CREATE2_SALT_V1,
    encode_iface_claim_asset_calldata, encode_iface_set_interface_calldata, interface_json_hash,
    interface_registry_bytecode, interface_registry_claimer_storage_key,
    interface_registry_field_storage_key, InterfaceRecord, IFACE_FIELD_HASH, IFACE_FIELD_PUBLISHER,
    IFACE_FIELD_URI0, IFACE_FIELD_URI_LEN, INTERFACE_REGISTRY_CREATE2_SALT_V1,
    INTERFACE_REGISTRY_MAX_URI_BYTES,
};
use boing_primitives::{
    create2_contract_address, AccessList, AccountId, SignedTransaction, Transaction,
    TransactionPayload,
};
use ed25519_dalek::SigningKey;
use serde_json::{json, Value};

/// Public testnet interface registry (CREATE2 from the canonical DEX deployer, salt `BOING_INTERFACE_REGISTRY_V1`).
pub const CANONICAL_TESTNET_INTERFACE_REGISTRY_HEX: &str =
    "0xbd7373b6a3830768b2cea3768bedc737b43d1f981123c83e7f0d75ebb310fb6c";

pub fn registry_address_for(deployer: &AccountId) -> AccountId {
    create2_contract_address(
        deployer,
        &INTERFACE_REGISTRY_CREATE2_SALT_V1,
        &interface_registry_bytecode(),
    )
}

pub fn build_registry_deploy_tx(key: &SigningKey, nonce: u64) -> SignedTransaction {
    let mut tx = Transaction {
        nonce,
        sender: account_of(key),
        payload: TransactionPayload::ContractDeployWithPurpose {
            bytecode: interface_registry_bytecode(),
            purpose_category: "dapp".to_string(),
            description_hash: None,
            create2_salt: Some(INTERFACE_REGISTRY_CREATE2_SALT_V1),
        },
        access_list: AccessList::default(),
    };
    tx.access_list = tx.suggested_parallel_access_list();
    SignedTransaction::new(tx, key)
}

/// Checks the JSON parses and has a `functions` array; returns its BLAKE3 hash.
pub fn check_interface_json(bytes: &[u8]) -> anyhow::Result<[u8; 32]> {
    let v: Value = serde_json::from_slice(bytes)
        .map_err(|e| anyhow::anyhow!("interface JSON does not parse: {e}"))?;
    let fns = v
        .get("functions")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("interface JSON needs a \"functions\" array"))?;
    for (i, f) in fns.iter().enumerate() {
        if f.get("selector")
            .and_then(Value::as_u64)
            .filter(|s| *s <= 255)
            .is_none()
        {
            anyhow::bail!("functions[{i}].selector must be an integer 0..=255");
        }
        if f.get("name")
            .and_then(Value::as_str)
            .map_or(true, str::is_empty)
        {
            anyhow::bail!("functions[{i}].name is required");
        }
    }
    Ok(interface_json_hash(bytes))
}

fn resolve_registry(arg: Option<&str>) -> anyhow::Result<AccountId> {
    parse_account(arg.unwrap_or(CANONICAL_TESTNET_INTERFACE_REGISTRY_HEX))
}

async fn storage_word(
    rpc_url: &str,
    registry: &AccountId,
    key: &[u8; 32],
) -> anyhow::Result<[u8; 32]> {
    let v = rpc(
        rpc_url,
        "boing_getContractStorage",
        json!([hex_account(registry), format!("0x{}", hex::encode(key))]),
    )
    .await?;
    let s = v.get("value").and_then(Value::as_str).unwrap_or("0x");
    let b = hex::decode(s.trim_start_matches("0x"))?;
    let mut w = [0u8; 32];
    if b.len() == 32 {
        w.copy_from_slice(&b);
    }
    Ok(w)
}

/// Reads the record via storage (no simulation needed). `Ok(None)` when no interface is published.
pub async fn read_record(
    rpc_url: &str,
    registry: &AccountId,
    contract: &AccountId,
) -> anyhow::Result<(AccountId, Option<InterfaceRecord>)> {
    let claimer = AccountId(
        storage_word(
            rpc_url,
            registry,
            &interface_registry_claimer_storage_key(contract),
        )
        .await?,
    );
    let hash = storage_word(
        rpc_url,
        registry,
        &interface_registry_field_storage_key(contract, IFACE_FIELD_HASH),
    )
    .await?;
    if hash == [0u8; 32] {
        return Ok((claimer, None));
    }
    let publisher = AccountId(
        storage_word(
            rpc_url,
            registry,
            &interface_registry_field_storage_key(contract, IFACE_FIELD_PUBLISHER),
        )
        .await?,
    );
    let len_w = storage_word(
        rpc_url,
        registry,
        &interface_registry_field_storage_key(contract, IFACE_FIELD_URI_LEN),
    )
    .await?;
    let len = (u64::from_be_bytes(len_w[24..32].try_into().unwrap()) as usize)
        .min(INTERFACE_REGISTRY_MAX_URI_BYTES);
    let mut uri = Vec::new();
    for i in 0..len.div_ceil(32) {
        let w = storage_word(
            rpc_url,
            registry,
            &interface_registry_field_storage_key(contract, IFACE_FIELD_URI0 + i as u8),
        )
        .await?;
        uri.extend_from_slice(&w);
    }
    uri.truncate(len);
    Ok((
        claimer,
        Some(InterfaceRecord {
            claimer,
            publisher,
            interface_hash: hash,
            uri: String::from_utf8_lossy(&uri).into_owned(),
        }),
    ))
}

pub async fn deploy(rpc_url: &str, s: Signer) -> anyhow::Result<()> {
    let key = resolve_signing_key(s.secret.clone())?;
    let addr = registry_address_for(&account_of(&key));
    println!("Interface registry (CREATE2, salt BOING_INTERFACE_REGISTRY_V1)");
    println!("  deployer {}", hex_account(&account_of(&key)));
    println!("  registry {}", hex_account(&addr));
    if !s.dry_run {
        if let Ok(v) = rpc(
            rpc_url,
            "boing_getContractCode",
            json!([hex_account(&addr)]),
        )
        .await
        {
            if !v.is_null() {
                println!("  already deployed; nothing to do");
                return Ok(());
            }
        }
    }
    let n = nonce_for(rpc_url, &key, s.nonce).await?;
    submit(rpc_url, &build_registry_deploy_tx(&key, n), s.dry_run).await
}

/// Built-in hosting: CREATE2 address of the interface blob store for `deployer`.
pub fn blob_store_address_for(deployer: &AccountId) -> AccountId {
    create2_contract_address(
        deployer,
        &INTERFACE_BLOB_STORE_CREATE2_SALT_V1,
        &interface_blob_store_bytecode(),
    )
}

pub fn build_blob_store_deploy_tx(key: &SigningKey, nonce: u64) -> SignedTransaction {
    let mut tx = Transaction {
        nonce,
        sender: account_of(key),
        payload: TransactionPayload::ContractDeployWithPurpose {
            bytecode: interface_blob_store_bytecode(),
            purpose_category: "dapp".to_string(),
            description_hash: None,
            create2_salt: Some(INTERFACE_BLOB_STORE_CREATE2_SALT_V1),
        },
        access_list: AccessList::default(),
    };
    tx.access_list = tx.suggested_parallel_access_list();
    SignedTransaction::new(tx, key)
}

/// Deploy the interface blob store (built-in hosting for interface JSON; CREATE2, salt BOING_IFACE_BLOB_STORE_V1).
pub async fn deploy_blob_store(rpc_url: &str, s: Signer) -> anyhow::Result<()> {
    let key = resolve_signing_key(s.secret.clone())?;
    let addr = blob_store_address_for(&account_of(&key));
    println!("Interface blob store (CREATE2, salt BOING_IFACE_BLOB_STORE_V1)");
    println!("  deployer {}", hex_account(&account_of(&key)));
    println!("  store    {}", hex_account(&addr));
    if !s.dry_run {
        if let Ok(v) = rpc(rpc_url, "boing_getContractCode", json!([hex_account(&addr)])).await {
            if !v.is_null() {
                println!("  already deployed; nothing to do");
                return Ok(());
            }
        }
    }
    let n = nonce_for(rpc_url, &key, s.nonce).await?;
    submit(rpc_url, &build_blob_store_deploy_tx(&key, n), s.dry_run).await
}

pub fn print_blob_store_bytecode() {
    println!("0x{}", hex::encode(interface_blob_store_bytecode()));
}

pub fn print_bytecode() {
    println!("0x{}", hex::encode(interface_registry_bytecode()));
}

pub fn hash_file(path: &str) -> anyhow::Result<()> {
    let bytes = std::fs::read(path)?;
    let h = check_interface_json(&bytes)?;
    println!("0x{}", hex::encode(h));
    Ok(())
}

pub async fn get(
    rpc_url: &str,
    registry: Option<&str>,
    contract: &str,
    json_out: bool,
) -> anyhow::Result<()> {
    let registry = resolve_registry(registry)?;
    let contract = parse_account(contract)?;
    let (claimer, rec) = read_record(rpc_url, &registry, &contract).await?;
    let unclaimed = claimer.0 == [0u8; 32];
    if json_out {
        let v = json!({
            "registry": hex_account(&registry),
            "contract": hex_account(&contract),
            "claimer": if unclaimed { Value::Null } else { json!(hex_account(&claimer)) },
            "interface": rec.as_ref().map(|r| json!({
                "publisher": hex_account(&r.publisher),
                "interface_hash": format!("0x{}", hex::encode(r.interface_hash)),
                "uri": r.uri,
            })),
        });
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }
    println!("  contract  {}", hex_account(&contract));
    println!(
        "  claimer   {}",
        if unclaimed {
            "(unclaimed)".to_string()
        } else {
            hex_account(&claimer)
        }
    );
    match rec {
        Some(r) => {
            println!("  publisher {}", hex_account(&r.publisher));
            println!("  hash      0x{}", hex::encode(r.interface_hash));
            println!("  uri       {}", r.uri);
        }
        None => println!("  interface (none published)"),
    }
    Ok(())
}

async fn wait_included(rpc_url: &str, stx: &SignedTransaction) -> anyhow::Result<()> {
    let id = format!("0x{}", hex::encode(stx.tx.id().0));
    for _ in 0..60 {
        if let Ok(r) = rpc(rpc_url, "boing_getTransactionReceipt", json!([id])).await {
            if !r.is_null() {
                if r.get("success").and_then(Value::as_bool) == Some(false) {
                    anyhow::bail!("tx {id} failed: {r}");
                }
                return Ok(());
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
    anyhow::bail!("tx {id} not included after 120s")
}

/// Claim (if needed) and record the interface for `contract`.
pub async fn publish(
    rpc_url: &str,
    s: Signer,
    registry: Option<&str>,
    contract: &str,
    json_path: &str,
    uri: &str,
) -> anyhow::Result<()> {
    let registry = resolve_registry(registry)?;
    let contract = parse_account(contract)?;
    let bytes = std::fs::read(json_path)?;
    let hash = check_interface_json(&bytes)?;
    let set_cd = encode_iface_set_interface_calldata(&contract, &hash, uri)
        .map_err(|e| anyhow::anyhow!(e))?;
    let key = resolve_signing_key(s.secret.clone())?;
    let me = account_of(&key);
    println!("Publish interface");
    println!("  registry  {}", hex_account(&registry));
    println!("  contract  {}", hex_account(&contract));
    println!("  publisher {}", hex_account(&me));
    println!("  hash      0x{}", hex::encode(hash));
    println!("  uri       {uri}");

    let offline = s.dry_run && s.nonce.is_some();
    let (claimer, existing) = if offline {
        (AccountId([0u8; 32]), None)
    } else {
        read_record(rpc_url, &registry, &contract).await?
    };
    if claimer.0 != [0u8; 32] && claimer != me {
        anyhow::bail!(
            "contract is already claimed by {} in this registry",
            hex_account(&claimer)
        );
    }
    if let Some(r) = &existing {
        if r.interface_hash == hash && r.uri == uri && r.publisher == me {
            println!("  already published with this hash and URI; nothing to do");
            return Ok(());
        }
    }
    let mut n = nonce_for(rpc_url, &key, s.nonce).await?;
    let mut txs = Vec::new();
    if claimer.0 == [0u8; 32] {
        println!("  step: claim_asset (nonce {n})");
        txs.push(build_pool_call_tx(
            &key,
            n,
            registry,
            encode_iface_claim_asset_calldata(&contract),
            &[],
        ));
        n += 1;
    }
    println!("  step: set_interface (nonce {n})");
    txs.push(build_pool_call_tx(&key, n, registry, set_cd, &[]));
    if s.dry_run {
        println!("Dry run (nothing submitted):");
    }
    for tx in &txs {
        submit(rpc_url, tx, s.dry_run).await?;
        if !s.dry_run {
            wait_included(rpc_url, tx).await?;
        }
    }
    if !s.dry_run {
        let (_, rec) = read_record(rpc_url, &registry, &contract).await?;
        match rec {
            Some(r) if r.interface_hash == hash && r.uri == uri => println!("  verified on-chain"),
            _ => anyhow::bail!("record not visible after inclusion"),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_validation_and_hash() {
        let ok = br#"{"functions":[{"selector":1,"name":"transfer","params":[]}]}"#;
        assert_eq!(check_interface_json(ok).unwrap(), interface_json_hash(ok));
        assert!(check_interface_json(b"{}").is_err());
        assert!(check_interface_json(br#"{"functions":[{"selector":300,"name":"x"}]}"#).is_err());
        assert!(check_interface_json(br#"{"functions":[{"selector":1}]}"#).is_err());
    }

    #[test]
    fn canonical_registry_matches_create2() {
        let deployer =
            parse_account("0x8079f30559c02c5fe087c9ad1bc3b751a29de35992f3f8431734243e825f696e")
                .unwrap();
        assert_eq!(
            hex_account(&registry_address_for(&deployer)),
            CANONICAL_TESTNET_INTERFACE_REGISTRY_HEX
        );
    }

    #[test]
    fn deploy_tx_uses_salt() {
        let key = SigningKey::from_bytes(&[7u8; 32]);
        let stx = build_registry_deploy_tx(&key, 0);
        match stx.tx.payload {
            TransactionPayload::ContractDeployWithPurpose { create2_salt, .. } => {
                assert_eq!(create2_salt, Some(INTERFACE_REGISTRY_CREATE2_SALT_V1))
            }
            _ => panic!("expected deploy"),
        }
    }
}
