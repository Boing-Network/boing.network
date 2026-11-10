//! `boing items` -- Boing reference **multi-token** (ERC-1155-style shared item types).
//!
//! One contract, many item types: every holder of item `7` holds the same item. Template and slot
//! layout: `boing_execution::reference_multi_token` / `docs/BOING-REFERENCE-MULTI-TOKEN.md`.
//! Deploy uses the `0xFD` init payload (admin = deployer), purpose `nft`. Balances are read with
//! `boing_getContractStorage` at the documented balance key. Signer handling matches `boing token`.

use boing_execution::{
    encode_mt_mint_batch_calldata, encode_mt_mint_calldata, encode_mt_set_item_metadata_calldata,
    encode_mt_transfer_admin_calldata, encode_mt_transfer_calldata,
    multi_token_balance_storage_key, multi_token_metadata_storage_key,
    multi_token_supply_storage_key, ref_multi_token_admin_key,
    reference_multi_token_deploy_bytecode, reference_multi_token_runtime_bytecode,
    MAX_REFERENCE_MULTI_TOKEN_BATCH,
};
use boing_primitives::{
    nonce_derived_contract_address, AccessList, AccountId, SignedTransaction, Transaction,
    TransactionPayload,
};
use ed25519_dalek::SigningKey;
use serde_json::{json, Value};

use crate::token::{
    account_of, build_call_tx, decode_balance_word, hex_account, nonce_for, parse_account,
    resolve_signing_key, rpc, submit, validate_metadata,
};

/// QA purpose category for multi-token deploys (existing `nft` category).
pub const ITEMS_PURPOSE: &str = "nft";

/// Signed deploy tx for the pinned multi-token template.
pub fn build_deploy_tx(
    key: &SigningKey,
    nonce: u64,
    name: Option<String>,
    symbol: Option<String>,
) -> SignedTransaction {
    let mut tx = Transaction {
        nonce,
        sender: account_of(key),
        payload: TransactionPayload::ContractDeployWithPurposeAndMetadata {
            bytecode: reference_multi_token_deploy_bytecode(),
            purpose_category: ITEMS_PURPOSE.to_string(),
            description_hash: None,
            asset_name: name,
            asset_symbol: symbol,
            create2_salt: None,
        },
        access_list: AccessList::default(),
    };
    tx.access_list = tx.suggested_parallel_access_list();
    SignedTransaction::new(tx, key)
}

/// Parse `item:amount` pairs (e.g. `1:10,2:5`).
pub fn parse_entries(s: &str) -> anyhow::Result<Vec<(u64, u128)>> {
    let mut out = Vec::new();
    for part in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (i, a) = part
            .split_once(':')
            .ok_or_else(|| anyhow::anyhow!("entry `{part}` must be item:amount"))?;
        let item: u64 = i
            .trim()
            .parse()
            .map_err(|e| anyhow::anyhow!("item `{i}`: {e}"))?;
        let amount: u128 = a
            .trim()
            .parse()
            .map_err(|e| anyhow::anyhow!("amount `{a}`: {e}"))?;
        if amount == 0 {
            anyhow::bail!("amount must be > 0 (entry `{part}`)");
        }
        out.push((item, amount));
    }
    if out.is_empty() || out.len() > MAX_REFERENCE_MULTI_TOKEN_BATCH as usize {
        anyhow::bail!(
            "need 1..={} entries, got {}",
            MAX_REFERENCE_MULTI_TOKEN_BATCH,
            out.len()
        );
    }
    Ok(out)
}

pub fn parse_hash32(s: &str) -> anyhow::Result<[u8; 32]> {
    let b = hex::decode(s.trim().trim_start_matches("0x"))
        .map_err(|e| anyhow::anyhow!("invalid hash hex: {e}"))?;
    b.try_into()
        .map_err(|_| anyhow::anyhow!("hash must be 32 bytes (64 hex chars)"))
}

pub async fn deploy(
    rpc_url: &str,
    secret: Option<String>,
    secret_file: Option<String>,
    name: Option<String>,
    symbol: Option<String>,
    nonce: Option<u64>,
    dry_run: bool,
) -> anyhow::Result<()> {
    validate_metadata(&name, &symbol)?;
    let key = resolve_signing_key(secret, secret_file)?;
    let me = account_of(&key);
    let n = nonce_for(rpc_url, &key, nonce).await?;
    let contract = nonce_derived_contract_address(&me, n);
    println!("Deploying Boing reference multi-token (template v1, purpose nft)");
    println!("  admin    {}", hex_account(&me));
    println!("  contract {}", hex_account(&contract));
    submit(rpc_url, &build_deploy_tx(&key, n, name, symbol), dry_run).await
}

