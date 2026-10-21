//! Phase 8: fuller (consensus-replay) sample market, versioned succinct DAS proof path,
//! and public-testnet 256 MiB activation notes helpers.
//!
//! Honest scope:
//! - **Sample market:** a replay-identical offer book rebuilt from on-chain Register/Settle
//!   (not SMT-resident yet). Local RPC puts stay on a separate gossip book.
//! - **Succinct DAS:** versioned proof envelope; `StubBlake3V0` is the only implemented kind
//!   (BLAKE3 commitment — **not** a SNARK). Reserved kinds reject until wired.
//! - **Cap:** default remains 16 MiB; activation checklist helpers do not change consensus.

use serde::{Deserialize, Serialize};

use crate::blobs::{
    media_blob_das_sample, verify_media_blob_das_sample, BlobError, BlobId, MediaBlobErasureBundle,
};
use crate::blobs_phase6::{
    das_light_client_sample_indices, MediaBlobDasLightClientBatch, MediaBlobSampleMarket,
    MediaBlobSampleMatch, MediaBlobSampleOffer, PHASE6_MAX_SAMPLE_OFFERS,
};
use crate::blobs_phase7::{
    das_light_client_sample_indices_vrf, das_light_client_seed_from_vrf,
    das_succinct_commitment_stub, validate_settle_blob_sample_match, OnChainBlobSampleOffer,
    PHASE7_ACTIVATED_MAX_MEDIA_BLOB_BYTES, PHASE7_CAP_ACTIVATION_ENV,
};
use crate::hasher;
use crate::AccountId;

/// Domain for Phase 8 succinct-proof envelope hashing.
pub const PHASE8_DAS_SUCCINCT_PROOF_DOMAIN: &[u8] = b"BOING_BLOB_DAS_SUCCINCT_PROOF_V0";

/// Env name for the gated 256 MiB raise (same as Phase 7 — unchanged).
pub const PHASE8_CAP_ACTIVATION_ENV: &str = PHASE7_CAP_ACTIVATION_ENV;

/// Activated max after coordinated height (256 MiB).
pub const PHASE8_ACTIVATED_MAX_MEDIA_BLOB_BYTES: usize = PHASE7_ACTIVATED_MAX_MEDIA_BLOB_BYTES;

/// Where a sample offer originated for discovery UX.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BlobSampleOfferSource {
    /// Rebuilt from `RegisterBlobSampleOffer` / removed by `SettleBlobSampleMatch`.
    OnChain,
    /// Node-local RPC / gossip upsert (`boing_putBlobSampleOffer`).
    Local,
}

/// Settlement fee breakdown for a matched sample (protocol fee + provider ask).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlobSampleSettleQuote {
    pub protocol_sample_fee: u128,
    pub ask_fee: u128,
    pub total: u128,
    pub provider: AccountId,
    pub blob_id: BlobId,
    pub chunk_index: u32,
    pub chunk_count: u32,
}

impl BlobSampleSettleQuote {
    pub fn from_offer(offer: &MediaBlobSampleOffer, protocol_sample_fee: u128) -> Self {
        let ask = offer.ask_fee;
        Self {
            protocol_sample_fee,
            ask_fee: ask,
            total: protocol_sample_fee.saturating_add(ask),
            provider: offer.provider,
            blob_id: offer.blob_id,
            chunk_index: offer.chunk_index,
            chunk_count: offer.chunk_count,
        }
    }

    pub fn from_on_chain(offer: &OnChainBlobSampleOffer, protocol_sample_fee: u128) -> Self {
        let ask = offer.ask_fee;
        Self {
            protocol_sample_fee,
            ask_fee: ask,
            total: protocol_sample_fee.saturating_add(ask),
            provider: offer.provider,
            blob_id: offer.blob_id,
            chunk_index: offer.chunk_index,
            chunk_count: offer.chunk_count,
        }
    }
}

