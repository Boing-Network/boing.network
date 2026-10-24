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
