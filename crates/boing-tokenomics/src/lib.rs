//! BOING Tokenomics — Sustainable Value Design
//!
//! Parameters for supply, emission, fees, and incentives.
//! See BOING-BLOCKCHAIN-DESIGN-PLAN.md for full design.

pub mod dapp_incentives;
pub mod fee_policy;

pub use fee_policy::{
    network_fee_policy_from_json, NetworkFeePolicy, GOVERNANCE_NETWORK_FEE_CONFIG_KEY,
};

use boing_primitives::{Account, AccountId, AccountState, TransactionPayload};
use boing_state::StateStore;

/// Max supply (1 billion BOING). Hard cap; no infinite inflation.
pub const MAX_SUPPLY: u128 = 1_000_000_000;

/// Target block time in seconds.
pub const BLOCK_TIME_SECS: u64 = 2;

/// Canonical 32-byte hex of [`PROTOCOL_TREASURY`] (`TREASURY` ASCII prefix + 0x01).
pub const PROTOCOL_TREASURY_HEX: &str =
    "5452454153555259000000000000000000000000000000000000000000000001";

/// Default fee split when no [`NetworkFeePolicy`] is supplied: 100% to treasury.
/// Runtime governance overrides these via [`NetworkFeePolicy`].
pub const FEE_VALIDATORS_BPS: u16 = 0;
/// Fee split: share to treasury (basis points). Default **100%**.
pub const FEE_TREASURY_BPS: u16 = 10_000;
/// Fee split: share to burn (basis points). Default **0%**.
pub const FEE_BURN_BPS: u16 = 0;

/// Share of the *emission formula* minted to the round proposer (independent of tx-fee split).
/// Kept at 70% of the year-N emission so changing fee BPS does not zero mining rewards.
pub const BLOCK_EMISSION_PROPOSER_BPS: u16 = 7_000;

/// Fixed gas price for fee market v0 (BOING per [`GAS_UNITS_PER_BOING`] gas).
/// Native balances are whole BOING (no 18-decimal wei). Fee market v0 still meters
/// execution in gas units; this price converts that meter into tokens.
pub const GAS_PRICE: u128 = 1;

/// Gas units that cost [`GAS_PRICE`] BOING (ceil). Matches `GAS_PER_TRANSFER` in
/// `boing-execution` so a simple transfer costs **1 BOING**.
pub const GAS_UNITS_PER_BOING: u64 = 21_000;

/// Protocol treasury AccountId (receives the treasury share of tx fees and pays QA voter rewards).
/// Distinct from the all-zero burn sink.
pub const PROTOCOL_TREASURY: AccountId = AccountId([
    0x54, 0x52, 0x45, 0x41, 0x53, 0x55, 0x52, 0x59, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 1,
]);

/// Burn sink AccountId (FEE_BURN_BPS of fees are sent here and never spent by protocol logic).
/// Distinct from the all-zero id so it does not collide with common test/dev accounts.
pub const FEE_BURN_SINK: AccountId = AccountId([
    b'B', b'U', b'R', b'N', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 1,
]);

/// Emission decay factor per year. Year N emission = Year 1 * DECAY^(N-1).
pub const EMISSION_DECAY: f64 = 0.85;
/// Year 1 annual inflation (basis points, 800 = 8%).
pub const EMISSION_YEAR_1_BPS: u16 = 800;
/// Long-term inflation floor (basis points, 100 = 1%).
pub const EMISSION_FLOOR_BPS: u16 = 100;

/// Typical validator commission range (basis points).
pub const VALIDATOR_COMMISSION_MIN_BPS: u16 = 500; // 5%
pub const VALIDATOR_COMMISSION_MAX_BPS: u16 = 1_000; // 10%

/// Minimum active stake to enter the stake-derived validator set (BOING units).
pub const MIN_VALIDATOR_STAKE: u128 = 10_000;

