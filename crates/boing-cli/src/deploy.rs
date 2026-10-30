//! `boing deploy` — submit a `ContractDeployWithPurpose` from a bytecode file.
//!
//! Path may be a `.hex` / `.bin` / `.bytecode` file, or a project directory containing one of
//! `bytecode.hex`, `contract.hex`, `out/bytecode.hex`, `build/bytecode.hex`.

use std::path::{Path, PathBuf};

use boing_primitives::{AccessList, SignedTransaction, Transaction, TransactionPayload};
use ed25519_dalek::SigningKey;
use tracing::info;

use crate::token::{account_of, hex_account, nonce_for, resolve_signing_key, submit};

const DEFAULT_PURPOSE: &str = "dapp";

/// Load bytecode bytes from `path` (file or project directory).
pub fn load_bytecode(path: &str) -> anyhow::Result<(PathBuf, Vec<u8>)> {
    let p = Path::new(path);
    let file = if p.is_dir() {
        find_bytecode_in_dir(p)?
    } else if p.is_file() {
        p.to_path_buf()
    } else {
        anyhow::bail!(
            "deploy path not found: {path} (expected a bytecode file or a project directory)"
        );
    };
    let raw =
        std::fs::read(&file).map_err(|e| anyhow::anyhow!("cannot read {}: {e}", file.display()))?;
    let bytecode = decode_bytecode_bytes(&raw, &file)?;
    if bytecode.is_empty() {
        anyhow::bail!("{} is empty", file.display());
    }
    Ok((file, bytecode))
}

fn find_bytecode_in_dir(dir: &Path) -> anyhow::Result<PathBuf> {
    const CANDIDATES: &[&str] = &[
        "bytecode.hex",
        "contract.hex",
        "bytecode.bin",
        "contract.bin",
        "out/bytecode.hex",
        "build/bytecode.hex",
        "src/bytecode.hex",
    ];
    for rel in CANDIDATES {
        let c = dir.join(rel);
        if c.is_file() {
            return Ok(c);
        }
    }
    anyhow::bail!(
        "no bytecode file in {} (looked for bytecode.hex, contract.hex, out/bytecode.hex, …)",
        dir.display()
    );
}

fn decode_bytecode_bytes(raw: &[u8], path: &Path) -> anyhow::Result<Vec<u8>> {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if name.ends_with(".bin") || name.ends_with(".bytecode") {
        return Ok(raw.to_vec());
    }
    // Hex file (possibly with 0x / whitespace) or ambiguous extension: try hex first.
    let text = std::str::from_utf8(raw).unwrap_or("");
    let compact: String = text
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .trim_start_matches("0x")
        .trim_start_matches("0X")
        .to_string();
    if !compact.is_empty()
        && compact.len() % 2 == 0
        && compact.chars().all(|c| c.is_ascii_hexdigit())
    {
        return hex::decode(&compact)
            .map_err(|e| anyhow::anyhow!("invalid hex in {}: {e}", path.display()));
    }
    if name.ends_with(".hex") {
        anyhow::bail!("{} does not contain valid hex bytecode", path.display());
    }
    Ok(raw.to_vec())
}

pub fn build_deploy_tx(
    key: &SigningKey,
    nonce: u64,
    bytecode: Vec<u8>,
    purpose: &str,
) -> SignedTransaction {
    let mut tx = Transaction {
        nonce,
        sender: account_of(key),
        payload: TransactionPayload::ContractDeployWithPurpose {
            bytecode,
            purpose_category: purpose.to_string(),
            description_hash: None,
            create2_salt: None,
        },
        access_list: AccessList::default(),
    };
    tx.access_list = tx.suggested_parallel_access_list();
    SignedTransaction::new(tx, key)
}

pub async fn run(
    rpc_url: &str,
    path: &str,
    secret_hex: Option<String>,
    secret_file: Option<String>,
    purpose: String,
    nonce: Option<u64>,
    dry_run: bool,
) -> anyhow::Result<()> {
    let (file, bytecode) = load_bytecode(path)?;
    let key = resolve_signing_key(secret_hex, secret_file)?;
    let n = nonce_for(rpc_url, &key, nonce).await?;
    let stx = build_deploy_tx(&key, n, bytecode.clone(), &purpose);

    info!(
        "Deploying {} bytes from {} as {} (purpose={purpose})",
        bytecode.len(),
        file.display(),
        hex_account(&account_of(&key)),
    );
    println!(
        "✓ Deploying {} bytes from {}",
        bytecode.len(),
        file.display()
    );
    println!("  sender {}", hex_account(&account_of(&key)));
    println!("  purpose {purpose}");
    submit(rpc_url, &stx, dry_run).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_hex_with_prefix_and_whitespace() {
        let dir = std::env::temp_dir().join(format!(
            "boing-deploy-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("bytecode.hex");
        std::fs::write(&f, "0x00\n f3\n").unwrap();
        let (_, b) = load_bytecode(dir.to_str().unwrap()).unwrap();
        assert_eq!(b, vec![0x00, 0xf3]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn decode_raw_bin() {
        let dir = std::env::temp_dir().join(format!(
            "boing-deploy-bin-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("contract.bin");
        std::fs::write(&f, [0x60, 0x00, 0xf3]).unwrap();
        let (_, b) = load_bytecode(f.to_str().unwrap()).unwrap();
        assert_eq!(b, vec![0x60, 0x00, 0xf3]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
