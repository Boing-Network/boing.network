//! Deterministic stake-based validator selection (opt-in, `--validator-set stake`).
//!
//! Every node must derive the **same** set from the same committed state, so:
//! - eligibility is `stake >= min_stake` (stake drops immediately on `Unbond` and on slashing);
//! - ranking is `stake` descending, ties broken by `AccountId` ascending, truncated to `top_n`;
//! - the resulting set is ordered by `AccountId` (round-robin leader order is the same everywhere);
//! - nothing node-local is mixed in (the old "always keep the local proposer" rule made sets diverge).
//!
//! The set changes only after committing an **epoch boundary** block: `height % epoch_len == 0` and
//! `height >= activation_height`. Before activation the static list (`--validators`) is used.
//! If nobody is eligible the **bootstrap** (static) list is used, so a chain whose operators have
//! not bonded keeps producing instead of halting.

use boing_primitives::AccountId;
use boing_state::StateStore;

/// Stake-set parameters (CLI `--stake-validator-*` / `BOING_STAKE_VALIDATOR_*`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StakeValidatorSetConfig {
    /// Maximum validators (at least 1).
    pub top_n: usize,
    /// Refresh after committing a block whose height is a multiple of `epoch_len`.
    pub epoch_len: u64,
    /// Minimum bonded `stake` to be eligible (default [`boing_tokenomics::MIN_VALIDATOR_STAKE`]).
    pub min_stake: u128,
    /// First height at which an epoch boundary may switch to the stake-derived set (`0` = from the first boundary).
    pub activation_height: u64,
    /// Static / genesis validator list: used before activation and whenever nobody is eligible.
    /// Empty = keep the current set when nobody is eligible (legacy behaviour).
    pub bootstrap: Vec<AccountId>,
}

impl Default for StakeValidatorSetConfig {
    fn default() -> Self {
        Self {
            top_n: 21,
            epoch_len: 100,
            min_stake: boing_tokenomics::MIN_VALIDATOR_STAKE,
            activation_height: 0,
            bootstrap: Vec::new(),
        }
    }
}

impl StakeValidatorSetConfig {
    /// Whether committing `height` triggers a validator-set refresh.
    #[must_use]
    pub fn is_refresh_height(&self, height: u64) -> bool {
        self.epoch_len > 0
            && self.top_n > 0
            && height > 0
            && height >= self.activation_height
            && height % self.epoch_len == 0
    }

    /// The most recent refresh height `<= height`, if any (used to restore after a restart).
    #[must_use]
    pub fn last_refresh_height(&self, height: u64) -> Option<u64> {
        if self.epoch_len == 0 || self.top_n == 0 {
            return None;
        }
        let h = height - height % self.epoch_len;
        (h > 0 && h >= self.activation_height).then_some(h)
    }
}

/// Eligible validators: `stake >= min_stake`, top `top_n` by (stake desc, id asc), returned sorted by id.
#[must_use]
pub fn eligible_stake_validators(
    state: &StateStore,
    top_n: usize,
    min_stake: u128,
) -> Vec<AccountId> {
    let mut all: Vec<(AccountId, u128)> = state
        .top_stakers(usize::MAX)
        .into_iter()
        .filter_map(|id| state.get(&id).map(|a| (id, a.stake)))
        .filter(|(_, s)| *s >= min_stake && *s > 0)
        .collect();
    all.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0 .0.cmp(&b.0 .0)));
    let mut out: Vec<AccountId> = all.into_iter().take(top_n).map(|(id, _)| id).collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// The set to use after an epoch boundary: eligible stakers, else the bootstrap list, else `current`.
