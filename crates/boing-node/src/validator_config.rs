//! Parse validator CLI / env configuration for multi-validator testnets.

use boing_primitives::AccountId;
use ed25519_dalek::SigningKey;

/// Default single-validator AccountId used when no multi-validator config is set.
pub fn default_local_validator() -> AccountId {
    AccountId([1u8; 32])
}

/// Parse a 32-byte AccountId from hex (optional `0x` prefix).
pub fn parse_account_id_hex(s: &str) -> Result<AccountId, String> {
    let hex = s.trim().strip_prefix("0x").unwrap_or(s.trim());
    let bytes = hex::decode(hex).map_err(|e| format!("invalid hex AccountId: {e}"))?;
    if bytes.len() != 32 {
        return Err(format!(
            "AccountId must be 32 bytes (64 hex chars), got {}",
            bytes.len()
        ));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(AccountId(arr))
}

/// Parse comma-separated AccountId hex list.
pub fn parse_validators_csv(s: &str) -> Result<Vec<AccountId>, String> {
    let mut out = Vec::new();
    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        out.push(parse_account_id_hex(part)?);
    }
    if out.is_empty() {
        return Err("validator list is empty".into());
    }
    Ok(out)
}

/// Parse a 32-byte Ed25519 secret key from hex (optional `0x`).
pub fn parse_validator_signing_key(s: &str) -> Result<SigningKey, String> {
    let hex = s.trim().strip_prefix("0x").unwrap_or(s.trim());
    let bytes = hex::decode(hex).map_err(|e| format!("invalid hex validator key: {e}"))?;
    if bytes.len() != 32 {
        return Err(format!(
            "validator key must be 32 bytes (64 hex chars), got {}",
            bytes.len()
        ));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(SigningKey::from_bytes(&arr))
}

/// Opt-in stake-derived validator set refresh (see [`crate::node::StakeValidatorSetConfig`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StakeValidatorSetCli {
    pub top_n: usize,
    pub epoch_len: u64,
    pub min_stake: u128,
    /// First height whose epoch boundary may switch to the stake set (`0` = first boundary).
    pub activation_height: u64,
}

/// Parse `--validator-set` / `BOING_VALIDATOR_SET`: `static` (default), `stake` or `open`.
pub fn parse_validator_set_mode(s: &str) -> Result<&'static str, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "static" | "" => Ok("static"),
        "stake" => Ok("stake"),
        "open" => Ok("open"),
        other => Err(format!(
            "unknown validator-set mode '{other}' (expected static|stake|open)"
        )),
    }
}

/// Resolved validator-set mode from CLI / `BOING_VALIDATOR_SET`.
pub fn resolve_validator_set_mode(validator_set_cli: Option<&str>) -> Result<&'static str, String> {
    let mode_str = validator_set_cli
        .map(|s| s.to_string())
        .or_else(|| std::env::var("BOING_VALIDATOR_SET").ok())
        .unwrap_or_else(|| "static".into());
    parse_validator_set_mode(&mode_str)
}

/// Open validator-set options (anchors are filled in from the static `--validators` list by the caller).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenValidatorSetCli {
    pub params: boing_state::OpenValidatorParams,
    /// Milliseconds an anchor waits for an offline scheduled leader before proposing itself.
    pub fallback_timeout_ms: u64,
}

fn env_parse<T: std::str::FromStr>(key: &str) -> Option<T> {
    std::env::var(key).ok().and_then(|s| s.trim().parse().ok())
}

