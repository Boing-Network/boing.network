//! BOING Tokenomics — Sustainable Value Design
//!
//! Parameters for supply, emission, fees, and incentives.
//! See BOING-BLOCKCHAIN-DESIGN-PLAN.md for full design.

pub mod dapp_incentives;

use boing_primitives::{Account, AccountId, AccountState};
use boing_state::StateStore;

/// Max supply (1 billion BOING). Hard cap; no infinite inflation.
pub const MAX_SUPPLY: u128 = 1_000_000_000;

/// Target block time in seconds.
pub const BLOCK_TIME_SECS: u64 = 2;

/// Fee split: share to validators (basis points, 10000 = 100%).
pub const FEE_VALIDATORS_BPS: u16 = 7_000; // 70%
/// Fee split: share to treasury (basis points).
pub const FEE_TREASURY_BPS: u16 = 2_000; // 20%
/// Fee split: share to burn (basis points).
pub const FEE_BURN_BPS: u16 = 1_000; // 10%

/// Fixed gas price for fee market v0 (BOING per gas unit). Fee = gas_used × GAS_PRICE.
pub const GAS_PRICE: u128 = 1;

/// Protocol treasury AccountId (receives FEE_TREASURY_BPS of tx fees).
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

/// Split a fee into (validators, treasury, burn) using [`FEE_VALIDATORS_BPS`] / [`FEE_TREASURY_BPS`] / [`FEE_BURN_BPS`].
/// Any remainder from integer division is added to the validator share so the parts sum to `fee`.
pub fn split_fee(fee: u128) -> (u128, u128, u128) {
    let validators = fee.saturating_mul(FEE_VALIDATORS_BPS as u128) / 10_000;
    let treasury = fee.saturating_mul(FEE_TREASURY_BPS as u128) / 10_000;
    let burn = fee.saturating_mul(FEE_BURN_BPS as u128) / 10_000;
    let allocated = validators.saturating_add(treasury).saturating_add(burn);
    let validators = validators.saturating_add(fee.saturating_sub(allocated));
    (validators, treasury, burn)
}

/// Fee charged for `gas_used` at the fixed [`GAS_PRICE`].
pub fn fee_for_gas(gas_used: u64) -> u128 {
    (gas_used as u128).saturating_mul(GAS_PRICE)
}

fn credit_account(state: &mut StateStore, id: AccountId, amount: u128) {
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

/// Deduct `fee` from `sender` and credit validator / treasury / burn shares.
/// Returns an error if the sender cannot pay.
pub fn charge_and_distribute_fee(
    state: &mut StateStore,
    sender: &AccountId,
    fee: u128,
    fee_recipient: &AccountId,
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
    let (to_validators, to_treasury, to_burn) = split_fee(fee);
    credit_account(state, *fee_recipient, to_validators);
    credit_account(state, PROTOCOL_TREASURY, to_treasury);
    credit_account(state, FEE_BURN_SINK, to_burn);
    Ok(())
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
    emission.max(floor_per_block) * FEE_VALIDATORS_BPS as u128 / 10_000
}

#[cfg(test)]
mod tests {
    use super::*;

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
                assert_eq!(v, fee * 7_000 / 10_000);
                assert_eq!(t, fee * 2_000 / 10_000);
                assert_eq!(b, fee * 1_000 / 10_000);
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
        let fee = fee_for_gas(21_000);
        charge_and_distribute_fee(&mut state, &sender, fee, &proposer).unwrap();
        let (v, t, b) = split_fee(fee);
        assert_eq!(state.get(&sender).unwrap().balance, 100_000 - fee);
        assert_eq!(state.get(&proposer).unwrap().balance, v);
        assert_eq!(state.get(&PROTOCOL_TREASURY).unwrap().balance, t);
        assert_eq!(state.get(&FEE_BURN_SINK).unwrap().balance, b);
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