/// Blocks after `Unbond` before `ClaimUnbond` may move funds back to balance.
pub const UNBONDING_DELAY_BLOCKS: u64 = 100;

/// Fraction of active stake burned on detected consensus equivocation (basis points).
pub const EQUIVOCATION_SLASH_BPS: u16 = 5_000; // 50%

/// Blocks after an equivocation slash during which an appeal may be submitted.
pub const EQUIVOCATION_APPEAL_WINDOW_BLOCKS: u64 = 1_000;

/// Consecutive missed quorum votes before a liveness slash (multi-validator only).
pub const LIVENESS_MISS_THRESHOLD: u32 = 3;

/// Fraction of active stake burned on liveness slash (basis points) — milder than equivocation.
pub const LIVENESS_SLASH_BPS: u16 = 1_000; // 10%

/// Appeal window for liveness slashes (same length as equivocation for the thin MVP).
pub const LIVENESS_APPEAL_WINDOW_BLOCKS: u64 = EQUIVOCATION_APPEAL_WINDOW_BLOCKS;

/// How long a non-leader waits for a proposal (with local pending txs) before counting a leader miss.
/// Default: 3× [`BLOCK_TIME_SECS`].
pub const LIVENESS_LEADER_TIMEOUT_SECS: u64 = BLOCK_TIME_SECS.saturating_mul(3);

/// Slash active stake for consensus equivocation; burned amount is credited to [`FEE_BURN_SINK`].
/// Returns the amount slashed (0 if account missing or stake is 0).
pub fn slash_equivocation_stake(state: &mut StateStore, validator: &AccountId) -> u128 {
    let amount = {
        let Some(st) = state.get_mut(validator) else {
            return 0;
        };
        if st.stake == 0 {
            return 0;
        }
        let mut amount = st
            .stake
            .saturating_mul(EQUIVOCATION_SLASH_BPS as u128)
            / 10_000;
        if amount == 0 {
            amount = 1u128.min(st.stake);
        }
        st.stake = st.stake.saturating_sub(amount);
        amount
    };
    credit_account(state, FEE_BURN_SINK, amount);
    amount
}

/// Slash active stake for liveness failure; burned amount is credited to [`FEE_BURN_SINK`].
pub fn slash_liveness_stake(state: &mut StateStore, validator: &AccountId) -> u128 {
    let amount = {
        let Some(st) = state.get_mut(validator) else {
            return 0;
        };
        if st.stake == 0 {
            return 0;
        }
        let mut amount = st
            .stake
            .saturating_mul(LIVENESS_SLASH_BPS as u128)
            / 10_000;
        if amount == 0 {
            amount = 1u128.min(st.stake);
        }
        st.stake = st.stake.saturating_sub(amount);
        amount
    };
    credit_account(state, FEE_BURN_SINK, amount);
    amount
}

/// Reverse an equivocation slash: debit [`FEE_BURN_SINK`] and restore `amount` to the validator's stake.
/// Returns the amount actually restored (may be less if the burn sink is short).
pub fn restore_equivocation_slash(
    state: &mut StateStore,
    validator: &AccountId,
    amount: u128,
) -> u128 {
    if amount == 0 {
        return 0;
    }
    let available = state.get(&FEE_BURN_SINK).map(|s| s.balance).unwrap_or(0);
    let restore = amount.min(available);
    if restore == 0 {
        return 0;
    }
    if let Some(sink) = state.get_mut(&FEE_BURN_SINK) {
        sink.balance = sink.balance.saturating_sub(restore);
    }
    if let Some(st) = state.get_mut(validator) {
        st.stake = st.stake.saturating_add(restore);
    } else {
        state.insert(Account {
            id: *validator,
            state: AccountState {
                balance: 0,
                nonce: 0,
                stake: restore,
                ..Default::default()
            },
        });
    }
    restore
}

/// dApp incentive cap per epoch (governance parameter; placeholder).
pub const DAPP_CAP_PER_EPOCH: u128 = 50_000;

