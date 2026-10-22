//! SMT-resident sample-serve challenge registry (Phase 10/11).
//!
//! After settle, a challenger can open a window for the provider to prove availability
//! via a DAS merkle sample. Phase 11 escrows a challenger bond and settles FailedServe
//! (bond refund + provider penalty) in the VM / end-of-block hook.

use std::collections::BTreeMap;

use boing_primitives::{
    blob_id_hex, hasher, open_serve_challenge_with_bond, serve_challenge_key,
    validate_respond_blob_sample_serve, AccountId, BlobError, BlobId, BlobSampleServeChallenge,
    BlobSampleServeChallengeStatus, MediaBlobDasSample, PHASE10_MAX_SERVE_CHALLENGES,
    PHASE10_SERVE_CHALLENGE_REGISTRY_DOMAIN,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SampleServeChallengeError {
    #[error("serve challenge registry full (max {max})")]
    Full { max: usize },
    #[error("no matching open serve challenge")]
    NotFound,
    #[error("serve challenge already closed")]
    Closed,
    #[error("media blob: {0}")]
    Blob(#[from] BlobError),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SampleServeChallengeRegistry {
    challenges: BTreeMap<String, BlobSampleServeChallenge>,
    /// Monotonic FailedServe slash ids (consensus-local; Phase 12).
    #[serde(default = "default_next_slash_id")]
    next_failed_serve_slash_id: u64,
}

fn default_next_slash_id() -> u64 {
    1
}

impl Default for SampleServeChallengeRegistry {
    fn default() -> Self {
        Self {
            challenges: BTreeMap::new(),
            next_failed_serve_slash_id: 1,
        }
    }
}

impl SampleServeChallengeRegistry {
    pub fn smt_key() -> [u8; 32] {
        let mut h = hasher();
        h.update(PHASE10_SERVE_CHALLENGE_REGISTRY_DOMAIN);
        *h.finalize().as_bytes()
    }

    pub fn is_empty(&self) -> bool {
        self.challenges.is_empty()
    }

    pub fn len(&self) -> usize {
        self.challenges.len()
    }

    pub fn commitment(&self) -> [u8; 32] {
        let mut h = hasher();
        h.update(PHASE10_SERVE_CHALLENGE_REGISTRY_DOMAIN);
        h.update(&bincode::serialize(self).unwrap_or_default());
        *h.finalize().as_bytes()
    }

    pub fn get(
        &self,
        blob_id: &BlobId,
        chunk_index: u32,
        provider: &AccountId,
        challenger: &AccountId,
    ) -> Option<&BlobSampleServeChallenge> {
        self.challenges
            .get(&serve_challenge_key(blob_id, chunk_index, provider, challenger))
    }

    pub fn get_mut(
        &mut self,
        blob_id: &BlobId,
        chunk_index: u32,
        provider: &AccountId,
        challenger: &AccountId,
    ) -> Option<&mut BlobSampleServeChallenge> {
        self.challenges
            .get_mut(&serve_challenge_key(blob_id, chunk_index, provider, challenger))
    }

    pub fn list_for_blob(
        &self,
        blob_id: &BlobId,
        tip_height: u64,
    ) -> Vec<&BlobSampleServeChallenge> {
        let want = blob_id_hex(blob_id);
        self.challenges
            .values()
            .filter(|c| blob_id_hex(&c.blob_id) == want)
            .filter(|c| {
                !(c.status == BlobSampleServeChallengeStatus::Open
                    && tip_height > c.deadline_height)
            })
            .collect()
    }

    /// Open a new challenge with bond / penalty amounts. Rejects duplicate open keys.
    pub fn challenge(
        &mut self,
        blob_id: BlobId,
        chunk_index: u32,
        chunk_count: u32,
        provider: AccountId,
        challenger: AccountId,
        tip_height: u64,
        bond_amount: u128,
        provider_penalty: u128,
    ) -> Result<BlobSampleServeChallenge, SampleServeChallengeError> {
        let _ = self.take_newly_failed(tip_height);
        let c = open_serve_challenge_with_bond(
            blob_id,
            chunk_index,
            chunk_count,
            provider,
            challenger,
            tip_height,
            bond_amount,
            provider_penalty,
        )?;
        let key = c.market_key();
        if let Some(existing) = self.challenges.get(&key) {
            if existing.status == BlobSampleServeChallengeStatus::Open {
                return Err(SampleServeChallengeError::Closed);
            }
        }
        if !self.challenges.contains_key(&key)
            && self.challenges.len() >= PHASE10_MAX_SERVE_CHALLENGES
        {
            return Err(SampleServeChallengeError::Full {
                max: PHASE10_MAX_SERVE_CHALLENGES,
            });
        }
        self.challenges.insert(key, c.clone());
        Ok(c)
    }

    /// Provider responds with a verified DAS sample.
    pub fn respond(
        &mut self,
        blob_id: &BlobId,
        chunk_index: u32,
        provider: &AccountId,
        challenger: &AccountId,
        tip_height: u64,
        sample: &MediaBlobDasSample,
    ) -> Result<&BlobSampleServeChallenge, SampleServeChallengeError> {
        let _ = self.take_newly_failed(tip_height);
        let key = serve_challenge_key(blob_id, chunk_index, provider, challenger);
        let c = self
            .challenges
            .get_mut(&key)
            .ok_or(SampleServeChallengeError::NotFound)?;
        validate_respond_blob_sample_serve(c, tip_height, sample)?;
        c.status = BlobSampleServeChallengeStatus::Responded;
        Ok(c)
    }

    /// Mark open challenges past deadline as FailedServe; return those still needing bond settle.
    pub fn take_newly_failed(&mut self, tip_height: u64) -> Vec<BlobSampleServeChallenge> {
        let mut out = Vec::new();
        for c in self.challenges.values_mut() {
            if c.status == BlobSampleServeChallengeStatus::Open && tip_height > c.deadline_height {
                c.status = BlobSampleServeChallengeStatus::FailedServe;
            }
            if c.status == BlobSampleServeChallengeStatus::FailedServe && !c.bond_settled {
                out.push(c.clone());
            }
        }
        out
    }

    /// Mark open challenges past deadline as FailedServe (status only; prefer [`take_newly_failed`]).
    pub fn resolve_expired(&mut self, tip_height: u64) -> usize {
        self.take_newly_failed(tip_height).len()
    }

    pub fn mark_bond_settled(
        &mut self,
        blob_id: &BlobId,
        chunk_index: u32,
        provider: &AccountId,
        challenger: &AccountId,
    ) {
        if let Some(c) = self.get_mut(blob_id, chunk_index, provider, challenger) {
            c.bond_settled = true;
        }
    }

    /// Assign the next local FailedServe slash id to a settled challenge.
    ///
    /// Prefer [`Self::set_failed_serve_slash_id`] with a consensus-book id (Phase 15 shared
    /// allocator) so FailedServe ids stay non-colliding with equivocation/liveness rows.
    pub fn assign_failed_serve_slash_id(
        &mut self,
        blob_id: &BlobId,
        chunk_index: u32,
        provider: &AccountId,
        challenger: &AccountId,
    ) -> Option<u64> {
        let key = serve_challenge_key(blob_id, chunk_index, provider, challenger);
        if let Some(existing) = self.challenges.get(&key).and_then(|c| c.slash_id) {
            return Some(existing);
        }
        if !self.challenges.contains_key(&key) {
            return None;
        }
        if self.next_failed_serve_slash_id == 0 {
            self.next_failed_serve_slash_id = 1;
        }
        let id = self.next_failed_serve_slash_id;
        self.next_failed_serve_slash_id = id.saturating_add(1);
        self.challenges.get_mut(&key)?.slash_id = Some(id);
        Some(id)
    }

    /// Pin a caller-chosen consensus slash id onto a settled challenge (Phase 15).
    /// No-ops if already assigned. Bumps the local counter past `id` for hygiene.
    pub fn set_failed_serve_slash_id(
        &mut self,
        blob_id: &BlobId,
        chunk_index: u32,
        provider: &AccountId,
        challenger: &AccountId,
        id: u64,
    ) -> Option<u64> {
        let key = serve_challenge_key(blob_id, chunk_index, provider, challenger);
        if let Some(existing) = self.challenges.get(&key).and_then(|c| c.slash_id) {
            return Some(existing);
        }
        if !self.challenges.contains_key(&key) {
            return None;
        }
        self.challenges.get_mut(&key)?.slash_id = Some(id);
        if self.next_failed_serve_slash_id <= id {
            self.next_failed_serve_slash_id = id.saturating_add(1);
        }
        Some(id)
    }

    /// Challenges that have a slash_id (for node SlashRegistry mirror).
    pub fn challenges_with_slash_id(&self) -> Vec<&BlobSampleServeChallenge> {
        self.challenges
            .values()
            .filter(|c| c.slash_id.is_some())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use boing_primitives::{chunk_media_blob, media_blob_das_sample};

    #[test]
    fn challenge_respond_roundtrip() {
        let mut r = SampleServeChallengeRegistry::default();
        let data = vec![1u8; 70_000];
        let bundle = chunk_media_blob(&data).unwrap();
        let sample = media_blob_das_sample(&bundle, 0).unwrap();
        let provider = AccountId([2u8; 32]);
        let challenger = AccountId([3u8; 32]);
        r.challenge(
            bundle.blob_id,
            0,
            sample.chunk_count,
            provider,
            challenger,
            1,
            0,
            0,
        )
        .unwrap();
        r.respond(
            &bundle.blob_id,
            0,
            &provider,
            &challenger,
            2,
            &sample,
        )
        .unwrap();
        let c = r
            .get(&bundle.blob_id, 0, &provider, &challenger)
            .unwrap();
        assert_eq!(c.status, BlobSampleServeChallengeStatus::Responded);
    }

    #[test]
    fn expiry_marks_failed_serve() {
        let mut r = SampleServeChallengeRegistry::default();
        r.challenge(
            [9u8; 32],
            0,
            1,
            AccountId([2u8; 32]),
            AccountId([3u8; 32]),
            1,
            10,
            20,
        )
        .unwrap();
        let failed = r.take_newly_failed(10_000);
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].bond_amount, 10);
        assert_eq!(failed[0].provider_penalty, 20);
        assert_eq!(
            r.get(&[9u8; 32], 0, &AccountId([2u8; 32]), &AccountId([3u8; 32]))
                .unwrap()
                .status,
            BlobSampleServeChallengeStatus::FailedServe
        );
    }
}