#[must_use]
pub fn next_validator_set(
    state: &StateStore,
    cfg: &StakeValidatorSetConfig,
    current: &[AccountId],
) -> Vec<AccountId> {
    let eligible = eligible_stake_validators(state, cfg.top_n, cfg.min_stake);
    if !eligible.is_empty() {
        return eligible;
    }
    if !cfg.bootstrap.is_empty() {
        return cfg.bootstrap.clone();
    }
    current.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use boing_primitives::{Account, AccountState};

    fn id(b: u8) -> AccountId {
        AccountId([b; 32])
    }
    fn state(stakes: &[(u8, u128)]) -> StateStore {
        let mut st = StateStore::new();
        for (b, s) in stakes {
            st.insert(Account {
                id: id(*b),
                state: AccountState {
                    balance: 1,
                    stake: *s,
                    ..Default::default()
                },
            });
        }
        st
    }
    fn cfg(
        top_n: usize,
        epoch_len: u64,
        min_stake: u128,
        activation_height: u64,
        bootstrap: Vec<AccountId>,
    ) -> StakeValidatorSetConfig {
        StakeValidatorSetConfig {
            top_n,
            epoch_len,
            min_stake,
            activation_height,
            bootstrap,
        }
    }

    #[test]
    fn eligibility_is_min_stake_inclusive_and_excludes_below_and_zero() {
        let st = state(&[(1, 9_999), (2, 10_000), (3, 50_000), (4, 0)]);
        assert_eq!(
            eligible_stake_validators(&st, 21, 10_000),
            vec![id(2), id(3)]
        );
        assert!(eligible_stake_validators(&st, 21, 60_000).is_empty());
    }

    #[test]
    fn top_n_truncates_by_stake_with_deterministic_ties_and_sorted_output() {
        // 0x09 and 0x05 tie at 20k; id ascending wins the last seat.
        let st = state(&[(7, 30_000), (9, 20_000), (5, 20_000), (3, 10_000)]);
        assert_eq!(
            eligible_stake_validators(&st, 2, 10_000),
            vec![id(5), id(7)]
        );
        assert_eq!(
            eligible_stake_validators(&st, 3, 10_000),
            vec![id(5), id(7), id(9)]
        );
        // Same answer regardless of insertion order (HashMap iteration).
        let st2 = state(&[(3, 10_000), (5, 20_000), (9, 20_000), (7, 30_000)]);
        for n in 1..=4 {
            assert_eq!(
                eligible_stake_validators(&st, n, 1),
                eligible_stake_validators(&st2, n, 1)
            );
        }
    }

    #[test]
    fn refresh_heights_respect_epoch_and_activation() {
        let c = cfg(21, 100, 10_000, 250, vec![]);
        assert!(!c.is_refresh_height(0));
        assert!(!c.is_refresh_height(100));
        assert!(!c.is_refresh_height(200));
        assert!(!c.is_refresh_height(250));
        assert!(!c.is_refresh_height(299));
        assert!(c.is_refresh_height(300));
        assert!(c.is_refresh_height(400));
        assert_eq!(c.last_refresh_height(299), None);
        assert_eq!(c.last_refresh_height(399), Some(300));
        assert_eq!(c.last_refresh_height(400), Some(400));
        let c0 = cfg(21, 100, 10_000, 0, vec![]);
        assert!(c0.is_refresh_height(100));
        assert_eq!(c0.last_refresh_height(99), None);
    }

    #[test]
    fn single_validator_chain_without_stakers_falls_back_to_bootstrap() {
        let boot = vec![id(0xb0)];
        let c = cfg(21, 100, 10_000, 0, boot.clone());
        // Nobody bonded, bootstrap validator itself unbonded: keep producing with the static list.
        let st = state(&[(0xb0, 0), (1, 9_000)]);
        assert_eq!(next_validator_set(&st, &c, &[id(0xee)]), boot);
        // Bootstrap validator bonds the minimum: it is the stake set (still one validator, no halt).
        let st = state(&[(0xb0, 10_000)]);
        assert_eq!(next_validator_set(&st, &c, &boot), boot);
        // Legacy (no bootstrap configured): keep the current set.
        let legacy = cfg(21, 100, 10_000, 0, vec![]);
        assert_eq!(
            next_validator_set(&state(&[]), &legacy, &[id(9)]),
            vec![id(9)]
        );
    }

    #[test]
    fn join_and_leave_transitions() {
        let boot = vec![id(0xb0)];
        let c = cfg(3, 10, 10_000, 0, boot.clone());
        let mut st = state(&[(0xb0, 10_000)]);
        assert_eq!(next_validator_set(&st, &c, &boot), vec![id(0xb0)]);
        // A bonds 10k -> joins.
        st.insert(Account {
            id: id(0xa1),
            state: AccountState {
                stake: 10_000,
                ..Default::default()
            },
        });
        assert_eq!(next_validator_set(&st, &c, &boot), vec![id(0xa1), id(0xb0)]);
        // A unbonds 1 (stake drops immediately) -> leaves at the next boundary.
        st.get_mut(&id(0xa1)).unwrap().stake = 9_999;
        assert_eq!(next_validator_set(&st, &c, &boot), vec![id(0xb0)]);
        // Bootstrap validator unbonds fully while A is back: set is A only (bootstrap is a fallback, not a member).
        st.get_mut(&id(0xa1)).unwrap().stake = 20_000;
        st.get_mut(&id(0xb0)).unwrap().stake = 0;
        assert_eq!(next_validator_set(&st, &c, &boot), vec![id(0xa1)]);
        // Everyone gone -> bootstrap.
        st.get_mut(&id(0xa1)).unwrap().stake = 0;
        assert_eq!(next_validator_set(&st, &c, &boot), boot);
    }
}