#[allow(clippy::too_many_arguments)]
pub async fn call(
    rpc_url: &str,
    secret: Option<String>,
    secret_file: Option<String>,
    contract: &str,
    calldata: impl FnOnce(&AccountId) -> anyhow::Result<Vec<u8>>,
    nonce: Option<u64>,
    dry_run: bool,
) -> anyhow::Result<()> {
    let key = resolve_signing_key(secret, secret_file)?;
    let contract = parse_account(contract)?;
    let data = calldata(&account_of(&key))?;
    let n = nonce_for(rpc_url, &key, nonce).await?;
    submit(rpc_url, &build_call_tx(&key, n, contract, data), dry_run).await
}

pub fn mint_calldata(to: AccountId, item: u64, amount: u128) -> anyhow::Result<Vec<u8>> {
    if amount == 0 {
        anyhow::bail!("amount must be > 0");
    }
    Ok(encode_mt_mint_calldata(&to, item, amount))
}

pub fn transfer_calldata(to: AccountId, item: u64, amount: u128) -> anyhow::Result<Vec<u8>> {
    if amount == 0 {
        anyhow::bail!("amount must be > 0");
    }
    Ok(encode_mt_transfer_calldata(&to, item, amount))
}

pub fn mint_batch_calldata(to: AccountId, entries: &str) -> anyhow::Result<Vec<u8>> {
    Ok(encode_mt_mint_batch_calldata(&to, &parse_entries(entries)?))
}

pub fn set_metadata_calldata(item: u64, hash: &str) -> anyhow::Result<Vec<u8>> {
    Ok(encode_mt_set_item_metadata_calldata(
        item,
        &parse_hash32(hash)?,
    ))
}

pub fn transfer_admin_calldata(new_admin: &str) -> anyhow::Result<Vec<u8>> {
    let a = parse_account(new_admin)?;
    if a.0 == [0u8; 32] {
        anyhow::bail!("new admin must be non-zero");
    }
    Ok(encode_mt_transfer_admin_calldata(&a))
}

async fn storage_word(
    rpc_url: &str,
    contract: &AccountId,
    key: [u8; 32],
) -> anyhow::Result<String> {
    let r = rpc(
        rpc_url,
        "boing_getContractStorage",
        json!([hex_account(contract), format!("0x{}", hex::encode(key))]),
    )
    .await?;
    r.get("value")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("unexpected boing_getContractStorage result: {r}"))
}

pub async fn balance(rpc_url: &str, contract: &str, holder: &str, item: u64) -> anyhow::Result<()> {
    let (contract, holder) = (parse_account(contract)?, parse_account(holder)?);
    let w = storage_word(
        rpc_url,
        &contract,
        multi_token_balance_storage_key(&holder, item),
    )
    .await?;
    println!("{}", decode_balance_word(&w)?);
    Ok(())
}

pub async fn info(rpc_url: &str, contract: &str, item: Option<u64>) -> anyhow::Result<()> {
    let contract = parse_account(contract)?;
    let admin = storage_word(rpc_url, &contract, ref_multi_token_admin_key()).await?;
    println!("admin  {admin}");
    if let Some(item) = item {
        let s = storage_word(rpc_url, &contract, multi_token_supply_storage_key(item)).await?;
        let m = storage_word(rpc_url, &contract, multi_token_metadata_storage_key(item)).await?;
        println!("item   {item}");
        println!("supply {}", decode_balance_word(&s)?);
        println!("meta   {m}");
    }
    Ok(())
}

pub fn print_bytecode(runtime: bool) {
    let code = if runtime {
        reference_multi_token_runtime_bytecode()
    } else {
        reference_multi_token_deploy_bytecode()
    };
    println!("0x{}", hex::encode(code));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_entries_and_rejects_bad_input() {
        assert_eq!(parse_entries("1:10, 2:5").unwrap(), vec![(1, 10), (2, 5)]);
        assert!(parse_entries("").is_err());
        assert!(parse_entries("1:0").is_err());
        assert!(parse_entries("1-2").is_err());
        let many: Vec<String> = (0..17).map(|i| format!("{i}:1")).collect();
        assert!(parse_entries(&many.join(",")).is_err());
    }

    #[test]
    fn deploy_tx_uses_pinned_payload_and_nft_purpose() {
        let key = SigningKey::from_bytes(&[7u8; 32]);
        let stx = build_deploy_tx(&key, 0, Some("Items".into()), Some("ITM".into()));
        match &stx.tx.payload {
            TransactionPayload::ContractDeployWithPurposeAndMetadata {
                bytecode,
                purpose_category,
                ..
            } => {
                assert_eq!(bytecode, &reference_multi_token_deploy_bytecode());
                assert_eq!(purpose_category, "nft");
            }
            other => panic!("unexpected payload {other:?}"),
        }
    }
}