/// Resolve open-mode options. Returns `None` unless the mode is `open`. The activation height is
/// **required** (`--open-validator-activation-height` / `BOING_OPEN_VALIDATOR_ACTIVATION_HEIGHT`) so a
/// running chain can never switch implicitly.
///
/// Env knobs (defaults): `BOING_OPEN_VALIDATOR_BOND` (100), `BOING_OPEN_VALIDATOR_DEREGISTER_DELAY` (100),
/// `BOING_OPEN_VALIDATOR_EPOCH_LEN` (100), `BOING_OPEN_VALIDATOR_MAX` (20),
/// `BOING_OPEN_VALIDATOR_LIVENESS_WINDOW` (1000), `BOING_OPEN_VALIDATOR_MISS_THRESHOLD_BPS` (1000 = 10%),
/// `BOING_OPEN_VALIDATOR_MIN_SAMPLES` (10), `BOING_OPEN_VALIDATOR_JAIL_COOLDOWN` (500),
/// `BOING_OPEN_VALIDATOR_FALLBACK_MS` (3000).
pub fn resolve_open_validator_set_config(
    validator_set_cli: Option<&str>,
    activation_height_cli: Option<u64>,
) -> Result<Option<OpenValidatorSetCli>, String> {
    if resolve_validator_set_mode(validator_set_cli)? != "open" {
        return Ok(None);
    }
    let activation_height = activation_height_cli
        .or_else(|| env_parse("BOING_OPEN_VALIDATOR_ACTIVATION_HEIGHT"))
        .ok_or_else(|| {
            "open validator-set mode requires BOING_OPEN_VALIDATOR_ACTIVATION_HEIGHT (or --open-validator-activation-height)".to_string()
        })?;
    let d = boing_state::OpenValidatorParams::default();
    let params = boing_state::OpenValidatorParams {
        activation_height,
        anchors: Vec::new(),
        bond: env_parse("BOING_OPEN_VALIDATOR_BOND").unwrap_or(d.bond),
        deregister_delay: env_parse("BOING_OPEN_VALIDATOR_DEREGISTER_DELAY")
            .unwrap_or(d.deregister_delay),
        epoch_len: env_parse("BOING_OPEN_VALIDATOR_EPOCH_LEN").unwrap_or(d.epoch_len),
        max_open_validators: env_parse("BOING_OPEN_VALIDATOR_MAX").unwrap_or(d.max_open_validators),
        liveness_window: env_parse("BOING_OPEN_VALIDATOR_LIVENESS_WINDOW")
            .unwrap_or(d.liveness_window),
        miss_threshold_bps: env_parse("BOING_OPEN_VALIDATOR_MISS_THRESHOLD_BPS")
            .unwrap_or(d.miss_threshold_bps),
        min_liveness_samples: env_parse("BOING_OPEN_VALIDATOR_MIN_SAMPLES")
            .unwrap_or(d.min_liveness_samples),
        jail_cooldown: env_parse("BOING_OPEN_VALIDATOR_JAIL_COOLDOWN").unwrap_or(d.jail_cooldown),
    };
    if params.epoch_len == 0 {
        return Err("open validator epoch_len must be >= 1".into());
    }
    if params.bond == 0 {
        return Err("open validator bond must be >= 1".into());
    }
    if params.liveness_window == 0 || params.miss_threshold_bps > 10_000 {
        return Err(
            "open validator liveness window must be >= 1 and threshold <= 10000 bps".into(),
        );
    }
    Ok(Some(OpenValidatorSetCli {
        params,
        fallback_timeout_ms: env_parse("BOING_OPEN_VALIDATOR_FALLBACK_MS").unwrap_or(3_000),
    }))
}

