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
}
