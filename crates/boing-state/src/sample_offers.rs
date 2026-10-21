//! SMT-resident media-blob sample-offer registry (Phase 9).
//!
//! Offers are committed as one SMT leaf once non-empty (names-registry pattern). Ask fees are
//! escrowed in a system account (`blob_sample_ask_escrow_account`); this registry tracks metadata
//! + escrowed amounts. Mutated only by Register / Settle / Cancel sample txs in the VM.

use std::collections::BTreeMap;

use boing_primitives::{
    blob_id_hex, hasher, sample_offer_market_key, settle_receipt_key, validate_pay_blob_sample,
    AccountId, BlobError, BlobId, MediaBlobSampleOffer, OnChainBlobSampleOffer,
    PHASE12_MAX_SETTLE_RECEIPTS, PHASE6_MAX_SAMPLE_OFFERS, PHASE9_SAMPLE_OFFER_REGISTRY_DOMAIN,
};
use serde::{Deserialize, Serialize};

/// Receipt that a payer settled a sample match (Phase 12 — gates Challenge).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettledSampleMatch {
    pub blob_id: BlobId,
    pub chunk_index: u32,
    pub chunk_count: u32,
    pub provider: AccountId,
    pub payer: AccountId,
    pub settled_at_height: u64,
}

/// Escrowed on-chain sample offer (provider prepaid `ask_fee` into the escrow account).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EscrowedSampleOffer {
    pub blob_id: BlobId,
    pub chunk_index: u32,
    pub chunk_count: u32,
    pub provider: AccountId,
    pub ask_fee: u128,
    pub expires_at_height: u64,
    pub registered_at_height: u64,
}

impl EscrowedSampleOffer {
    pub fn market_key(&self) -> String {
        sample_offer_market_key(&self.blob_id, self.chunk_index, &self.provider)
    }

    pub fn is_expired(&self, tip_height: u64) -> bool {
        self.expires_at_height > 0 && tip_height > self.expires_at_height
    }

    pub fn to_media_offer(&self) -> MediaBlobSampleOffer {
        MediaBlobSampleOffer {
            blob_id: self.blob_id,
            chunk_index: self.chunk_index,
            chunk_count: self.chunk_count,
            provider: self.provider,
            ask_fee: self.ask_fee,
            expires_at_height: self.expires_at_height,
        }
    }

