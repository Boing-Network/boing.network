//! Height-gated (versioned) execution rules.
//!
//! A live chain must replay from genesis with the current binary, so a rule change that alters state
//! roots is never applied retroactively: each historical rule set is kept as an *era* that starts at a
//! height, and blocks below the next era's start execute with it. New chains (and mainnet) start with
//! no history and run the current rules from genesis.

use boing_tokenomics::{FeeRemainder, NetworkFeePolicy};

/// Fee rule in force from `from_height` (inclusive) until the next era.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeeEra {
    pub from_height: u64,
    pub policy: NetworkFeePolicy,
    pub remainder: FeeRemainder,
}

/// Ordered list of historical rule eras for one network.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExecutionHistory {
    /// Network this history belongs to (chain id); a node refuses to apply it elsewhere.
    pub chain_id: u64,
    /// Sorted by `from_height`. Heights at/after `current_from` use the executor's configured policy.
    pub fee_eras: Vec<FeeEra>,
    /// First height executed with the current rules (no era applies at/after it).
    pub current_from: u64,
    /// First height at which a failed tx (nonce matched) is charged the failed-tx fee (security review
    /// #3). Below it a failed tx only consumes its nonce, as the network did historically.
    pub failed_tx_fee_from: u64,
    /// First height whose txs must carry v2 (chain-id) signatures (security review #2). Mempool
    /// admission rule only: block bodies carry no signatures, so this never changes execution.
    pub tx_v2_from: u64,
    /// First height whose blocks must carry one valid signature per transaction, checked by every
    /// importing node (security review #16). Below it blocks carry none (historical encoding).
    pub block_sigs_from: u64,
    /// First height at which an open-validator registration proof must be the chain-bound v2
    /// message (security review #14). Below it the v1 proof (and v2, when the chain id is known)
    /// is accepted.
    pub registration_proof_v2_from: u64,
    /// First height of the 2026-10-17 security-review rules (#6 sender-keyed name commitments, #9 no
    /// parallel batch touching the fee recipient, #10 open-validator registry cap, #11 no deregistering
    /// while jailed). Below it the network replays its historical behaviour.
    pub security_v4_from: u64,
    /// First height whose ledger is in base units (1 BOING = 10^18). At exactly this height, before
    /// the block's txs, every native amount in state is multiplied by 10^18 once (see
    /// [`boing_state::StateStore::redenominate`]). Below it amounts are whole BOING. `0` = base units
    /// from genesis (no migration).
    pub base_units_from: u64,
}

impl ExecutionHistory {
    /// Fee era covering `height`, or `None` when the current rules apply.
    pub fn fee_era_at(&self, height: u64) -> Option<&FeeEra> {
        if height >= self.current_from {
            return None;
        }
        self.fee_eras.iter().rev().find(|e| e.from_height <= height)
    }

    pub fn failed_tx_fee_active(&self, height: u64) -> bool {
        height >= self.failed_tx_fee_from
    }

    pub fn security_v4_active(&self, height: u64) -> bool {
        height >= self.security_v4_from
    }

    pub fn denomination_at(&self, height: u64) -> boing_primitives::Denomination {
        if height >= self.base_units_from {
            boing_primitives::Denomination::BaseUnits
        } else {
            boing_primitives::Denomination::WholeBoing
        }
    }
}
