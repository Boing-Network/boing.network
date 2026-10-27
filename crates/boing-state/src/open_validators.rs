//! Open (permissionless) validator registry with liveness-based jailing.
//!
//! Everything here is part of consensus state: it is mutated only by transactions
//! (`ValidatorRegister` / `ValidatorDeregister` / `ValidatorUnjail`) and by the deterministic
//! end-of-block hook [`OpenValidatorRegistry::on_block`], and it is committed into the state root
//! (one SMT leaf) as soon as it is non-empty. Every node therefore derives the same set.
//!
//! ## Model
//! - **Anchors** (from config, e.g. the operator's `boing-testnet-1`) are always in the set, can never be
//!   jailed or dropped, and alone form the commit quorum (see `boing-consensus` anchor quorum).
//! - **Open validators** register with a fixed bond, become `Pending`, and join at the next epoch
//!   boundary (heights that are multiples of `epoch_len`, at/after `activation_height`), ordered by
//!   registration height then node id, capped at `max_open_validators`.
//! - **Liveness** is measured over the last `liveness_window` *produced* blocks: for every block the
//!   scheduled round-robin leader is recorded together with whether it actually proposed. Idle time
//!   produces no blocks, so it never counts against anyone. At each boundary an open validator whose
//!   missed share exceeds `miss_threshold_bps` (with at least `min_liveness_samples` scheduled slots)
//!   is jailed until `height + jail_cooldown`. The bond is kept (no slashing on testnet).

use std::collections::{BTreeMap, VecDeque};

use boing_primitives::{hasher, AccountId};
use serde::{Deserialize, Serialize};

/// Node configuration for open mode. **Not** part of the state root, but every node must use the same
/// values (a mismatch shows up as a state-root mismatch on the first affected block).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenValidatorParams {
    /// Registrations are accepted from this height; the set switches at the first epoch boundary >= it.
    pub activation_height: u64,
    /// Permanent validators (never jailed / dropped). Must equal the static `--validators` list.
    pub anchors: Vec<AccountId>,
    /// Bond locked by `ValidatorRegister`.
    pub bond: u128,
    /// Blocks after `ValidatorDeregister` before the bond can be claimed with `ClaimUnbond`.
    pub deregister_delay: u64,
    pub epoch_len: u64,
    pub max_open_validators: usize,
    /// Number of most recent produced blocks considered for liveness.
    pub liveness_window: usize,
    /// Jail when missed / scheduled > this many basis points (1000 = 10%).
    pub miss_threshold_bps: u32,
    /// Minimum scheduled proposals in the window before a validator can be jailed.
    pub min_liveness_samples: u32,
    /// Blocks a jailed validator must wait before `ValidatorUnjail`.
    pub jail_cooldown: u64,
}

impl Default for OpenValidatorParams {
    fn default() -> Self {
        Self {
            activation_height: 0,
            anchors: Vec::new(),
            bond: 100,
            deregister_delay: 100,
            epoch_len: 100,
            max_open_validators: 20,
            liveness_window: 1_000,
            miss_threshold_bps: 1_000,
            min_liveness_samples: 10,
            jail_cooldown: 500,
        }
    }
}

