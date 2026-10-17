//! On-chain name registry (consensus state).
//!
//! Mutated only by the `Name*` transactions (see `boing_primitives::names` for label rules, rent and
//! commitments) and committed into the state root as one SMT leaf once non-empty, so the state root of a
//! chain that never uses names is unchanged. All maps are `BTreeMap`s: iteration order (and therefore the
//! bincode commitment) is identical on every node.
//!
//! ## Lifecycle (heights are block heights)
//! - `NameCommit` stores `commitment -> (sender, height)`.
//! - `NameRegister` reveals `(label, salt)`; the commitment must be by the same sender and between
//!   `commit_min_age` and `commit_max_age` blocks old. The name is **available** if it never existed or
//!   its grace period is over. Expires at `height + years * blocks_per_year`.
//! - **Active** while `height < expires_at`; **grace** until `expires_at + grace_blocks` (does not resolve,
//!   only the owner may renew); **expired** afterwards (anyone may register it again).
//! - Reverse records (`address -> label`) are only set by the address itself and only count while the
//!   name is active and still targets that address; they are also cleared eagerly on transfer, re-target,
//!   release and re-registration.

use std::collections::BTreeMap;

use boing_primitives::{check_registrable, hasher, name_commitment, name_rent, AccountId};
use boing_primitives::{NAME_MAX_YEARS, NAME_MIN_YEARS};
use serde::{Deserialize, Serialize};

/// Node configuration. **Not** part of the state root, but every node must use the same values.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NameRegistryParams {
    /// Name transactions are rejected before this height.
    pub activation_height: u64,
    /// Blocks per registration year (2 s blocks: 15,768,000).
    pub blocks_per_year: u64,
    /// Blocks after expiry during which only the owner can renew.
    pub grace_blocks: u64,
    /// Minimum age of a commitment before it can be revealed.
    pub commit_min_age: u64,
    /// Maximum age of a commitment (older ones are rejected and pruned).
    pub commit_max_age: u64,
}

