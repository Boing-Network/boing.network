//! Media blob activation: `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT` or CLI.
//!
//! Unset = feature off (PutBlob rejected). All validators must agree on the same height.
//!
//! Phase 3 header commitment: `BOING_BLOBS_ROOT_HEADER_ACTIVATION_HEIGHT` (independent opt-in).
//!
//! Phase 7 gated cap raise: `BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT` (unset = stay 16 MiB).

fn parse_optional_u64_env(name: &str) -> Result<Option<u64>, String> {
    match std::env::var(name) {
        Ok(s) => {
            let s = s.trim();
            if s.is_empty() {
                return Ok(None);
            }
            let h: u64 = s.parse().map_err(|_| format!("invalid {name}: {s}"))?;
            Ok(Some(h))
        }
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Resolve activation height from CLI override or env. `None` = media blobs disabled.
pub fn resolve_media_blobs_activation(cli: Option<u64>) -> Result<Option<u64>, String> {
    if let Some(h) = cli {
        return Ok(Some(h));
    }
    parse_optional_u64_env("BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT")
}

/// Resolve Phase 3 header `blobs_root` activation. `None` = field omitted (default).
pub fn resolve_blobs_root_header_activation(cli: Option<u64>) -> Result<Option<u64>, String> {
    if let Some(h) = cli {
        return Ok(Some(h));
    }
    parse_optional_u64_env("BOING_BLOBS_ROOT_HEADER_ACTIVATION_HEIGHT")
}

/// Resolve the storage-tree v2 activation height (**`BOING_STORAGE_TREE_V2_ACTIVATION_HEIGHT`**).
/// `None` = off (default): storage trees keep the legacy 64-bit routing and every state root is
/// unchanged.
pub fn resolve_storage_tree_v2_activation(cli: Option<u64>) -> Result<Option<u64>, String> {
    if let Some(h) = cli {
        return Ok(Some(h));
    }
    parse_optional_u64_env("BOING_STORAGE_TREE_V2_ACTIVATION_HEIGHT")
}

/// Resolve the account-tree v3 activation height (**`BOING_ACCOUNT_TREE_V3_ACTIVATION_HEIGHT`**).
/// `None` = off (default): the account tree keeps the legacy 64-bit routing and every state root
/// is unchanged.
pub fn resolve_account_tree_v3_activation(cli: Option<u64>) -> Result<Option<u64>, String> {
    if let Some(h) = cli {
        return Ok(Some(h));
    }
    parse_optional_u64_env("BOING_ACCOUNT_TREE_V3_ACTIVATION_HEIGHT")
}

/// Resolve Phase 7 raised Put/Announce cap activation. `None` = keep **16 MiB** (Fly-safe).
pub fn resolve_media_blobs_phase7_cap_activation(cli: Option<u64>) -> Result<Option<u64>, String> {
    if let Some(h) = cli {
        return Ok(Some(h));
    }
    parse_optional_u64_env(boing_primitives::PHASE7_CAP_ACTIVATION_ENV)
}

/// Resolve Phase 32 BLS multi-pin committee flip. `None` = keep Ed25519 tx **32** (Phase 31).
pub fn resolve_media_blobs_bls_committee_flip_activation(
    cli: Option<u64>,
) -> Result<Option<u64>, String> {
    if let Some(h) = cli {
        return Ok(Some(h));
    }
    parse_optional_u64_env(boing_primitives::PHASE32_BLS_COMMITTEE_FLIP_ENV)
}

/// Resolve governance consensus-tx activation (`BOING_GOVERNANCE_CONSENSUS_TX_ACTIVATION_HEIGHT`).
/// Supports `never` / `off` / empty → `None`. CLI override wins when set.
pub fn resolve_governance_consensus_tx_activation(cli: Option<u64>) -> Result<Option<u64>, String> {
    if let Some(h) = cli {
        return Ok(Some(h));
    }
    let raw =
        match std::env::var(boing_governance::consensus_tx::GOVERNANCE_CONSENSUS_TX_ACTIVATION_ENV)
        {
            Ok(s) => Some(s),
            Err(std::env::VarError::NotPresent) => None,
            Err(e) => return Err(e.to_string()),
        };
    boing_governance::consensus_tx::parse_activation_height(raw.as_deref())
}

/// Resolve Blake3 opcode activation (`BOING_VM_BLAKE3_OPCODE_ACTIVATION_HEIGHT`).
/// Supports `never` / `off` / empty → `None`.
pub fn resolve_blake3_opcode_activation(cli: Option<u64>) -> Result<Option<u64>, String> {
    if let Some(h) = cli {
        return Ok(Some(h));
    }
    let raw = match std::env::var(boing_execution::BLAKE3_OPCODE_ACTIVATION_ENV) {
        Ok(s) => Some(s),
        Err(std::env::VarError::NotPresent) => None,
        Err(e) => return Err(e.to_string()),
    };
    boing_execution::parse_blake3_activation_height(raw.as_deref())
}
