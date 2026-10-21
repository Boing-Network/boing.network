//! SMT-resident sample-serve challenge registry (Phase 10).
//!
//! After settle, a challenger can open a window for the provider to prove availability
//! via a DAS merkle sample. FailedServe is a consensus signal — no automatic slash yet.

use std::collections::BTreeMap;

use boing_primitives::{
    blob_id_hex, hasher, open_serve_challenge, serve_challenge_key,
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

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SampleServeChallengeRegistry {
    challenges: BTreeMap<String, BlobSampleServeChallenge>,
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

    pub fn list_for_blob(
        &self,
        blob_id: &BlobId,
        tip_height: u64,
    ) -> Vec<&BlobSampleServeChallenge> {
        let want = blob_id_hex(blob_id);
        // resolve expired views for listing without mutating
        self.challenges
            .values()
            .filter(|c| blob_id_hex(&c.blob_id) == want)
            .filter(|c| {
                !(c.status == BlobSampleServeChallengeStatus::Open
                    && tip_height > c.deadline_height)
            })
            .collect()
    }

    /// Open a new challenge. Rejects duplicate open keys.
    pub fn challenge(
        &mut self,
        blob_id: BlobId,
        chunk_index: u32,
        chunk_count: u32,
        provider: AccountId,
        challenger: AccountId,
        tip_height: u64,
    ) -> Result<BlobSampleServeChallenge, SampleServeChallengeError> {
        self.resolve_expired(tip_height);
        let c = open_serve_challenge(
            blob_id,
            chunk_index,
            chunk_count,
            provider,
            challenger,
            tip_height,
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
    ) -> Result<(), SampleServeChallengeError> {
        self.resolve_expired(tip_height);
        let key = serve_challenge_key(blob_id, chunk_index, provider, challenger);
        let c = self
            .challenges
            .get_mut(&key)
            .ok_or(SampleServeChallengeError::NotFound)?;
        validate_respond_blob_sample_serve(c, tip_height, sample)?;
        c.status = BlobSampleServeChallengeStatus::Responded;
        Ok(())
    }

    /// Mark open challenges past deadline as FailedServe.
    pub fn resolve_expired(&mut self, tip_height: u64) -> usize {
        let mut n = 0;
        for c in self.challenges.values_mut() {
            if c.status == BlobSampleServeChallengeStatus::Open && tip_height > c.deadline_height {
                c.status = BlobSampleServeChallengeStatus::FailedServe;
                n += 1;
            }
        }
        n
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
        )
        .unwrap();
        let n = r.resolve_expired(10_000);
        assert_eq!(n, 1);
        assert_eq!(
            r.get(&[9u8; 32], 0, &AccountId([2u8; 32]), &AccountId([3u8; 32]))
                .unwrap()
                .status,
            BlobSampleServeChallengeStatus::FailedServe
        );
    }
}
