//! Governance-tunable native fee split and extra protocol levy.
//!
//! Loaded from `network_fee_config.json` / `boing_operatorApplyFeePolicy`. Apply path uses
//! [`crate::charge_and_distribute_fee_with_policy`].

use serde::{Deserialize, Serialize};

/// Governance JSON for native transaction fees (`target_key` [`GOVERNANCE_NETWORK_FEE_CONFIG_KEY`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkFeePolicy {
    /// Share of the charged fee credited to the block proposer (basis points, 10_000 = 100%).
    #[serde(default = "default_validators_bps")]
    pub fee_validators_bps: u16,
    /// Share credited to [`crate::PROTOCOL_TREASURY`].
    #[serde(default = "default_treasury_bps")]
    pub fee_treasury_bps: u16,
    /// Share credited to [`crate::FEE_BURN_SINK`].
    #[serde(default = "default_burn_bps")]
    pub fee_burn_bps: u16,
    /// Extra whole-BOING levy on every successful included transaction (in addition to gas).
    #[serde(default, with = "u128_string")]
    pub extra_fixed_fee: u128,
    /// Extra levy as basis points of a `Transfer` amount (0 = off). Other payloads ignore this.
    #[serde(default)]
    pub transfer_amount_bps: u16,
    /// Independent **blob data fee** (ledger units per KiB of PutBlob / BlobAnnounce size).
    /// `0` = off (size still metered via blob gas). Separates media congestion from execution gas.
    #[serde(default, with = "u128_string")]
    pub blob_base_fee_per_kib: u128,
    /// Extra Forever **pin fee** (ledger units per KiB) charged only when retention is Forever.
    /// Minimal on-chain pin-rent path; `0` = off. Prunable blobs are unaffected.
    #[serde(default, with = "u128_string")]
    pub blob_forever_pin_fee_per_kib: u128,
    /// Share of the **execution gas** schedule that is actually charged (basis points).
    /// `10_000` = full historical gas fee; `0` = waive execution gas (free-by-default path).
    /// Does **not** waive `extra_fixed_fee`, transfer BPS, blob data/pin fees, or priority tips.
    #[serde(default = "default_execution_gas_fee_bps")]
    pub execution_gas_fee_bps: u16,
    /// Minimum sender balance (ledger units) required to admit a **zero-tip** transaction when
    /// execution gas is waived. `0` = no balance gate beyond fee solvency. Anti-spam for free txs.
    #[serde(default, with = "u128_string")]
    pub free_tx_min_balance: u128,
}

fn default_execution_gas_fee_bps() -> u16 {
    10_000
}

fn default_validators_bps() -> u16 {
    0
}

fn default_treasury_bps() -> u16 {
    10_000
}

fn default_burn_bps() -> u16 {
    0
}

/// JSON numbers larger than 2^53 must be strings; accept either.
mod u128_string {
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &u128, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u128, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum U128In {
            Str(String),
            U64(u64),
            I64(i64),
        }
        match U128In::deserialize(deserializer)? {
            U128In::Str(s) => s.parse().map_err(serde::de::Error::custom),
            U128In::U64(n) => Ok(n as u128),
            U128In::I64(n) => {
                if n < 0 {
                    Err(serde::de::Error::custom("extra_fixed_fee must be >= 0"))
                } else {
                    Ok(n as u128)
                }
            }
        }
    }
}

impl Default for NetworkFeePolicy {
    fn default() -> Self {
        Self::treasury_default()
    }
}

impl NetworkFeePolicy {
    /// Production default: 100% of native fees to the protocol treasury (gas + optional extras).
    /// Block *emission* still goes to the round proposer; this policy only covers tx fees.
    pub fn treasury_default() -> Self {
        Self {
            fee_validators_bps: default_validators_bps(),
            fee_treasury_bps: default_treasury_bps(),
            fee_burn_bps: default_burn_bps(),
            extra_fixed_fee: 0,
            transfer_amount_bps: 0,
            blob_base_fee_per_kib: 0,
            blob_forever_pin_fee_per_kib: 0,
            execution_gas_fee_bps: default_execution_gas_fee_bps(),
            free_tx_min_balance: 0,
        }
    }

    /// Free-by-default: waive execution gas; blob/pin/extras and tips still apply.
    /// Useful as the post-activation operator default (set via `network_fee_config.json`).
    pub fn optional_fees_default() -> Self {
        Self {
            execution_gas_fee_bps: 0,
            ..Self::treasury_default()
        }
    }

    /// Legacy 70/20/10 split used before treasury-first fees.
    pub fn legacy_proposer_split() -> Self {
        Self {
            fee_validators_bps: 7_000,
            fee_treasury_bps: 2_000,
            fee_burn_bps: 1_000,
            extra_fixed_fee: 0,
            transfer_amount_bps: 0,
            blob_base_fee_per_kib: 0,
            blob_forever_pin_fee_per_kib: 0,
            execution_gas_fee_bps: default_execution_gas_fee_bps(),
            free_tx_min_balance: 0,
        }
    }

    /// True when execution gas is fully waived (users pay tip / extras / blobs only).
    pub fn execution_gas_waived(&self) -> bool {
        self.execution_gas_fee_bps == 0
    }

    /// BPS fields must sum to 10_000. Extra levies are independent.
    pub fn validate(&self) -> Result<(), String> {
        let sum = self
            .fee_validators_bps
            .saturating_add(self.fee_treasury_bps)
            .saturating_add(self.fee_burn_bps);
        if sum != 10_000 {
            return Err(format!(
                "fee_validators_bps + fee_treasury_bps + fee_burn_bps must equal 10000 (got {sum})"
            ));
        }
        if self.transfer_amount_bps > 10_000 {
            return Err("transfer_amount_bps must be <= 10000".into());
        }
        if self.execution_gas_fee_bps > 10_000 {
            return Err("execution_gas_fee_bps must be <= 10000".into());
        }
        Ok(())
    }
}

/// Governance proposal key for [`NetworkFeePolicy`] JSON.
pub const GOVERNANCE_NETWORK_FEE_CONFIG_KEY: &str = "network_fee_config";

/// Deserialize fee policy from JSON bytes.
pub fn network_fee_policy_from_json(bytes: &[u8]) -> Result<NetworkFeePolicy, serde_json::Error> {
    serde_json::from_slice(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn treasury_default_is_100_percent_treasury() {
        let p = NetworkFeePolicy::treasury_default();
        p.validate().unwrap();
        assert_eq!(p.fee_treasury_bps, 10_000);
        assert_eq!(p.fee_validators_bps, 0);
        assert_eq!(p.fee_burn_bps, 0);
    }

    #[test]
    fn json_roundtrip_string_u128() {
        let raw = br#"{
            "fee_validators_bps": 0,
            "fee_treasury_bps": 10000,
            "fee_burn_bps": 0,
            "extra_fixed_fee": "2",
            "transfer_amount_bps": 5
        }"#;
        let p = network_fee_policy_from_json(raw).unwrap();
        assert_eq!(p.extra_fixed_fee, 2);
        assert_eq!(p.transfer_amount_bps, 5);
        assert_eq!(p.execution_gas_fee_bps, 10_000, "omitted field keeps full gas fee");
    }

    #[test]
    fn optional_fees_default_waives_execution_gas() {
        let p = NetworkFeePolicy::optional_fees_default();
        p.validate().unwrap();
        assert!(p.execution_gas_waived());
        assert_eq!(p.blob_base_fee_per_kib, 0);
    }
}