/// Consensus-replay sample market: same APIs as Phase 6 book, but intended only for
/// Register/Settle ingest so honest full nodes converge.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsensusNativeSampleMarket {
    inner: MediaBlobSampleMarket,
}

impl ConsensusNativeSampleMarket {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn as_market(&self) -> &MediaBlobSampleMarket {
        &self.inner
    }

    pub fn upsert_from_register(
        &mut self,
        offer: MediaBlobSampleOffer,
    ) -> Result<(), BlobError> {
        self.inner.upsert_offer(offer)
    }

    pub fn remove_on_settle(
        &mut self,
        blob_id: &BlobId,
        chunk_index: u32,
        provider: &AccountId,
    ) -> bool {
        self.inner.remove_offer(blob_id, chunk_index, provider)
    }

    pub fn prune_expired(&mut self, tip_height: u64) -> usize {
        self.inner.prune_expired(tip_height)
    }

    pub fn list_offers(
        &self,
        blob_id: &BlobId,
        chunk_index: Option<u32>,
        tip_height: u64,
    ) -> Vec<&MediaBlobSampleOffer> {
        self.inner.list_offers(blob_id, chunk_index, tip_height)
    }

    pub fn best_offer(
        &self,
        blob_id: &BlobId,
        chunk_index: u32,
        tip_height: u64,
    ) -> Option<&MediaBlobSampleOffer> {
        self.inner.best_offer(blob_id, chunk_index, tip_height)
    }

    /// Match against the consensus-replay book (prefer this over local gossip).
    pub fn match_settle(
        &self,
        blob_id: &BlobId,
        chunk_index: u32,
        chunk_count: u32,
        payer: AccountId,
        paid_fee: u128,
        tip_height: u64,
        provider: Option<&AccountId>,
        protocol_sample_fee: u128,
    ) -> Result<(MediaBlobSampleMatch, BlobSampleSettleQuote), BlobError> {
        let m = self.inner.match_pay_blob_sample(
            blob_id,
            chunk_index,
            chunk_count,
            payer,
            paid_fee,
            tip_height,
            provider,
        )?;
        let quote = BlobSampleSettleQuote::from_offer(&m.offer, protocol_sample_fee);
        if paid_fee < quote.total {
            return Err(BlobError::SampleFeeTooLow {
                paid: paid_fee,
                ask: quote.total,
            });
        }
        // Cross-check Phase 7 settle validation shape.
        let on_chain = OnChainBlobSampleOffer {
            blob_id: m.offer.blob_id,
            chunk_index: m.offer.chunk_index,
            chunk_count: m.offer.chunk_count,
            provider: m.offer.provider,
            ask_fee: m.offer.ask_fee,
            expires_at_height: m.offer.expires_at_height,
            registered_at_height: 0,
        };
        validate_settle_blob_sample_match(&on_chain, &m.payer, tip_height, &m.offer.provider)?;
        Ok((m, quote))
    }
}

/// Prefer consensus-replay book, fall back to local gossip book.
pub fn match_sample_prefer_on_chain(
    on_chain: &ConsensusNativeSampleMarket,
    local: &MediaBlobSampleMarket,
    blob_id: &BlobId,
    chunk_index: u32,
    chunk_count: u32,
    payer: AccountId,
    paid_fee: u128,
    tip_height: u64,
    provider: Option<&AccountId>,
    protocol_sample_fee: u128,
) -> Result<(MediaBlobSampleMatch, BlobSampleSettleQuote, BlobSampleOfferSource), BlobError> {
    match on_chain.match_settle(
        blob_id,
        chunk_index,
        chunk_count,
        payer,
        paid_fee,
        tip_height,
        provider,
        protocol_sample_fee,
    ) {
        Ok((m, q)) => Ok((m, q, BlobSampleOfferSource::OnChain)),
        Err(BlobError::SampleOfferNotFound) => {
            let m = local.match_pay_blob_sample(
                blob_id,
                chunk_index,
                chunk_count,
                payer,
                paid_fee,
                tip_height,
                provider,
            )?;
            let quote = BlobSampleSettleQuote::from_offer(&m.offer, protocol_sample_fee);
            if paid_fee < quote.total {
                return Err(BlobError::SampleFeeTooLow {
                    paid: paid_fee,
                    ask: quote.total,
                });
            }
            Ok((m, quote, BlobSampleOfferSource::Local))
        }
        Err(e) => Err(e),
    }
}

