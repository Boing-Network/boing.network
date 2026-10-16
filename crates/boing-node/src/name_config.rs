//! Name registry node config from the environment.
//!
//! - `BOING_NAME_REGISTRY_ACTIVATION_HEIGHT` (or `--name-registry-activation-height`): enables the
//!   registry from that height. Unset = registry off (name txs rejected).
//! - Optional overrides (all nodes must agree; meant for local testing): `BOING_NAME_BLOCKS_PER_YEAR`,
//!   `BOING_NAME_GRACE_BLOCKS`, `BOING_NAME_COMMIT_MIN_AGE`, `BOING_NAME_COMMIT_MAX_AGE`.

use boing_state::NameRegistryParams;

fn env_u64(key: &str) -> Result<Option<u64>, String> {
    match std::env::var(key) {
        Ok(v) if !v.trim().is_empty() => v
            .trim()
            .parse::<u64>()
            .map(Some)
            .map_err(|_| format!("{key} must be an unsigned integer")),
        _ => Ok(None),
    }
}

pub fn resolve_name_registry_params(
    cli_activation: Option<u64>,
) -> Result<Option<NameRegistryParams>, String> {
    let Some(activation_height) =
        cli_activation.or(env_u64("BOING_NAME_REGISTRY_ACTIVATION_HEIGHT")?)
    else {
        return Ok(None);
    };
    let mut p = NameRegistryParams {
        activation_height,
        ..Default::default()
    };
    if let Some(v) = env_u64("BOING_NAME_BLOCKS_PER_YEAR")? {
        p.blocks_per_year = v.max(1);
    }
    if let Some(v) = env_u64("BOING_NAME_GRACE_BLOCKS")? {
        p.grace_blocks = v;
    }
    if let Some(v) = env_u64("BOING_NAME_COMMIT_MIN_AGE")? {
        p.commit_min_age = v;
    }
    if let Some(v) = env_u64("BOING_NAME_COMMIT_MAX_AGE")? {
        p.commit_max_age = v;
    }
    if p.commit_max_age < p.commit_min_age {
        return Err("BOING_NAME_COMMIT_MAX_AGE must be >= BOING_NAME_COMMIT_MIN_AGE".into());
    }
    Ok(Some(p))
}