impl Default for NameRegistryParams {
    fn default() -> Self {
        Self {
            activation_height: 0,
            blocks_per_year: 15_768_000,
            grace_blocks: 1_296_000, // 30 days
            commit_min_age: 1,
            commit_max_age: 43_200, // 1 day
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NameRecord {
    pub owner: AccountId,
    pub target: AccountId,
    pub registered_at: u64,
    pub expires_at: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitRecord {
    pub sender: AccountId,
    pub height: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameStatus {
    Active,
    Grace,
    Expired,
}

impl NameStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            NameStatus::Active => "active",
            NameStatus::Grace => "grace",
            NameStatus::Expired => "expired",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum NameRegistryError {
    #[error("the name registry is not enabled on this network")]
    Disabled,
    #[error("the name registry activates at height {activation_height}")]
    NotYetActive { activation_height: u64 },
    #[error("invalid name: {0}")]
    InvalidName(String),
    #[error("years must be {NAME_MIN_YEARS}-{NAME_MAX_YEARS}")]
    InvalidYears,
    #[error("commitment already exists")]
    CommitExists,
    #[error("no matching commitment from this sender (send NameCommit first)")]
    CommitNotFound,
    #[error("commitment too new: reveal at or after height {ready_at}")]
    CommitTooNew { ready_at: u64 },
    #[error("commitment expired; commit again")]
    CommitTooOld,
    #[error("name is not available")]
    NotAvailable,
    #[error("name is not registered")]
    NotRegistered,
    #[error("only the owner can do this")]
    NotOwner,
    #[error("name has expired")]
    Expired,
    #[error("renewal would exceed {NAME_MAX_YEARS} years of prepaid time")]
    TooFarAhead,
    #[error("primary name must be active and resolve to the sender")]
    PrimaryMismatch,
    #[error("insufficient balance for rent ({rent} BOING)")]
    InsufficientBalance { rent: u128 },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NameRegistry {
    pub names: BTreeMap<String, NameRecord>,
    pub reverse: BTreeMap<AccountId, String>,
    pub commits: BTreeMap<[u8; 32], CommitRecord>,
}

impl NameRegistry {
    pub fn smt_key() -> [u8; 32] {
        let mut h = hasher();
        h.update(b"BOING_NAME_REGISTRY_V1");
        *h.finalize().as_bytes()
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty() && self.reverse.is_empty() && self.commits.is_empty()
    }

    pub fn commitment(&self) -> [u8; 32] {
        let mut h = hasher();
        h.update(b"BOING_NAME_REGISTRY_V1");
        h.update(&bincode::serialize(self).unwrap_or_default());
        *h.finalize().as_bytes()
    }

    pub fn status_of(rec: &NameRecord, params: &NameRegistryParams, height: u64) -> NameStatus {
        if height < rec.expires_at {
            NameStatus::Active
        } else if height < rec.expires_at.saturating_add(params.grace_blocks) {
            NameStatus::Grace
        } else {
            NameStatus::Expired
        }
    }

    pub fn status(
        &self,
        label: &str,
        params: &NameRegistryParams,
        height: u64,
    ) -> Option<NameStatus> {
        self.names
            .get(label)
            .map(|r| Self::status_of(r, params, height))
    }

    pub fn is_available(&self, label: &str, params: &NameRegistryParams, height: u64) -> bool {
        match self.names.get(label) {
            None => true,
            Some(r) => Self::status_of(r, params, height) == NameStatus::Expired,
        }
    }

    /// Active target of `label` (None if missing, in grace or expired).
    pub fn resolve(
        &self,
        label: &str,
        params: &NameRegistryParams,
        height: u64,
    ) -> Option<&NameRecord> {
        self.names
            .get(label)
            .filter(|r| Self::status_of(r, params, height) == NameStatus::Active)
    }

    /// Verified reverse lookup: the address's primary name, only if it is active and resolves back.
    pub fn reverse_verified(
        &self,
        addr: &AccountId,
        params: &NameRegistryParams,
        height: u64,
    ) -> Option<&str> {
        let label = self.reverse.get(addr)?;
        let rec = self.resolve(label, params, height)?;
        (rec.target == *addr).then_some(label.as_str())
    }

    /// Names owned by `owner` (any status except expired), sorted.
    pub fn owned_by(
        &self,
        owner: &AccountId,
        params: &NameRegistryParams,
        height: u64,
    ) -> Vec<(&String, &NameRecord)> {
        self.names
            .iter()
            .filter(|(_, r)| {
                r.owner == *owner && Self::status_of(r, params, height) != NameStatus::Expired
            })
            .collect()
    }

    fn clear_reverse_pointing_to(&mut self, addr: &AccountId, label: &str) {
        if self.reverse.get(addr).map(|l| l == label).unwrap_or(false) {
            self.reverse.remove(addr);
        }
    }

    fn prune_commits(&mut self, params: &NameRegistryParams, height: u64) {
        let max = params.commit_max_age;
        self.commits
            .retain(|_, c| height.saturating_sub(c.height) <= max);
    }

    /// Storage key of a sender-keyed commitment (security review #6). Before the v4 height commitments
    /// were stored under the raw hash, so anyone who copied a pending `NameCommit` hash could block it
    /// (`CommitExists`). Keying by `(sender, commitment)` keeps the map type (and bincode layout) unchanged.
    pub fn sender_commit_key(sender: &AccountId, commitment: &[u8; 32]) -> [u8; 32] {
        let mut h = hasher();
        h.update(b"BOING_NAME_COMMIT_V2");
        h.update(&sender.0);
        h.update(commitment);
        *h.finalize().as_bytes()
    }

    /// Legacy (pre-v4) commit: global key.
    pub fn commit(
        &mut self,
        params: &NameRegistryParams,
        height: u64,
        sender: AccountId,
        commitment: [u8; 32],
    ) -> Result<(), NameRegistryError> {
        self.commit_keyed(params, height, sender, commitment, false)
    }

    pub fn commit_keyed(
        &mut self,
        params: &NameRegistryParams,
        height: u64,
        sender: AccountId,
        commitment: [u8; 32],
        sender_keyed: bool,
    ) -> Result<(), NameRegistryError> {
        self.prune_commits(params, height);
        let key = if sender_keyed {
            Self::sender_commit_key(&sender, &commitment)
        } else {
            commitment
        };
        if self.commits.contains_key(&key) {
            return Err(NameRegistryError::CommitExists);
        }
        self.commits.insert(key, CommitRecord { sender, height });
        Ok(())
    }

    /// Finds the sender's record for `commitment`: the sender-keyed entry first (v4), then the legacy
    /// global entry (commits made before the activation height, or all of them when not keyed).
    fn find_commit(
        &self,
        sender: &AccountId,
        commitment: &[u8; 32],
        sender_keyed: bool,
    ) -> Option<([u8; 32], &CommitRecord)> {
        if sender_keyed {
            let k = Self::sender_commit_key(sender, commitment);
            if let Some(r) = self.commits.get(&k) {
                return Some((k, r));
            }
        }
        self.commits
            .get(commitment)
            .filter(|r| r.sender == *sender)
            .map(|r| (*commitment, r))
    }

    /// Validate a registration without mutating; returns the rent to burn.
    pub fn check_register(
        &self,
        params: &NameRegistryParams,
        height: u64,
        sender: AccountId,
        label: &str,
        salt: &[u8; 32],
        years: u8,
    ) -> Result<u128, NameRegistryError> {
        self.check_register_keyed(params, height, sender, label, salt, years, false)
            .map(|(rent, _)| rent)
    }

    /// Like [`Self::check_register`]; also returns the storage key of the matching commitment.
    #[allow(clippy::too_many_arguments)]
    pub fn check_register_keyed(
        &self,
        params: &NameRegistryParams,
        height: u64,
        sender: AccountId,
        label: &str,
        salt: &[u8; 32],
        years: u8,
        sender_keyed: bool,
    ) -> Result<(u128, [u8; 32]), NameRegistryError> {
        check_registrable(label).map_err(|e| NameRegistryError::InvalidName(e.to_string()))?;
        if !(NAME_MIN_YEARS..=NAME_MAX_YEARS).contains(&years) {
            return Err(NameRegistryError::InvalidYears);
        }
        let c = name_commitment(label, &sender, salt);
        let (key, rec) = self
            .find_commit(&sender, &c, sender_keyed)
            .ok_or(NameRegistryError::CommitNotFound)?;
        let age = height.saturating_sub(rec.height);
        if age < params.commit_min_age {
            return Err(NameRegistryError::CommitTooNew {
                ready_at: rec.height + params.commit_min_age,
            });
        }
        if age > params.commit_max_age {
            return Err(NameRegistryError::CommitTooOld);
        }
        if !self.is_available(label, params, height) {
            return Err(NameRegistryError::NotAvailable);
        }
        Ok((name_rent(label, years), key))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn register(
        &mut self,
        params: &NameRegistryParams,
        height: u64,
        sender: AccountId,
        label: &str,
        salt: &[u8; 32],
        target: AccountId,
        years: u8,
    ) -> Result<u128, NameRegistryError> {
        self.register_keyed(params, height, sender, label, salt, target, years, false)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn register_keyed(
        &mut self,
        params: &NameRegistryParams,
        height: u64,
        sender: AccountId,
        label: &str,
        salt: &[u8; 32],
        target: AccountId,
        years: u8,
        sender_keyed: bool,
    ) -> Result<u128, NameRegistryError> {
        let (rent, key) =
            self.check_register_keyed(params, height, sender, label, salt, years, sender_keyed)?;
        self.commits.remove(&key);
        if let Some(old) = self.names.remove(label) {
            self.clear_reverse_pointing_to(&old.target, label);
            self.clear_reverse_pointing_to(&old.owner, label);
        }
        self.names.insert(
            label.to_string(),
            NameRecord {
                owner: sender,
                target,
                registered_at: height,
                expires_at: height
                    .saturating_add(params.blocks_per_year.saturating_mul(years as u64)),
            },
        );
        self.prune_commits(params, height);
        Ok(rent)
    }

    fn owned_mut(
        &mut self,
        params: &NameRegistryParams,
        height: u64,
        sender: &AccountId,
        label: &str,
        allow_grace: bool,
    ) -> Result<&mut NameRecord, NameRegistryError> {
        let rec = self
            .names
            .get_mut(label)
            .ok_or(NameRegistryError::NotRegistered)?;
        match Self::status_of(rec, params, height) {
            NameStatus::Active => {}
            NameStatus::Grace if allow_grace => {}
            NameStatus::Grace => return Err(NameRegistryError::Expired),
            NameStatus::Expired => return Err(NameRegistryError::NotRegistered),
        }
        if rec.owner != *sender {
            return Err(NameRegistryError::NotOwner);
        }
        Ok(rec)
    }

    /// Returns the rent to burn.
    pub fn renew(
        &mut self,
        params: &NameRegistryParams,
        height: u64,
        sender: AccountId,
        label: &str,
        years: u8,
    ) -> Result<u128, NameRegistryError> {
        if !(NAME_MIN_YEARS..=NAME_MAX_YEARS).contains(&years) {
            return Err(NameRegistryError::InvalidYears);
        }
        let bpy = params.blocks_per_year;
        let rec = self.owned_mut(params, height, &sender, label, true)?;
        let base = rec.expires_at.max(height);
        let new_exp = base.saturating_add(bpy.saturating_mul(years as u64));
        if new_exp.saturating_sub(height) > bpy.saturating_mul(NAME_MAX_YEARS as u64) {
            return Err(NameRegistryError::TooFarAhead);
        }
        rec.expires_at = new_exp;
        Ok(name_rent(label, years))
    }

    pub fn transfer(
        &mut self,
        params: &NameRegistryParams,
        height: u64,
        sender: AccountId,
        label: &str,
        new_owner: AccountId,
    ) -> Result<(), NameRegistryError> {
        let rec = self.owned_mut(params, height, &sender, label, false)?;
        rec.owner = new_owner;
        self.clear_reverse_pointing_to(&sender, label);
        Ok(())
    }

    pub fn set_target(
        &mut self,
        params: &NameRegistryParams,
        height: u64,
        sender: AccountId,
        label: &str,
        target: AccountId,
    ) -> Result<(), NameRegistryError> {
        let rec = self.owned_mut(params, height, &sender, label, false)?;
        let old = rec.target;
        rec.target = target;
        if old != target {
            self.clear_reverse_pointing_to(&old, label);
        }
        Ok(())
    }

    pub fn set_primary(
        &mut self,
        params: &NameRegistryParams,
        height: u64,
        sender: AccountId,
        label: Option<&str>,
    ) -> Result<(), NameRegistryError> {
        match label {
            None => {
                self.reverse.remove(&sender);
                Ok(())
            }
            Some(l) => {
                let ok = self
                    .resolve(l, params, height)
                    .map(|r| r.target == sender)
                    .unwrap_or(false);
                if !ok {
                    return Err(NameRegistryError::PrimaryMismatch);
                }
                self.reverse.insert(sender, l.to_string());
                Ok(())
            }
        }
    }

    pub fn release(
        &mut self,
        params: &NameRegistryParams,
        height: u64,
        sender: AccountId,
        label: &str,
    ) -> Result<(), NameRegistryError> {
        let rec = self.owned_mut(params, height, &sender, label, true)?;
        let (owner, target) = (rec.owner, rec.target);
        self.names.remove(label);
        self.clear_reverse_pointing_to(&target, label);
        self.clear_reverse_pointing_to(&owner, label);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: AccountId = AccountId([1; 32]);
    const B: AccountId = AccountId([2; 32]);
    const C: AccountId = AccountId([3; 32]);
    const SALT: [u8; 32] = [9; 32];

    fn p() -> NameRegistryParams {
        NameRegistryParams {
            activation_height: 0,
            blocks_per_year: 100,
            grace_blocks: 20,
            commit_min_age: 2,
            commit_max_age: 50,
        }
    }

    fn reg_alice(r: &mut NameRegistry) {
        r.commit(&p(), 10, A, name_commitment("alice", &A, &SALT))
            .unwrap();
        assert_eq!(r.register(&p(), 12, A, "alice", &SALT, A, 1), Ok(2));
    }

    #[test]
    fn commit_reveal_rules() {
        let mut r = NameRegistry::default();
        let c = name_commitment("alice", &A, &SALT);
        assert_eq!(
            r.register(&p(), 5, A, "alice", &SALT, A, 1),
            Err(NameRegistryError::CommitNotFound)
        );
        r.commit(&p(), 10, A, c).unwrap();
        assert_eq!(
            r.commit(&p(), 10, A, c),
            Err(NameRegistryError::CommitExists)
        );
        assert_eq!(
            r.register(&p(), 11, A, "alice", &SALT, A, 1),
            Err(NameRegistryError::CommitTooNew { ready_at: 12 })
        );
        // another sender cannot reveal A's commitment (commitment binds the owner)
        assert_eq!(
            r.register(&p(), 12, B, "alice", &SALT, B, 1),
            Err(NameRegistryError::CommitNotFound)
        );
        assert_eq!(
            r.register(&p(), 12, A, "alice", &SALT, A, 0),
            Err(NameRegistryError::InvalidYears)
        );
        assert_eq!(
            r.register(&p(), 12, A, "alice", &SALT, A, 11),
            Err(NameRegistryError::InvalidYears)
        );
        assert_eq!(
            r.register(&p(), 61, A, "alice", &SALT, A, 1),
            Err(NameRegistryError::CommitTooOld)
        );
        assert_eq!(r.register(&p(), 12, A, "alice", &SALT, A, 3), Ok(6));
        assert!(r.commits.is_empty(), "commitment consumed");
        let rec = r.names.get("alice").unwrap();
        assert_eq!((rec.owner, rec.target, rec.expires_at), (A, A, 312));
    }

    #[test]
    fn reserved_and_invalid_rejected() {
        let mut r = NameRegistry::default();
        r.commit(&p(), 1, A, name_commitment("boing", &A, &SALT))
            .unwrap();
        assert!(matches!(
            r.register(&p(), 5, A, "boing", &SALT, A, 1),
            Err(NameRegistryError::InvalidName(_))
        ));
        assert!(matches!(
            r.check_register(&p(), 5, A, "Bad", &SALT, 1),
            Err(NameRegistryError::InvalidName(_))
        ));
    }

    #[test]
    fn taken_until_grace_ends_then_reregistrable() {
        let mut r = NameRegistry::default();
        reg_alice(&mut r); // expires 112
        r.set_primary(&p(), 20, A, Some("alice")).unwrap();
        r.commit(&p(), 20, B, name_commitment("alice", &B, &SALT))
            .unwrap();
        assert_eq!(
            r.register(&p(), 30, B, "alice", &SALT, B, 1),
            Err(NameRegistryError::NotAvailable)
        );
        assert_eq!(r.status("alice", &p(), 111), Some(NameStatus::Active));
        assert_eq!(r.status("alice", &p(), 112), Some(NameStatus::Grace));
        assert!(
            r.resolve("alice", &p(), 112).is_none(),
            "grace names do not resolve"
        );
        assert!(r.reverse_verified(&A, &p(), 112).is_none());
        assert_eq!(r.status("alice", &p(), 132), Some(NameStatus::Expired));
        r.commit(&p(), 130, B, name_commitment("alice", &B, &[5; 32]))
            .unwrap();
        r.register(&p(), 132, B, "alice", &[5; 32], C, 1).unwrap();
        assert_eq!(r.resolve("alice", &p(), 133).unwrap().target, C);
        assert!(
            r.reverse.get(&A).is_none(),
            "old reverse cleared on re-registration"
        );
    }

    #[test]
    fn renew_rules() {
        let mut r = NameRegistry::default();
        reg_alice(&mut r);
        assert_eq!(
            r.renew(&p(), 20, B, "alice", 1),
            Err(NameRegistryError::NotOwner)
        );
        assert_eq!(r.renew(&p(), 20, A, "alice", 1), Ok(2));
        assert_eq!(r.names["alice"].expires_at, 212);
        assert_eq!(
            r.renew(&p(), 20, A, "alice", 9),
            Err(NameRegistryError::TooFarAhead)
        );
        // grace renewal extends from now
        assert_eq!(r.renew(&p(), 220, A, "alice", 1), Ok(2));
        assert_eq!(r.names["alice"].expires_at, 320);
        assert_eq!(
            r.renew(&p(), 400, A, "alice", 1),
            Err(NameRegistryError::NotRegistered)
        );
    }

    #[test]
    fn transfer_target_primary_release() {
        let mut r = NameRegistry::default();
        reg_alice(&mut r);
        assert_eq!(
            r.set_primary(&p(), 20, B, Some("alice")),
            Err(NameRegistryError::PrimaryMismatch)
        );
        r.set_primary(&p(), 20, A, Some("alice")).unwrap();
        assert_eq!(r.reverse_verified(&A, &p(), 20), Some("alice"));
        assert_eq!(
            r.transfer(&p(), 21, B, "alice", B),
            Err(NameRegistryError::NotOwner)
        );
        r.transfer(&p(), 21, A, "alice", B).unwrap();
        assert_eq!(r.names["alice"].owner, B);
        assert_eq!(r.names["alice"].target, A, "target kept on transfer");
        assert!(r.reverse.get(&A).is_none());
        assert_eq!(
            r.set_target(&p(), 22, A, "alice", C),
            Err(NameRegistryError::NotOwner)
        );
        r.set_target(&p(), 22, B, "alice", C).unwrap();
        r.set_primary(&p(), 23, C, Some("alice")).unwrap();
        r.set_target(&p(), 24, B, "alice", B).unwrap();
        assert!(
            r.reverse.get(&C).is_none(),
            "re-target clears the old target's reverse"
        );
        r.set_primary(&p(), 25, B, Some("alice")).unwrap();
        r.release(&p(), 26, B, "alice").unwrap();
        assert!(r.names.is_empty() && r.reverse.is_empty());
        r.set_primary(&p(), 27, B, None).unwrap();
    }

    #[test]
    fn stale_reverse_is_not_verified() {
        let mut r = NameRegistry::default();
        reg_alice(&mut r);
        r.set_primary(&p(), 20, A, Some("alice")).unwrap();
        // simulate a stale entry (e.g. manual): target moved without eager clear
        r.names.get_mut("alice").unwrap().target = B;
        assert!(r.reverse_verified(&A, &p(), 21).is_none());
    }

    #[test]
    fn commits_pruned_and_commitment_deterministic() {
        let mut a = NameRegistry::default();
        let mut b = NameRegistry::default();
        for reg in [&mut a, &mut b] {
            reg.commit(&p(), 1, A, [1; 32]).unwrap();
            reg.commit(&p(), 2, B, [2; 32]).unwrap();
            reg.commit(&p(), 100, C, [3; 32]).unwrap();
        }
        assert_eq!(a.commits.len(), 1, "commits older than max age pruned");
        assert_eq!(a.commitment(), b.commitment());
        assert!(NameRegistry::default().is_empty());
    }

    #[test]
    fn copied_commitment_cannot_block_victim_when_sender_keyed() {
        // Security review #6: B front-runs A's NameCommit with the same hash.
        let c = name_commitment("alice", &A, &SALT);
        // Legacy (pre-v4): the copy blocks A.
        let mut legacy = NameRegistry::default();
        legacy.commit_keyed(&p(), 10, B, c, false).unwrap();
        assert_eq!(
            legacy.commit_keyed(&p(), 10, A, c, false),
            Err(NameRegistryError::CommitExists)
        );
        // v4: both commits coexist, A registers, B's copy is useless to B.
        let mut r = NameRegistry::default();
        r.commit_keyed(&p(), 10, B, c, true).unwrap();
        r.commit_keyed(&p(), 10, A, c, true).unwrap();
        assert_eq!(
            r.commit_keyed(&p(), 10, A, c, true),
            Err(NameRegistryError::CommitExists),
            "a sender still cannot duplicate its own commitment"
        );
        assert_eq!(
            r.register_keyed(&p(), 12, B, "alice", &SALT, B, 1, true),
            Err(NameRegistryError::CommitNotFound)
        );
        assert_eq!(r.register_keyed(&p(), 12, A, "alice", &SALT, A, 1, true), Ok(2));
        assert_eq!(r.names.get("alice").unwrap().owner, A);
        assert_eq!(r.commits.len(), 1, "only B's dead commitment remains (pruned at max age)");
    }

    #[test]
    fn pre_activation_commit_still_revealable_after_activation() {
        let mut r = NameRegistry::default();
        let c = name_commitment("alice", &A, &SALT);
        r.commit_keyed(&p(), 10, A, c, false).unwrap();
        assert_eq!(r.register_keyed(&p(), 12, A, "alice", &SALT, A, 1, true), Ok(2));
        assert!(r.commits.is_empty());
        // a legacy entry made by someone else is not usable by the reveal of A
        let mut r2 = NameRegistry::default();
        r2.commit_keyed(&p(), 10, B, c, false).unwrap();
        assert_eq!(
            r2.register_keyed(&p(), 12, A, "alice", &SALT, A, 1, true),
            Err(NameRegistryError::CommitNotFound)
        );
    }
}