/// Resolve stake-set options from CLI / env. Returns `None` unless mode is `stake`.
///
/// Env: `BOING_VALIDATOR_SET=stake`, `BOING_STAKE_VALIDATOR_TOP_N` (default 21),
/// `BOING_STAKE_VALIDATOR_EPOCH_LEN` (default 100), `BOING_STAKE_VALIDATOR_MIN_STAKE`
/// (default [`boing_tokenomics::MIN_VALIDATOR_STAKE`]).
pub fn resolve_stake_validator_set_config(
    validator_set_cli: Option<&str>,
    top_n_cli: Option<usize>,
    epoch_len_cli: Option<u64>,
    min_stake_cli: Option<u128>,
    activation_height_cli: Option<u64>,
) -> Result<Option<StakeValidatorSetCli>, String> {
    if resolve_validator_set_mode(validator_set_cli)? != "stake" {
        return Ok(None);
    }

    let top_n = top_n_cli
        .or_else(|| {
            std::env::var("BOING_STAKE_VALIDATOR_TOP_N")
                .ok()
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(21);
    let epoch_len = epoch_len_cli
        .or_else(|| {
            std::env::var("BOING_STAKE_VALIDATOR_EPOCH_LEN")
                .ok()
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(100);
    let min_stake = min_stake_cli
        .or_else(|| {
            std::env::var("BOING_STAKE_VALIDATOR_MIN_STAKE")
                .ok()
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(boing_tokenomics::MIN_VALIDATOR_STAKE);

    let activation_height = activation_height_cli
        .or_else(|| {
            std::env::var("BOING_STAKE_VALIDATOR_ACTIVATION_HEIGHT")
                .ok()
                .and_then(|s| s.trim().parse().ok())
        })
        .unwrap_or(0);

    if top_n == 0 {
        return Err("stake validator top_n must be >= 1".into());
    }
    if epoch_len == 0 {
        return Err("stake validator epoch_len must be >= 1".into());
    }
    if min_stake == 0 {
        return Err("stake validator min_stake must be >= 1".into());
    }

    Ok(Some(StakeValidatorSetCli {
        top_n,
        epoch_len,
        min_stake,
        activation_height,
    }))
}

/// Parse `--leader-election` / `BOING_LEADER_ELECTION`: `round_robin` (default) or `vrf`.
pub fn resolve_leader_election(
    leader_election_cli: Option<&str>,
) -> Result<boing_consensus::LeaderElection, String> {
    let s = leader_election_cli
        .map(|s| s.to_string())
        .or_else(|| std::env::var("BOING_LEADER_ELECTION").ok())
        .unwrap_or_else(|| "round_robin".into());
    match s.trim().to_ascii_lowercase().as_str() {
        "round_robin" | "round-robin" | "rr" | "" => {
            Ok(boing_consensus::LeaderElection::RoundRobin)
        }
        "vrf" => Ok(boing_consensus::LeaderElection::Vrf),
        other => Err(format!(
            "unknown leader-election mode '{other}' (expected round_robin|vrf)"
        )),
    }
}

/// Resolve validator set + local identity from CLI flags and env.
///
/// Precedence: CLI `--validators` / `--validator-key` over `BOING_VALIDATORS` / `BOING_VALIDATOR_KEY`.
/// If neither validators nor key is set, returns the single-validator default.
pub fn resolve_validator_config(
    validators_cli: Option<&str>,
    validator_key_cli: Option<&str>,
) -> Result<(Vec<AccountId>, AccountId, Option<SigningKey>), String> {
    resolve_validator_config_with(validators_cli, validator_key_cli, false)
}

/// Like [`resolve_validator_config`]; with `allow_local_outside_set` (open mode) the local key may be
/// absent from `--validators` (which then lists the anchors): the node follows until admitted on-chain.
pub fn resolve_validator_config_with(
    validators_cli: Option<&str>,
    validator_key_cli: Option<&str>,
    allow_local_outside_set: bool,
) -> Result<(Vec<AccountId>, AccountId, Option<SigningKey>), String> {
    let validators_str = validators_cli
        .map(|s| s.to_string())
        .or_else(|| std::env::var("BOING_VALIDATORS").ok());
    let key_str = validator_key_cli
        .map(|s| s.to_string())
        .or_else(|| std::env::var("BOING_VALIDATOR_KEY").ok());

    if validators_str.is_none() && key_str.is_none() {
        let local = default_local_validator();
        return Ok((vec![local], local, None));
    }

    let signing_key = match key_str.as_deref() {
        Some(s) => Some(parse_validator_signing_key(s)?),
        None => None,
    };

    let local = if let Some(ref key) = signing_key {
        AccountId(key.verifying_key().to_bytes())
    } else {
        default_local_validator()
    };

    let validators = if let Some(ref s) = validators_str {
        parse_validators_csv(s)?
    } else {
        vec![local]
    };

    if !validators.contains(&local) && !allow_local_outside_set {
        return Err(format!(
            "local validator {} is not in --validators / BOING_VALIDATORS",
            hex::encode(local.0)
        ));
    }

    Ok((validators, local, signing_key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_account_and_csv() {
        let a = parse_account_id_hex(&"11".repeat(32)).unwrap();
        assert_eq!(a.0[0], 0x11);
        let list =
            parse_validators_csv(&format!("{},{}", "11".repeat(32), "22".repeat(32))).unwrap();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn stake_mode_defaults() {
        assert!(
            resolve_stake_validator_set_config(Some("static"), None, None, None, None)
                .unwrap()
                .is_none()
        );
        let cfg = resolve_stake_validator_set_config(Some("stake"), None, None, None, None)
            .unwrap()
            .expect("stake mode");
        assert_eq!(cfg.top_n, 21);
        assert_eq!(cfg.epoch_len, 100);
        assert_eq!(cfg.min_stake, boing_tokenomics::MIN_VALIDATOR_STAKE);
        let cfg2 = resolve_stake_validator_set_config(
            Some("stake"),
            Some(5),
            Some(10),
            Some(1_000),
            Some(500),
        )
        .unwrap()
        .unwrap();
        assert_eq!(cfg2.top_n, 5);
        assert_eq!(cfg2.epoch_len, 10);
        assert_eq!(cfg2.min_stake, 1_000);
        assert_eq!(cfg2.activation_height, 500);
        assert_eq!(cfg.activation_height, 0);
    }

    #[test]
    fn open_mode_requires_activation_and_defaults() {
        assert!(resolve_open_validator_set_config(Some("static"), None)
            .unwrap()
            .is_none());
        assert!(resolve_open_validator_set_config(Some("stake"), None)
            .unwrap()
            .is_none());
        if std::env::var("BOING_OPEN_VALIDATOR_ACTIVATION_HEIGHT").is_err() {
            assert!(resolve_open_validator_set_config(Some("open"), None).is_err());
        }
        let cfg = resolve_open_validator_set_config(Some("open"), Some(5_000))
            .unwrap()
            .unwrap();
        assert_eq!(cfg.params.activation_height, 5_000);
        assert_eq!(cfg.params.bond, 100);
        assert_eq!(cfg.params.epoch_len, 100);
        assert_eq!(cfg.params.liveness_window, 1_000);
        assert_eq!(cfg.params.miss_threshold_bps, 1_000);
        assert!(
            resolve_stake_validator_set_config(Some("open"), None, None, None, None)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn open_mode_allows_local_outside_set() {
        let key = "07".repeat(32);
        let anchors = "11".repeat(32);
        assert!(resolve_validator_config(Some(&anchors), Some(&key)).is_err());
        let (v, local, k) =
            resolve_validator_config_with(Some(&anchors), Some(&key), true).unwrap();
        assert_eq!(v.len(), 1);
        assert!(!v.contains(&local));
        assert!(k.is_some());
    }

    #[test]
    fn leader_election_parse() {
        assert_eq!(
            resolve_leader_election(Some("round_robin")).unwrap(),
            boing_consensus::LeaderElection::RoundRobin
        );
        assert_eq!(
            resolve_leader_election(Some("vrf")).unwrap(),
            boing_consensus::LeaderElection::Vrf
        );
    }
}