/// Blocks per year (approximate).
pub const BLOCKS_PER_YEAR: u64 = 365 * 24 * 3600 / BLOCK_TIME_SECS;

/// Split a fee using compile-time [`FEE_*_BPS`] defaults (treasury-first).
/// Remainder from integer division goes to the **treasury** share so parts sum to `fee`.
pub fn split_fee(fee: u128) -> (u128, u128, u128) {
    split_fee_with_policy(fee, &NetworkFeePolicy::treasury_default())
}

/// Split `fee` using an explicit [`NetworkFeePolicy`]. Remainder goes to treasury.
pub fn split_fee_with_policy(fee: u128, policy: &NetworkFeePolicy) -> (u128, u128, u128) {
    let validators = fee.saturating_mul(policy.fee_validators_bps as u128) / 10_000;
    let treasury = fee.saturating_mul(policy.fee_treasury_bps as u128) / 10_000;
    let burn = fee.saturating_mul(policy.fee_burn_bps as u128) / 10_000;
    let allocated = validators.saturating_add(treasury).saturating_add(burn);
    let treasury = treasury.saturating_add(fee.saturating_sub(allocated));
    (validators, treasury, burn)
}

/// Total native fee for a successful included tx: gas fee + extra fixed + optional transfer BPS.
pub fn network_fee_amount(
    gas_used: u64,
    payload: &TransactionPayload,
    policy: &NetworkFeePolicy,
) -> u128 {
    let gas_fee = fee_for_gas(gas_used);
    let bps_fee = match payload {
        TransactionPayload::Transfer { amount, .. } if policy.transfer_amount_bps > 0 => {
            amount.saturating_mul(policy.transfer_amount_bps as u128) / 10_000
        }
        _ => 0,
    };
    gas_fee
        .saturating_add(policy.extra_fixed_fee)
        .saturating_add(bps_fee)
}

/// Fee charged for `gas_used` at the fixed [`GAS_PRICE`].
///
/// `fee = ceil(gas_used × GAS_PRICE / GAS_UNITS_PER_BOING)`. Any non-zero gas pays
/// at least 1 BOING when `GAS_PRICE >= 1`.
pub fn fee_for_gas(gas_used: u64) -> u128 {
    if gas_used == 0 {
        return 0;
    }
    let num = (gas_used as u128).saturating_mul(GAS_PRICE);
    let den = GAS_UNITS_PER_BOING as u128;
    num.saturating_add(den.saturating_sub(1)) / den
}

pub(crate) fn credit_account(state: &mut StateStore, id: AccountId, amount: u128) {
    if amount == 0 {
        return;
    }
    match state.get_mut(&id) {
        Some(s) => s.balance = s.balance.saturating_add(amount),
        None => {
            state.insert(Account {
                id,
                state: AccountState {
                    balance: amount,
                    nonce: 0,
                    stake: 0,
                    ..Default::default()
                },
            });
        }
    }
}

/// Deduct `fee` from `sender` and credit validator / treasury / burn shares using default policy.
/// Returns an error if the sender cannot pay (fail closed).
pub fn charge_and_distribute_fee(
    state: &mut StateStore,
    sender: &AccountId,
    fee: u128,
    fee_recipient: &AccountId,
) -> Result<(), FeeError> {
    charge_and_distribute_fee_with_policy(
        state,
        sender,
        fee,
        fee_recipient,
        &NetworkFeePolicy::treasury_default(),
    )
}