/// Kind discriminant for versioned succinct DAS proofs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum DasSuccinctProofKind {
    /// BLAKE3 commitment stub (Phase 7/8). **Not** a SNARK.
    StubBlake3V0 = 0,
    /// Reserved: future real succinct system (IPA / KZG / SNARK).
    ReservedRealV1 = 1,
}

impl DasSuccinctProofKind {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::StubBlake3V0),
            1 => Some(Self::ReservedRealV1),
            _ => None,
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn is_implemented(self) -> bool {
        matches!(self, Self::StubBlake3V0)
    }
}

/// Versioned succinct DAS proof envelope (honest: stub kind only today).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DasSuccinctProof {
    pub kind: u8,
    pub commitment: [u8; 32],
    pub blob_id: BlobId,
    pub chunks_root: [u8; 32],
    /// Height binding when seed is height-derived; `0` when VRF-bound.
    pub seed_height: u64,
    /// Optional VRF output used for index selection (when non-zero / present).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vrf_output: Option<[u8; 32]>,
    pub sample_indices: Vec<u32>,
}

impl DasSuccinctProof {
    pub fn kind_enum(&self) -> Option<DasSuccinctProofKind> {
        DasSuccinctProofKind::from_u8(self.kind)
    }
}

/// Build a stub succinct proof over a verified LC batch.
pub fn build_das_succinct_proof_stub(batch: &MediaBlobDasLightClientBatch) -> DasSuccinctProof {
    let commitment = das_succinct_commitment_stub(batch);
    DasSuccinctProof {
        kind: DasSuccinctProofKind::StubBlake3V0.as_u8(),
        commitment,
        blob_id: batch.blob_id,
        chunks_root: batch.chunks_root,
        seed_height: batch.seed_height,
        vrf_output: None,
        sample_indices: batch.samples.iter().map(|s| s.chunk_index).collect(),
    }
}

/// Build stub proof for a VRF-bound batch (`seed_height` left 0; `vrf_output` recorded).
pub fn build_das_succinct_proof_stub_vrf(
    batch: &MediaBlobDasLightClientBatch,
    vrf_output: &[u8; 32],
) -> DasSuccinctProof {
    let mut p = build_das_succinct_proof_stub(batch);
    p.seed_height = 0;
    p.vrf_output = Some(*vrf_output);
    // Re-bind commitment with VRF tag so height-bound and VRF proofs do not collide.
    let mut h = hasher();
    h.update(PHASE8_DAS_SUCCINCT_PROOF_DOMAIN);
    h.update(&[DasSuccinctProofKind::StubBlake3V0.as_u8()]);
    h.update(&p.commitment);
    h.update(vrf_output);
    let mut out = [0u8; 32];
    out.copy_from_slice(h.finalize().as_bytes());
    p.commitment = out;
    p
}