    pub fn to_on_chain(&self) -> OnChainBlobSampleOffer {
        OnChainBlobSampleOffer {
            blob_id: self.blob_id,
            chunk_index: self.chunk_index,
            chunk_count: self.chunk_count,
            provider: self.provider,
            ask_fee: self.ask_fee,
            expires_at_height: self.expires_at_height,
            registered_at_height: self.registered_at_height,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SampleOfferRegistryError {
    #[error("sample market full (max {max} offers)")]
    MarketFull { max: usize },
    #[error("no matching sample offer")]
    NotFound,
    #[error("sample offer expired at height {expires_at_height} (tip {tip_height})")]
    Expired {
        expires_at_height: u64,
        tip_height: u64,
    },
    #[error("only the offer provider can cancel")]
    NotProvider,
    #[error("media blob: {0}")]
    Blob(#[from] BlobError),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SampleOfferRegistry {
    /// Key: `blob_hex:chunk:provider_hex` → escrowed offer.
    offers: BTreeMap<String, EscrowedSampleOffer>,
    /// Key: settle_receipt_key → settled match (Phase 12).
    #[serde(default)]
    settled_receipts: BTreeMap<String, SettledSampleMatch>,
}

impl SampleOfferRegistry {
    pub fn smt_key() -> [u8; 32] {
        let mut h = hasher();
        h.update(PHASE9_SAMPLE_OFFER_REGISTRY_DOMAIN);
        *h.finalize().as_bytes()
    }

    pub fn is_empty(&self) -> bool {
        self.offers.is_empty() && self.settled_receipts.is_empty()
    }

    pub fn len(&self) -> usize {
        self.offers.len()
    }

    pub fn settle_receipt_count(&self) -> usize {
        self.settled_receipts.len()
    }

    pub fn commitment(&self) -> [u8; 32] {
        let mut h = hasher();
        h.update(PHASE9_SAMPLE_OFFER_REGISTRY_DOMAIN);
        h.update(&bincode::serialize(self).unwrap_or_default());
        *h.finalize().as_bytes()
    }

    pub fn get(
        &self,
        blob_id: &BlobId,
        chunk_index: u32,
        provider: &AccountId,
    ) -> Option<&EscrowedSampleOffer> {
        self.offers
            .get(&sample_offer_market_key(blob_id, chunk_index, provider))
    }

    pub fn list(
        &self,
        blob_id: &BlobId,
        chunk_index: Option<u32>,
        tip_height: u64,
    ) -> Vec<&EscrowedSampleOffer> {
        let want = blob_id_hex(blob_id);
        self.offers
            .values()
            .filter(|o| {
                blob_id_hex(&o.blob_id) == want
                    && !o.is_expired(tip_height)
                    && chunk_index.is_none_or(|i| o.chunk_index == i)
            })
            .collect()
    }

    pub fn best(
        &self,
        blob_id: &BlobId,
        chunk_index: u32,
        tip_height: u64,
    ) -> Option<&EscrowedSampleOffer> {
        self.list(blob_id, Some(chunk_index), tip_height)
            .into_iter()
            .min_by_key(|o| o.ask_fee)
    }

    /// Insert offer. Returns previous offer at the same key (caller must refund its escrow).
    pub fn register(
        &mut self,
        offer: EscrowedSampleOffer,
    ) -> Result<Option<EscrowedSampleOffer>, SampleOfferRegistryError> {
        validate_pay_blob_sample(&offer.blob_id, offer.chunk_index, offer.chunk_count)?;
        let key = offer.market_key();
        let prev = self.offers.remove(&key);
        if prev.is_none() && self.offers.len() >= PHASE6_MAX_SAMPLE_OFFERS {
            return Err(SampleOfferRegistryError::MarketFull {
                max: PHASE6_MAX_SAMPLE_OFFERS,
            });
        }
        self.offers.insert(key, offer);
        Ok(prev)
    }

    /// Remove on settle; errors if missing or expired.
    pub fn settle(
        &mut self,
        blob_id: &BlobId,
        chunk_index: u32,
        provider: &AccountId,
        tip_height: u64,
    ) -> Result<EscrowedSampleOffer, SampleOfferRegistryError> {
        let key = sample_offer_market_key(blob_id, chunk_index, provider);
        let offer = self
            .offers
            .remove(&key)
            .ok_or(SampleOfferRegistryError::NotFound)?;
        if offer.is_expired(tip_height) {
            // Put it back so expire-refund path can reclaim; or treat as expired error.
            // Prefer: return Expired without re-insert — funds recovered via prune_expired.
            self.offers.insert(key, offer.clone());
            return Err(SampleOfferRegistryError::Expired {
                expires_at_height: offer.expires_at_height,
                tip_height,
            });
        }
        Ok(offer)
    }

    /// Provider cancel: remove and return offer for escrow refund.
    pub fn cancel(
        &mut self,
        blob_id: &BlobId,
        chunk_index: u32,
        provider: &AccountId,
    ) -> Result<EscrowedSampleOffer, SampleOfferRegistryError> {
        let key = sample_offer_market_key(blob_id, chunk_index, provider);
        self.offers
            .remove(&key)
            .ok_or(SampleOfferRegistryError::NotFound)
    }

    /// Drop expired offers; returns them so the VM can refund escrow.
    pub fn take_expired(&mut self, tip_height: u64) -> Vec<EscrowedSampleOffer> {
        let mut out = Vec::new();
        self.offers.retain(|_, o| {
            if o.is_expired(tip_height) {
                out.push(o.clone());
                false
            } else {
                true
            }
        });
        out
    }

    /// Record a settle receipt so the payer may later Challenge the provider.
    pub fn record_settle_receipt(
        &mut self,
        receipt: SettledSampleMatch,
    ) -> Result<(), SampleOfferRegistryError> {
        let key = settle_receipt_key(
            &receipt.blob_id,
            receipt.chunk_index,
            &receipt.provider,
            &receipt.payer,
        );
        if !self.settled_receipts.contains_key(&key)
            && self.settled_receipts.len() >= PHASE12_MAX_SETTLE_RECEIPTS
        {
            // Evict oldest by settled_at_height to admit new receipt.
            if let Some(oldest_key) = self
                .settled_receipts
                .iter()
                .min_by_key(|(_, r)| r.settled_at_height)
                .map(|(k, _)| k.clone())
            {
                self.settled_receipts.remove(&oldest_key);
            }
        }
        self.settled_receipts.insert(key, receipt);
        Ok(())
    }

    pub fn has_settle_receipt(
        &self,
        blob_id: &BlobId,
        chunk_index: u32,
        provider: &AccountId,
        payer: &AccountId,
    ) -> bool {
        self.settled_receipts.contains_key(&settle_receipt_key(
            blob_id, chunk_index, provider, payer,
        ))
    }

    pub fn get_settle_receipt(
        &self,
        blob_id: &BlobId,
        chunk_index: u32,
        provider: &AccountId,
        payer: &AccountId,
    ) -> Option<&SettledSampleMatch> {
        self.settled_receipts.get(&settle_receipt_key(
            blob_id, chunk_index, provider, payer,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_settle_roundtrip() {
        let mut r = SampleOfferRegistry::default();
        let blob = [9u8; 32];
        let provider = AccountId([1u8; 32]);
        r.register(EscrowedSampleOffer {
            blob_id: blob,
            chunk_index: 0,
            chunk_count: 2,
            provider,
            ask_fee: 10,
            expires_at_height: 100,
            registered_at_height: 1,
        })
        .unwrap();
        assert_eq!(r.len(), 1);
        let o = r.settle(&blob, 0, &provider, 50).unwrap();
        assert_eq!(o.ask_fee, 10);
        assert!(r.is_empty());
    }

    #[test]
    fn commitment_changes() {
        let mut r = SampleOfferRegistry::default();
        let empty = r.commitment();
        r.register(EscrowedSampleOffer {
            blob_id: [1u8; 32],
            chunk_index: 0,
            chunk_count: 1,
            provider: AccountId([2u8; 32]),
            ask_fee: 1,
            expires_at_height: 0,
            registered_at_height: 0,
        })
        .unwrap();
        assert_ne!(empty, r.commitment());
    }
}