impl OpenValidatorParams {
    pub fn is_anchor(&self, id: &AccountId) -> bool {
        self.anchors.contains(id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidatorStatus {
    /// Registered; joins at the next epoch boundary (if within the cap).
    Pending,
    /// In the consensus set.
    Active,
    /// Removed for missing proposals; may `ValidatorUnjail` at/after `until`.
    Jailed { until: u64 },
    /// Deregistered while active; leaves the set at the next boundary.
    Exiting,
}

impl ValidatorStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ValidatorStatus::Pending => "pending",
            ValidatorStatus::Active => "active",
            ValidatorStatus::Jailed { .. } => "jailed",
            ValidatorStatus::Exiting => "exiting",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorRecord {
    /// Account that signed the registration and owns the bond.
    pub owner: AccountId,
    /// Locked bond (0 once released on deregistration).
    pub bond: u128,
    pub registered_at: u64,
    pub status: ValidatorStatus,
    pub jail_count: u32,
}

/// One produced block: who was scheduled and who proposed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LivenessEntry {
    pub height: u64,
    pub expected: AccountId,
    pub proposer: AccountId,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum OpenValidatorError {
    #[error("open validator registration is not enabled on this network")]
    Disabled,
    #[error("open validator registration activates at height {activation_height}")]
    NotYetActive { activation_height: u64 },
    #[error("invalid node signature (node key must sign the registration for this owner)")]
    InvalidNodeSignature,
    #[error("node is already registered")]
    AlreadyRegistered,
    #[error("owner already has a registered validator")]
    OwnerAlreadyRegistered,
    #[error("anchor validators cannot be registered")]
    AnchorNotRegistrable,
    #[error("validator not registered")]
    NotRegistered,
    #[error("only the registering owner can do this")]
    NotOwner,
    #[error("validator is already exiting")]
    AlreadyExiting,
    #[error("validator is not jailed")]
    NotJailed,
    #[error("jail cooldown until height {until}")]
    JailCooldown { until: u64 },
    #[error("open validator registry is full ({max} records)")]
    RegistryFull { max: usize },
}

/// Liveness numbers for one validator over the current window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LivenessStats {
    pub scheduled: u32,
    pub missed: u32,
}

impl LivenessStats {
    /// Proposal uptime in percent (100 when never scheduled).
    pub fn uptime_pct(&self) -> f64 {
        if self.scheduled == 0 {
            100.0
        } else {
            100.0 * (self.scheduled - self.missed) as f64 / self.scheduled as f64
        }
    }
}

/// Liveness at the moment a validator was jailed (the window entries are purged on jailing so an
/// unjailed validator starts fresh). Node-local, derived data: **not** part of the state commitment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JailSnapshot {
    pub jailed_at: u64,
    pub scheduled: u32,
    pub missed: u32,
}

impl JailSnapshot {
    pub fn stats(&self) -> LivenessStats {
        LivenessStats {
            scheduled: self.scheduled,
            missed: self.missed,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenValidatorRegistry {
    /// First epoch boundary at which the open set took over (None = static set still in force).
    pub activated_at: Option<u64>,
    pub records: BTreeMap<AccountId, ValidatorRecord>,
    /// Open validators currently in the set (sorted by id).
    pub active: Vec<AccountId>,
    pub liveness: VecDeque<LivenessEntry>,
    /// Last jailing snapshot per validator. `#[serde(skip)]` keeps it out of the bincode encoding, so the
    /// registry commitment (state root) and `open_validators.bin` are byte-identical to before; it is
    /// persisted in its own file. Deterministic (computed in `on_block`), so replaying nodes agree.
    #[serde(skip)]
    pub last_jail: BTreeMap<AccountId, JailSnapshot>,
}

impl OpenValidatorRegistry {
    /// SMT key of the registry leaf.
    pub fn smt_key() -> [u8; 32] {
        let mut h = hasher();
        h.update(b"BOING_OPEN_VALIDATOR_REGISTRY_V1");
        *h.finalize().as_bytes()
    }

    /// Empty registries are not committed, so the state root is unchanged until first use.
    pub fn is_empty(&self) -> bool {
        self.activated_at.is_none() && self.records.is_empty() && self.liveness.is_empty()
    }

    pub fn commitment(&self) -> [u8; 32] {
        let mut h = hasher();
        h.update(b"BOING_OPEN_VALIDATOR_REGISTRY_V1");
        h.update(&bincode::serialize(self).unwrap_or_default());
        *h.finalize().as_bytes()
    }

    /// Consensus set once activated: anchors ∪ active open validators, sorted by id. `None` before activation.
    pub fn consensus_set(&self, params: &OpenValidatorParams) -> Option<Vec<AccountId>> {
        self.activated_at?;
        let mut set: Vec<AccountId> = params.anchors.clone();
        for id in &self.active {
            if !set.contains(id) {
                set.push(*id);
            }
        }
        set.sort();
        set.dedup();
        Some(set)
    }

    /// Security review #10: upper bound on registry records (pending + active + jailed + exiting), so the
    /// one-leaf registry (re-serialized and re-hashed on every change) cannot grow without limit.
    pub fn max_records(params: &OpenValidatorParams) -> usize {
        params.max_open_validators.saturating_mul(4).max(16)
    }

    /// Legacy registration (no record cap), as executed below the security-v4 height.
    pub fn register(
        &mut self,
        params: &OpenValidatorParams,
        height: u64,
        owner: AccountId,
        node_id: AccountId,
    ) -> Result<(), OpenValidatorError> {
        self.register_with_cap(params, height, owner, node_id, false)
    }

    /// `capped`: enforce [`Self::max_records`] (security-v4 rule).
    pub fn register_with_cap(
        &mut self,
        params: &OpenValidatorParams,
        height: u64,
        owner: AccountId,
        node_id: AccountId,
        capped: bool,
    ) -> Result<(), OpenValidatorError> {
        if height < params.activation_height {
            return Err(OpenValidatorError::NotYetActive {
                activation_height: params.activation_height,
            });
        }
        if params.is_anchor(&node_id) {
            return Err(OpenValidatorError::AnchorNotRegistrable);
        }
        if self.records.contains_key(&node_id) {
            return Err(OpenValidatorError::AlreadyRegistered);
        }
        if self.records.values().any(|r| r.owner == owner) {
            return Err(OpenValidatorError::OwnerAlreadyRegistered);
        }
        if capped && self.records.len() >= Self::max_records(params) {
            return Err(OpenValidatorError::RegistryFull {
                max: Self::max_records(params),
            });
        }
        self.records.insert(
            node_id,
            ValidatorRecord {
                owner,
                bond: params.bond,
                registered_at: height,
                status: ValidatorStatus::Pending,
                jail_count: 0,
            },
        );
        Ok(())
    }

    /// Legacy deregistration (a jailed validator could leave and reclaim its bond at once).
    pub fn deregister(
        &mut self,
        owner: AccountId,
        node_id: AccountId,
    ) -> Result<u128, OpenValidatorError> {
        self.deregister_at(owner, node_id, 0, false)
    }

    /// Returns the bond to release to the owner's `pending_unbond`. With `jail_locks_bond` (security-v4,
    /// review #11) a jailed validator cannot exit before its jail ends, so jailing is no longer a free,
    /// immediate exit with the full bond; it may deregister (or unjail) at/after `until`.
    pub fn deregister_at(
        &mut self,
        owner: AccountId,
        node_id: AccountId,
        height: u64,
        jail_locks_bond: bool,
    ) -> Result<u128, OpenValidatorError> {
        let rec = self
            .records
            .get_mut(&node_id)
            .ok_or(OpenValidatorError::NotRegistered)?;
        if rec.owner != owner {
            return Err(OpenValidatorError::NotOwner);
        }
        let bond = rec.bond;
        match rec.status {
            ValidatorStatus::Exiting => return Err(OpenValidatorError::AlreadyExiting),
            ValidatorStatus::Jailed { until } if jail_locks_bond && height < until => {
                return Err(OpenValidatorError::JailCooldown { until })
            }
            ValidatorStatus::Active => {
                // Stay in the set until the next boundary so the set never changes mid-epoch.
                rec.status = ValidatorStatus::Exiting;
                rec.bond = 0;
            }
            ValidatorStatus::Pending | ValidatorStatus::Jailed { .. } => {
                self.records.remove(&node_id);
            }
        }
        Ok(bond)
    }

    pub fn unjail(
        &mut self,
        owner: AccountId,
        node_id: AccountId,
        height: u64,
    ) -> Result<(), OpenValidatorError> {
        let rec = self
            .records
            .get_mut(&node_id)
            .ok_or(OpenValidatorError::NotRegistered)?;
        if rec.owner != owner {
            return Err(OpenValidatorError::NotOwner);
        }
        match rec.status {
            ValidatorStatus::Jailed { until } if height >= until => {
                rec.status = ValidatorStatus::Pending;
                Ok(())
            }
            ValidatorStatus::Jailed { until } => Err(OpenValidatorError::JailCooldown { until }),
            _ => Err(OpenValidatorError::NotJailed),
        }
    }

    /// Liveness over the current window.
    pub fn stats(&self, id: &AccountId) -> LivenessStats {
        let mut s = LivenessStats::default();
        for e in &self.liveness {
            if e.expected == *id {
                s.scheduled += 1;
                if e.proposer != *id {
                    s.missed += 1;
                }
            }
        }
        s
    }

    /// Deterministic end-of-block hook (called by producer and importer before the state root).
    pub fn on_block(&mut self, params: &OpenValidatorParams, height: u64, proposer: AccountId) {
        if height < params.activation_height || params.epoch_len == 0 {
            return;
        }
        if let Some(set) = self.consensus_set(params) {
            if !set.is_empty() {
                let expected = set[(height as usize) % set.len()];
                self.liveness.push_back(LivenessEntry {
                    height,
                    expected,
                    proposer,
                });
                while self.liveness.len() > params.liveness_window.max(1) {
                    self.liveness.pop_front();
                }
            }
        }
        if height.is_multiple_of(params.epoch_len) {
            if self.activated_at.is_none() {
                self.activated_at = Some(height);
            }
            self.epoch_transition(params, height);
        }
    }

    fn epoch_transition(&mut self, params: &OpenValidatorParams, height: u64) {
        // 1. Jail active open validators that missed too many scheduled proposals.
        let mut jailed = Vec::new();
        for id in &self.active {
            let s = self.stats(id);
            if s.scheduled >= params.min_liveness_samples.max(1)
                && (s.missed as u64) * 10_000
                    > (params.miss_threshold_bps as u64) * (s.scheduled as u64)
            {
                jailed.push(*id);
            }
        }
        for id in &jailed {
            let s = self.stats(id);
            self.last_jail.insert(
                *id,
                JailSnapshot {
                    jailed_at: height,
                    scheduled: s.scheduled,
                    missed: s.missed,
                },
            );
            if let Some(rec) = self.records.get_mut(id) {
                if rec.status == ValidatorStatus::Active {
                    rec.status = ValidatorStatus::Jailed {
                        until: height.saturating_add(params.jail_cooldown),
                    };
                    rec.jail_count = rec.jail_count.saturating_add(1);
                }
            }
            // Fresh start after unjail: forget old misses.
            self.liveness.retain(|e| e.expected != *id);
        }
        // 2. Exiting validators leave now.
        self.records
            .retain(|_, r| r.status != ValidatorStatus::Exiting);
        let records = &self.records;
        self.last_jail.retain(|id, _| records.contains_key(id));
        // 3. Select the next open set: earliest registration first, then node id; capped.
        let mut eligible: Vec<(u64, AccountId)> = self
            .records
            .iter()
            .filter(|(_, r)| matches!(r.status, ValidatorStatus::Pending | ValidatorStatus::Active))
            .map(|(id, r)| (r.registered_at, *id))
            .collect();
        eligible.sort();
        let chosen: Vec<AccountId> = eligible
            .into_iter()
            .take(params.max_open_validators)
            .map(|(_, id)| id)
            .collect();
        for (id, rec) in self.records.iter_mut() {
            if matches!(
                rec.status,
                ValidatorStatus::Pending | ValidatorStatus::Active
            ) {
                rec.status = if chosen.contains(id) {
                    ValidatorStatus::Active
                } else {
                    ValidatorStatus::Pending
                };
            }
        }
        let mut active = chosen;
        active.sort();
        self.active = active;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(b: u8) -> AccountId {
        AccountId([b; 32])
    }

    fn params() -> OpenValidatorParams {
        OpenValidatorParams {
            activation_height: 10,
            anchors: vec![id(1)],
            epoch_len: 10,
            liveness_window: 50,
            min_liveness_samples: 3,
            jail_cooldown: 20,
            max_open_validators: 2,
            ..Default::default()
        }
    }

    #[test]
    fn register_rules() {
        let p = params();
        let mut r = OpenValidatorRegistry::default();
        assert_eq!(
            r.register(&p, 5, id(9), id(2)),
            Err(OpenValidatorError::NotYetActive {
                activation_height: 10
            })
        );
        assert_eq!(
            r.register(&p, 10, id(9), id(1)),
            Err(OpenValidatorError::AnchorNotRegistrable)
        );
        r.register(&p, 10, id(9), id(2)).unwrap();
        assert_eq!(
            r.register(&p, 11, id(8), id(2)),
            Err(OpenValidatorError::AlreadyRegistered)
        );
        assert_eq!(
            r.register(&p, 11, id(9), id(3)),
            Err(OpenValidatorError::OwnerAlreadyRegistered)
        );
        assert_eq!(
            r.deregister(id(8), id(2)),
            Err(OpenValidatorError::NotOwner)
        );
        assert_eq!(r.deregister(id(9), id(2)), Ok(100));
        assert!(r.records.is_empty());
    }

    #[test]
    fn joins_at_boundary_with_cap_and_order() {
        let p = params();
        let mut r = OpenValidatorRegistry::default();
        assert!(r.is_empty());
        r.register(&p, 10, id(90), id(5)).unwrap();
        r.register(&p, 11, id(91), id(3)).unwrap();
        r.register(&p, 12, id(92), id(4)).unwrap();
        assert!(r.consensus_set(&p).is_none());
        r.on_block(&p, 20, id(1));
        // Cap 2: the two earliest registrations join (id5 @10, id3 @11); id4 waits.
        assert_eq!(r.active, vec![id(3), id(5)]);
        assert_eq!(r.records[&id(4)].status, ValidatorStatus::Pending);
        assert_eq!(r.consensus_set(&p).unwrap(), vec![id(1), id(3), id(5)]);
    }

    #[test]
    fn jails_offline_validator_and_unjail_after_cooldown() {
        let p = params();
        let mut r = OpenValidatorRegistry::default();
        r.register(&p, 10, id(90), id(5)).unwrap();
        r.on_block(&p, 10, id(1)); // activation boundary → id(5) active
        assert_eq!(r.active, vec![id(5)]);
        // id(5) never proposes; anchor id(1) proposes every block (fallback).
        for h in 11..=20 {
            r.on_block(&p, h, id(1));
        }
        assert!(matches!(
            r.records[&id(5)].status,
            ValidatorStatus::Jailed { until: 40 }
        ));
        assert!(r.active.is_empty());
        assert_eq!(
            r.unjail(id(90), id(5), 30),
            Err(OpenValidatorError::JailCooldown { until: 40 })
        );
        r.unjail(id(90), id(5), 40).unwrap();
        assert_eq!(r.records[&id(5)].status, ValidatorStatus::Pending);
        assert_eq!(r.records[&id(5)].bond, 100, "bond kept while jailed");
    }

    #[test]
    fn online_validator_is_not_jailed_and_idle_is_free() {
        let p = params();
        let mut r = OpenValidatorRegistry::default();
        r.register(&p, 10, id(90), id(5)).unwrap();
        r.on_block(&p, 10, id(1));
        // Few blocks over a long idle span; each proposed by its scheduled leader.
        for h in [11u64, 12, 13, 14, 15, 16, 17, 18, 19, 20, 1_000, 1_001] {
            let set = r.consensus_set(&p).unwrap();
            let leader = set[(h as usize) % set.len()];
            r.on_block(&p, h, leader);
        }
        assert_eq!(r.records[&id(5)].status, ValidatorStatus::Active);
        assert_eq!(r.stats(&id(5)).missed, 0);
    }

    #[test]
    fn exiting_leaves_at_boundary() {
        let p = params();
        let mut r = OpenValidatorRegistry::default();
        r.register(&p, 10, id(90), id(5)).unwrap();
        r.on_block(&p, 10, id(1));
        assert_eq!(r.deregister(id(90), id(5)), Ok(100));
        assert_eq!(r.records[&id(5)].status, ValidatorStatus::Exiting);
        assert_eq!(r.active, vec![id(5)], "stays until boundary");
        assert_eq!(
            r.deregister(id(90), id(5)),
            Err(OpenValidatorError::AlreadyExiting)
        );
        for h in 11..=20 {
            let set = r.consensus_set(&p).unwrap();
            r.on_block(&p, h, set[(h as usize) % set.len()]);
        }
        assert!(r.active.is_empty());
        assert!(!r.records.contains_key(&id(5)));
    }
}

#[cfg(test)]
mod security_v4_tests {
    use super::*;

    fn id(n: u8) -> AccountId {
        AccountId([n; 32])
    }

    #[test]
    fn registry_cap_only_when_capped() {
        let p = OpenValidatorParams {
            max_open_validators: 2,
            ..Default::default()
        };
        let max = OpenValidatorRegistry::max_records(&p);
        assert_eq!(max, 16);
        let mut r = OpenValidatorRegistry::default();
        for n in 0..max as u8 {
            r.register_with_cap(&p, 1, id(100 + n), id(n + 1), true)
                .unwrap();
        }
        assert_eq!(
            r.register_with_cap(&p, 1, id(250), id(200), true),
            Err(OpenValidatorError::RegistryFull { max })
        );
        // legacy rule (replay below the activation height) is unchanged
        let mut legacy = r.clone();
        assert!(legacy
            .register_with_cap(&p, 1, id(250), id(200), false)
            .is_ok());
        // freeing a slot lets the next registration through
        r.deregister_at(id(100), id(1), 1, true).unwrap();
        assert!(r.register_with_cap(&p, 1, id(250), id(200), true).is_ok());
    }

    #[test]
    fn jailed_validator_cannot_exit_before_jail_ends() {
        let p = OpenValidatorParams::default();
        let mut r = OpenValidatorRegistry::default();
        r.register(&p, 1, id(90), id(5)).unwrap();
        r.records.get_mut(&id(5)).unwrap().status = ValidatorStatus::Jailed { until: 600 };
        let mut legacy = r.clone();
        assert_eq!(
            legacy.deregister_at(id(90), id(5), 100, false),
            Ok(100),
            "legacy: immediate exit"
        );
        assert_eq!(
            r.deregister_at(id(90), id(5), 599, true),
            Err(OpenValidatorError::JailCooldown { until: 600 })
        );
        assert_eq!(r.records[&id(5)].bond, 100, "bond stays locked");
        assert_eq!(r.deregister_at(id(90), id(5), 600, true), Ok(100));
        assert!(!r.records.contains_key(&id(5)));
    }
}

#[cfg(test)]
mod jail_snapshot_compat_tests {
    use super::*;

    /// `last_jail` must not change the registry encoding (state root / open_validators.bin), so nodes
    /// upgraded after block 400 stay on the same chain.
    #[test]
    fn last_jail_is_not_committed_or_encoded() {
        let mut a = OpenValidatorRegistry {
            activated_at: Some(400),
            ..Default::default()
        };
        let b = a.clone();
        a.last_jail.insert(
            AccountId([7u8; 32]),
            JailSnapshot {
                jailed_at: 600,
                scheduled: 50,
                missed: 50,
            },
        );
        assert_eq!(a.commitment(), b.commitment());
        assert_eq!(
            bincode::serialize(&a).unwrap(),
            bincode::serialize(&b).unwrap()
        );
        let back: OpenValidatorRegistry =
            bincode::deserialize(&bincode::serialize(&a).unwrap()).unwrap();
        assert!(back.last_jail.is_empty());
    }
}