/// Same as [`charge_and_distribute_fee`] with an explicit fee-split policy.
pub fn charge_and_distribute_fee_with_policy(
    state: &mut StateStore,
    sender: &AccountId,
    fee: u128,
    fee_recipient: &AccountId,
    policy: &NetworkFeePolicy,
) -> Result<(), FeeError> {
    if fee == 0 {
        return Ok(());
    }
    let sender_state = state.get_mut(sender).ok_or(FeeError::SenderMissing)?;
    if sender_state.balance < fee {
        return Err(FeeError::InsufficientBalance {
            have: sender_state.balance,
            need: fee,
        });
    }
    sender_state.balance -= fee;
    let (to_validators, to_treasury, to_burn) = split_fee_with_policy(fee, policy);
    credit_account(state, *fee_recipient, to_validators);
    credit_account(state, PROTOCOL_TREASURY, to_treasury);
    credit_account(state, FEE_BURN_SINK, to_burn);
    Ok(())
}

/// Pay QA pool voters from [`PROTOCOL_TREASURY`].
///
/// Each voter is owed `reward_each`. If the treasury cannot cover the full amount, remaining
/// balance is split evenly (integer division; remainder stays in the treasury). Returns total paid.
/// Does not fail the surrounding transaction.
pub fn pay_qa_counted_voters(
    state: &mut StateStore,
    voters: &[AccountId],
    reward_each: u128,
) -> u128 {
    if voters.is_empty() || reward_each == 0 {
        return 0;
    }
    let n = voters.len() as u128;
    let want = reward_each.saturating_mul(n);
    let available = state.get(&PROTOCOL_TREASURY).map(|s| s.balance).unwrap_or(0);
    if available == 0 {
        return 0;
    }
    let (per, total) = if available >= want {
        (reward_each, want)
    } else {
        let per = available / n;
        (per, per.saturating_mul(n))
    };
    if per == 0 {
        return 0;
    }
    if let Some(t) = state.get_mut(&PROTOCOL_TREASURY) {
        t.balance = t.balance.saturating_sub(total);
    }
    for voter in voters {
        credit_account(state, *voter, per);
    }
    total
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FeeError {
    #[error("sender account missing")]
    SenderMissing,
    #[error("insufficient balance for fee: have {have}, need {need}")]
    InsufficientBalance { have: u128, need: u128 },
}

/// Block emission reward (validators' share). Year N = Year 1 * EMISSION_DECAY^(N-1).
pub fn block_emission_validators(block_height: u64) -> u128 {
    if block_height == 0 {
        return 0;
    }
    let year = (block_height - 1) / BLOCKS_PER_YEAR;
    let decay = EMISSION_DECAY.powi(year as i32);
    let year1_per_block =
        (MAX_SUPPLY * EMISSION_YEAR_1_BPS as u128 / 10_000) / BLOCKS_PER_YEAR as u128;
    let floor_per_block =
        (MAX_SUPPLY * EMISSION_FLOOR_BPS as u128 / 10_000) / BLOCKS_PER_YEAR as u128;
    let emission = (year1_per_block as f64 * decay) as u128;
    emission.max(floor_per_block) * BLOCK_EMISSION_PROPOSER_BPS as u128 / 10_000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fee_for_gas_scales_meter_to_whole_boing() {
        assert_eq!(fee_for_gas(0), 0);
        assert_eq!(fee_for_gas(1), 1);
        assert_eq!(fee_for_gas(GAS_UNITS_PER_BOING), 1);
        assert_eq!(fee_for_gas(21_000), 1);
        // Reference fungible init is typically ~381k gas → 19 BOING, not 381_612.
        assert_eq!(fee_for_gas(381_612), 19);
    }

    #[test]
    fn test_block_emission_year1() {
        assert_eq!(block_emission_validators(0), 0);
        let r1 = block_emission_validators(1);
        assert!(r1 > 0);
        assert!(r1 < 100); // sanity: per-block reward small
    }

    #[test]
    fn split_fee_sums_to_input() {
        for fee in [0u128, 1, 10, 21_000, 100_000] {
            let (v, t, b) = split_fee(fee);
            assert_eq!(v + t + b, fee);
            if fee >= 10_000 {
                assert_eq!(v, 0);
                assert_eq!(t, fee);
                assert_eq!(b, 0);
            }
        }
    }

    #[test]
    fn charge_fee_distributes() {
        let sender = AccountId([1u8; 32]);
        let proposer = AccountId([2u8; 32]);
        let mut state = StateStore::new();
        state.insert(Account {
            id: sender,
            state: AccountState {
                balance: 100_000,
                nonce: 0,
                stake: 0,
                ..Default::default()
            },
        });
        let fee = 10_000u128;
        charge_and_distribute_fee(&mut state, &sender, fee, &proposer).unwrap();
        let (v, t, b) = split_fee(fee);
        assert_eq!(v + t + b, fee);
        assert_eq!(state.get(&sender).unwrap().balance, 100_000 - fee);
        assert_eq!(state.get(&proposer).map(|s| s.balance).unwrap_or(0), v);
        assert_eq!(state.get(&PROTOCOL_TREASURY).unwrap().balance, t);
        assert_eq!(state.get(&FEE_BURN_SINK).map(|s| s.balance).unwrap_or(0), b);
        assert_eq!(fee_for_gas(21_000), 1);
        assert_eq!(t, fee);
        assert_eq!(v, 0);
    }

    #[test]
    fn protocol_treasury_canonical_bytes() {
        assert_eq!(&PROTOCOL_TREASURY.0[..8], b"TREASURY");
        assert_eq!(PROTOCOL_TREASURY.0[31], 1);
        assert_eq!(
            PROTOCOL_TREASURY_HEX.to_ascii_lowercase(),
            "5452454153555259000000000000000000000000000000000000000000000001"
        );
    }

    #[test]
    fn pay_qa_voters_debits_treasury() {
        let voter = AccountId([3u8; 32]);
        let mut state = StateStore::new();
        credit_account(&mut state, PROTOCOL_TREASURY, 10);
        let paid = pay_qa_counted_voters(&mut state, &[voter], 1);
        assert_eq!(paid, 1);
        assert_eq!(state.get(&PROTOCOL_TREASURY).unwrap().balance, 9);
        assert_eq!(state.get(&voter).unwrap().balance, 1);
    }

    #[test]
    fn slash_equivocation_burns_half_stake() {
        let v = AccountId([9u8; 32]);
        let mut state = StateStore::new();
        state.insert(Account {
            id: v,
            state: AccountState {
                balance: 0,
                nonce: 0,
                stake: 10_000,
                ..Default::default()
            },
        });
        let burned = slash_equivocation_stake(&mut state, &v);
        assert_eq!(burned, 5_000);
        assert_eq!(state.get(&v).unwrap().stake, 5_000);
        assert_eq!(state.get(&FEE_BURN_SINK).unwrap().balance, 5_000);
    }

    #[test]
    fn restore_equivocation_returns_stake_from_burn_sink() {
        let v = AccountId([9u8; 32]);
        let mut state = StateStore::new();
        state.insert(Account {
            id: v,
            state: AccountState {
                balance: 0,
                nonce: 0,
                stake: 10_000,
                ..Default::default()
            },
        });
        let burned = slash_equivocation_stake(&mut state, &v);
        let restored = restore_equivocation_slash(&mut state, &v, burned);
        assert_eq!(restored, burned);
        assert_eq!(state.get(&v).unwrap().stake, 10_000);
        assert_eq!(state.get(&FEE_BURN_SINK).unwrap().balance, 0);
    }

    #[test]
    fn slash_liveness_burns_ten_percent_stake() {
        let v = AccountId([8u8; 32]);
        let mut state = StateStore::new();
        state.insert(Account {
            id: v,
            state: AccountState {
                balance: 0,
                nonce: 0,
                stake: 10_000,
                ..Default::default()
            },
        });
        let burned = slash_liveness_stake(&mut state, &v);
        assert_eq!(burned, 1_000);
        assert_eq!(state.get(&v).unwrap().stake, 9_000);
        assert_eq!(state.get(&FEE_BURN_SINK).unwrap().balance, 1_000);
    }
}