/// Verify a succinct proof against a batch.
///
/// - `StubBlake3V0`: recomputes stub commitment (+ optional VRF rebind) and checks merkle samples.
/// - `ReservedRealV1`: always errors (`DasProofInvalid`) until a real system is wired.
pub fn verify_das_succinct_proof(
    proof: &DasSuccinctProof,
    batch: &MediaBlobDasLightClientBatch,
) -> Result<(), BlobError> {
    batch.validate_structure()?;
    if proof.blob_id != batch.blob_id || proof.chunks_root != batch.chunks_root {
        return Err(BlobError::DasProofInvalid);
    }
    let idxs: Vec<u32> = batch.samples.iter().map(|s| s.chunk_index).collect();
    if idxs != proof.sample_indices {
        return Err(BlobError::DasProofInvalid);
    }
    for s in &batch.samples {
        verify_media_blob_das_sample(s)?;
    }
    match proof.kind_enum() {
        Some(DasSuccinctProofKind::StubBlake3V0) => {
            let expected = if let Some(vrf) = proof.vrf_output {
                build_das_succinct_proof_stub_vrf(batch, &vrf).commitment
            } else {
                if proof.seed_height != batch.seed_height {
                    return Err(BlobError::DasProofInvalid);
                }
                das_succinct_commitment_stub(batch)
            };
            if expected != proof.commitment {
                return Err(BlobError::DasProofInvalid);
            }
            Ok(())
        }
        Some(DasSuccinctProofKind::ReservedRealV1) | None => Err(BlobError::DasProofInvalid),
    }
}

/// VRF-bound DAS light-client batch (caller must have verified ECVRF on `vrf_output`).
pub fn build_das_light_client_batch_vrf(
    bundle: &MediaBlobErasureBundle,
    vrf_output: &[u8; 32],
    sample_count: usize,
) -> Result<MediaBlobDasLightClientBatch, BlobError> {
    let _seed = das_light_client_seed_from_vrf(&bundle.blob_id, vrf_output);
    let idxs = das_light_client_sample_indices_vrf(
        bundle.original_len as usize,
        sample_count,
        &bundle.blob_id,
        vrf_output,
    );
    if idxs.is_empty() {
        return Err(BlobError::ChunkIndexOutOfRange {
            index: 0,
            count: 0,
        });
    }
    let mut samples = Vec::with_capacity(idxs.len());
    for i in idxs {
        samples.push(media_blob_das_sample(bundle, i)?);
    }
    let batch = MediaBlobDasLightClientBatch {
        blob_id: bundle.blob_id,
        chunks_root: bundle.chunks_root.0,
        // VRF path: height binding unused; indices come from VRF seed.
        seed_height: 0,
        samples,
    };
    batch.validate_structure()?;
    Ok(batch)
}

/// Operator checklist lines for coordinated public-testnet 256 MiB activation.
/// Default must remain unset (16 MiB / Fly-safe) until every validator agrees on `H`.
pub fn phase8_cap_activation_checklist(activation_height: Option<u64>) -> Vec<&'static str> {
    let _ = activation_height;
    vec![
        "Leave BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT unset unless every public-testnet validator agrees (Fly-safe default = 16 MiB).",
        "Agree on identical activation height H; deploy the same binary that honors media_blob_max_bytes_at.",
        "Set the env (or --media-blobs-phase7-cap-activation-height) on ALL validators and RPC nodes, then restart so the knob is live.",
        "After tip >= H, confirm boing_getMediaBlobParams: phase7CapActiveAtTip=true and effectiveMaxBytesAtTip=268435456 (256 MiB).",
        "Ensure disk/RAM can absorb 256 MiB Put/Announce + 64 KiB DAS chunks + RS parity=2 before enabling.",
        "Mismatched caps across validators fork admission — do not partial-roll the env.",
        "Update tools/boing-node-public-testnet.env.example and docs once H is chosen and activated.",
    ]
}

/// Max offers tracked (same admission as Phase 6 book).
pub const PHASE8_MAX_SAMPLE_OFFERS: usize = PHASE6_MAX_SAMPLE_OFFERS;

