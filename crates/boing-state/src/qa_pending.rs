//! On-chain pending QA pool items (Unsure deploys waiting for public votes).

use std::collections::{BTreeMap, BTreeSet};

use boing_primitives::{hasher, AccountId, Hash, QaPoolVoteKind, Transaction};
use serde::{Deserialize, Serialize};

/// One Unsure deploy registered in state until Allow (contract created) or Reject (dropped).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QaPendingRecord {
    pub deployer: AccountId,
    pub deploy_nonce: u64,
    pub tx: Transaction,
    pub votes: BTreeMap<AccountId, QaPoolVoteKind>,
    pub rewarded: BTreeSet<AccountId>,
    pub entered_height: u64,
}

impl QaPendingRecord {
    pub fn new(tx: Transaction, entered_height: u64) -> Self {
        Self {
            deployer: tx.sender,
            deploy_nonce: tx.nonce,
            tx,
            votes: BTreeMap::new(),
            rewarded: BTreeSet::new(),
            entered_height,
        }
    }

    pub fn commitment_hash(&self) -> Hash {
        let bytes = bincode::serialize(self).unwrap_or_default();
        let mut h = hasher();
        h.update(b"boing.qa_pending.v1");
        h.update(&bytes);
        let mut out = [0u8; 32];
        out.copy_from_slice(h.finalize().as_bytes());
        Hash(out)
    }

    /// SMT leaf key: domain-separated so it cannot collide with account ids.
    pub fn smt_key(subject: &Hash) -> [u8; 32] {
        let mut h = hasher();
        h.update(b"boing.qa_pending.key.v1");
        h.update(&subject.0);
        let mut out = [0u8; 32];
        out.copy_from_slice(h.finalize().as_bytes());
        out
    }
}

/// Counted Allow/Reject (and optional Abstain) voters that have not yet been paid.
pub fn unpaid_counted_voters(
    record: &QaPendingRecord,
    pay_abstain: bool,
) -> Vec<AccountId> {
    record
        .votes
        .iter()
        .filter(|(id, kind)| {
            if record.rewarded.contains(id) {
                return false;
            }
            match kind {
                QaPoolVoteKind::Allow | QaPoolVoteKind::Reject => true,
                QaPoolVoteKind::Abstain => pay_abstain,
            }
        })
        .map(|(id, _)| *id)
        .collect()
}