/// Re-export height-seed helper for SDK parity tests (indices from a raw seed).
pub fn das_indices_from_seed(blob_size: usize, sample_count: usize, seed: u64) -> Vec<u32> {
    das_light_client_sample_indices(blob_size, sample_count, seed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blobs::chunk_media_blob;
    use crate::blobs_phase6::build_das_light_client_batch;

    #[test]
    fn consensus_market_match_quotes_total() {
        let mut m = ConsensusNativeSampleMarket::new();
        let blob = [7u8; 32];
        m.upsert_from_register(MediaBlobSampleOffer {
            blob_id: blob,
            chunk_index: 0,
            chunk_count: 2,
            provider: AccountId([1u8; 32]),
            ask_fee: 50,
            expires_at_height: 0,
        })
        .unwrap();
        let (_match, quote) = m
            .match_settle(
                &blob,
                0,
                2,
                AccountId([2u8; 32]),
                60,
                10,
                None,
                10,
            )
            .unwrap();
        assert_eq!(quote.protocol_sample_fee, 10);
        assert_eq!(quote.ask_fee, 50);
        assert_eq!(quote.total, 60);
        assert!(m
            .match_settle(
                &blob,
                0,
                2,
                AccountId([2u8; 32]),
                59,
                10,
                None,
                10,
            )
            .is_err());
    }

    #[test]
    fn prefer_on_chain_over_local() {
        let mut on = ConsensusNativeSampleMarket::new();
        let mut local = MediaBlobSampleMarket::new();
        let blob = [3u8; 32];
        on.upsert_from_register(MediaBlobSampleOffer {
            blob_id: blob,
            chunk_index: 0,
            chunk_count: 1,
            provider: AccountId([1u8; 32]),
            ask_fee: 1,
            expires_at_height: 0,
        })
        .unwrap();
        local
            .upsert_offer(MediaBlobSampleOffer {
                blob_id: blob,
                chunk_index: 0,
                chunk_count: 1,
                provider: AccountId([9u8; 32]),
                ask_fee: 0,
                expires_at_height: 0,
            })
            .unwrap();
        let (m, _q, src) = match_sample_prefer_on_chain(
            &on,
            &local,
            &blob,
            0,
            1,
            AccountId([2u8; 32]),
            1,
            1,
            None,
            0,
        )
        .unwrap();
        assert_eq!(src, BlobSampleOfferSource::OnChain);
        assert_eq!(m.offer.provider, AccountId([1u8; 32]));
    }

    #[test]
    fn succinct_stub_verify_roundtrip() {
        let data = vec![5u8; 70_000];
        let bundle = chunk_media_blob(&data).unwrap();
        let batch = build_das_light_client_batch(&bundle, 42, 4).unwrap();
        let proof = build_das_succinct_proof_stub(&batch);
        assert_eq!(proof.kind, 0);
        verify_das_succinct_proof(&proof, &batch).unwrap();
        let mut bad = proof.clone();
        bad.commitment[0] ^= 1;
        assert!(verify_das_succinct_proof(&bad, &batch).is_err());
    }

    #[test]
    fn vrf_batch_and_proof() {
        let data = vec![9u8; 80_000];
        let bundle = chunk_media_blob(&data).unwrap();
        let vrf = [0xabu8; 32];
        let batch = build_das_light_client_batch_vrf(&bundle, &vrf, 3).unwrap();
        assert_eq!(batch.seed_height, 0);
        let proof = build_das_succinct_proof_stub_vrf(&batch, &vrf);
        assert!(proof.vrf_output.is_some());
        verify_das_succinct_proof(&proof, &batch).unwrap();
    }

    #[test]
    fn reserved_kind_rejects() {
        let data = vec![1u8; 65_000];
        let bundle = chunk_media_blob(&data).unwrap();
        let batch = build_das_light_client_batch(&bundle, 1, 2).unwrap();
        let mut proof = build_das_succinct_proof_stub(&batch);
        proof.kind = DasSuccinctProofKind::ReservedRealV1.as_u8();
        assert!(verify_das_succinct_proof(&proof, &batch).is_err());
    }

    #[test]
    fn activation_checklist_nonempty() {
        assert!(phase8_cap_activation_checklist(None).len() >= 5);
        assert_eq!(
            PHASE8_ACTIVATED_MAX_MEDIA_BLOB_BYTES,
            256 * 1024 * 1024
        );
    }
}
