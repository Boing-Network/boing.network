//! Node sidecar for L1 media blob bytes (`data/blobs/`).
//!
//! Consensus PutBlob txs carry the payload in the block; this module persists bytes + metadata for
//! `boing_getBlob` / `boing_getBlobMeta`. Not part of the account SMT.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use boing_primitives::{
    blob_id_hex, blob_id_of, AccountId, BlobDownloadAcl, BlobId, BlobRetention,
    ConsensusNativeSampleMarket, MediaBlobSampleMarket, MediaBlobSampleOffer, Transaction,
    TransactionPayload,
};
use serde::{Deserialize, Serialize};

const BLOBS_DIR: &str = "blobs";
const INDEX_FILE: &str = "index.bin";
const ACCESS_INDEX_FILE: &str = "access_index.bin";
const SAMPLE_OFFERS_FILE: &str = "sample_offers.bin";
/// Phase 8: offers rebuilt only from Register/Settle (consensus-replay identical).
const ONCHAIN_SAMPLE_OFFERS_FILE: &str = "sample_offers_onchain.bin";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlobMetaRecord {
    pub blob_id: BlobId,
    pub size: u32,
    pub content_type: String,
    pub retention: BlobRetention,
    pub publisher: [u8; 32],
    pub included_height: u64,
    pub tx_id: [u8; 32],
    /// Phase 5: optional paid pin horizon (renewals extend this). When set, overrides prunable TTL
    /// for GC; Forever blobs with `None` stay best-effort until a renew sets a horizon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin_until_height: Option<u64>,
}

impl BlobMetaRecord {
    pub fn expiry_height(&self) -> Option<u64> {
        if let Some(p) = self.pin_until_height {
            return Some(p);
        }
        self.retention.expiry_height(self.included_height)
    }

    pub fn is_expired(&self, tip_height: u64) -> bool {
        self.expiry_height().is_some_and(|e| tip_height > e)
    }
}

#[derive(Default, Serialize, Deserialize)]
struct BlobIndex {
    entries: HashMap<String, BlobMetaRecord>,
}

#[derive(Default, Serialize, Deserialize)]
struct BlobAccessIndex {
    /// blob_id_hex → ACL (publisher is always allowed via meta, not stored here).
    entries: HashMap<String, BlobDownloadAcl>,
}

/// Disk-backed media blob sidecar under `{data_dir}/blobs/`.
pub struct MediaBlobStore {
    dir: PathBuf,
    index: RwLock<BlobIndex>,
    access: RwLock<BlobAccessIndex>,
    /// Phase 6: node-local sample offer book (RPC / gossip — not consensus).
    sample_market: RwLock<MediaBlobSampleMarket>,
    /// Phase 8: consensus-replay book (Register/Settle only).
    onchain_sample_market: RwLock<ConsensusNativeSampleMarket>,
}

impl MediaBlobStore {
    pub fn open(data_dir: impl AsRef<Path>) -> std::io::Result<Self> {
        let dir = data_dir.as_ref().join(BLOBS_DIR);
        fs::create_dir_all(&dir)?;
        let index_path = dir.join(INDEX_FILE);
        let index = if index_path.exists() {
            let bytes = fs::read(&index_path)?;
            bincode::deserialize(&bytes).unwrap_or_default()
        } else {
            BlobIndex::default()
        };
        let access_path = dir.join(ACCESS_INDEX_FILE);
        let access = if access_path.exists() {
            let bytes = fs::read(&access_path)?;
            bincode::deserialize(&bytes).unwrap_or_default()
        } else {
            BlobAccessIndex::default()
        };
        let offers_path = dir.join(SAMPLE_OFFERS_FILE);
        let sample_market = if offers_path.exists() {
            let bytes = fs::read(&offers_path)?;
            bincode::deserialize(&bytes).unwrap_or_default()
        } else {
            MediaBlobSampleMarket::default()
        };
        let onchain_path = dir.join(ONCHAIN_SAMPLE_OFFERS_FILE);
        let onchain_sample_market = if onchain_path.exists() {
            let bytes = fs::read(&onchain_path)?;
            bincode::deserialize(&bytes).unwrap_or_default()
        } else {
            ConsensusNativeSampleMarket::default()
        };
        Ok(Self {
            dir,
            index: RwLock::new(index),
            access: RwLock::new(access),
            sample_market: RwLock::new(sample_market),
            onchain_sample_market: RwLock::new(onchain_sample_market),
        })
    }

    fn index_path(&self) -> PathBuf {
        self.dir.join(INDEX_FILE)
    }

    fn access_index_path(&self) -> PathBuf {
        self.dir.join(ACCESS_INDEX_FILE)
    }

    fn persist_access(&self, access: &BlobAccessIndex) -> std::io::Result<()> {
        let bytes = bincode::serialize(access)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        fs::write(self.access_index_path(), bytes)
    }

    pub fn get_access(&self, id: &BlobId) -> BlobDownloadAcl {
        let access = self.access.read().unwrap_or_else(|e| e.into_inner());
        access
            .entries
            .get(&blob_id_hex(id))
            .cloned()
            .unwrap_or_default()
    }

    /// Publisher or ACL grantee may download.
    pub fn may_download(&self, id: &BlobId, account: &AccountId) -> bool {
        if let Some(meta) = self.get_meta(id) {
            if meta.publisher == account.0 {
                return true;
            }
        } else {
            return false;
        }
        self.get_access(id).contains(account)
    }

    pub fn grant_download(
        &self,
        blob_id: &BlobId,
        publisher: &AccountId,
        grantee: AccountId,
    ) -> Result<bool, String> {
        let meta = self
            .get_meta(blob_id)
            .ok_or_else(|| "blob not in sidecar".to_string())?;
        if meta.publisher != publisher.0 {
            return Err("only the blob publisher may grant download access".into());
        }
        if grantee.0 == meta.publisher {
            return Ok(false);
        }
        let key = blob_id_hex(blob_id);
        let mut access = self.access.write().unwrap_or_else(|e| e.into_inner());
        let entry = access.entries.entry(key).or_default();
        let changed = entry.grant(grantee).map_err(|e| e.to_string())?;
        if changed {
            self.persist_access(&access).map_err(|e| e.to_string())?;
        }
        Ok(changed)
    }

    pub fn revoke_download(
        &self,
        blob_id: &BlobId,
        publisher: &AccountId,
        grantee: &AccountId,
    ) -> Result<bool, String> {
        let meta = self
            .get_meta(blob_id)
            .ok_or_else(|| "blob not in sidecar".to_string())?;
        if meta.publisher != publisher.0 {
            return Err("only the blob publisher may revoke download access".into());
        }
        let key = blob_id_hex(blob_id);
        let mut access = self.access.write().unwrap_or_else(|e| e.into_inner());
        let Some(entry) = access.entries.get_mut(&key) else {
            return Ok(false);
        };
        let changed = entry.revoke(grantee);
        if changed {
            if entry.grantees.is_empty() && entry.epoch == 0 {
                access.entries.remove(&key);
            }
            self.persist_access(&access).map_err(|e| e.to_string())?;
        }
        Ok(changed)
    }

    fn sample_offers_path(&self) -> PathBuf {
        self.dir.join(SAMPLE_OFFERS_FILE)
    }

    fn onchain_sample_offers_path(&self) -> PathBuf {
        self.dir.join(ONCHAIN_SAMPLE_OFFERS_FILE)
    }

    fn persist_sample_market(&self, market: &MediaBlobSampleMarket) -> std::io::Result<()> {
        let bytes = bincode::serialize(market)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        fs::write(self.sample_offers_path(), bytes)
    }

    fn persist_onchain_sample_market(
        &self,
        market: &ConsensusNativeSampleMarket,
    ) -> std::io::Result<()> {
        let bytes = bincode::serialize(market)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        fs::write(self.onchain_sample_offers_path(), bytes)
    }

    /// Upsert a Phase 6 sample offer into the node-local book.
    pub fn upsert_sample_offer(&self, offer: MediaBlobSampleOffer) -> Result<(), String> {
        let mut m = self
            .sample_market
            .write()
            .unwrap_or_else(|e| e.into_inner());
        m.upsert_offer(offer).map_err(|e| e.to_string())?;
        self.persist_sample_market(&m).map_err(|e| e.to_string())
    }

    /// Upsert into the consensus-replay book (RegisterBlobSampleOffer ingest).
    pub fn upsert_onchain_sample_offer(&self, offer: MediaBlobSampleOffer) -> Result<(), String> {
        let mut m = self
            .onchain_sample_market
            .write()
            .unwrap_or_else(|e| e.into_inner());
        m.upsert_from_register(offer).map_err(|e| e.to_string())?;
        self.persist_onchain_sample_market(&m)
            .map_err(|e| e.to_string())
    }

    pub fn sample_market_snapshot(&self) -> MediaBlobSampleMarket {
        self.sample_market
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn onchain_sample_market_snapshot(&self) -> ConsensusNativeSampleMarket {
        self.onchain_sample_market
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn prune_sample_offers(&self, tip_height: u64) -> std::io::Result<usize> {
        let mut n = 0;
        {
            let mut m = self
                .sample_market
                .write()
                .unwrap_or_else(|e| e.into_inner());
            let k = m.prune_expired(tip_height);
            if k > 0 {
                self.persist_sample_market(&m)?;
            }
            n += k;
        }
        {
            let mut m = self
                .onchain_sample_market
                .write()
                .unwrap_or_else(|e| e.into_inner());
            let k = m.prune_expired(tip_height);
            if k > 0 {
                self.persist_onchain_sample_market(&m)?;
            }
            n += k;
        }
        Ok(n)
    }

    fn data_path(&self, id: &BlobId) -> PathBuf {
        self.dir.join(format!("{}.bin", blob_id_hex(id)))
    }

    fn persist_index(&self, index: &BlobIndex) -> std::io::Result<()> {
        let bytes = bincode::serialize(index)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        fs::write(self.index_path(), bytes)
    }

    /// Store bytes + metadata (idempotent if the same id already exists with matching hash).
    pub fn put(&self, meta: BlobMetaRecord, data: &[u8]) -> std::io::Result<()> {
        let id = blob_id_of(data);
        if id != meta.blob_id {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "blob id does not match data",
            ));
        }
        fs::create_dir_all(&self.dir)?;
        let path = self.data_path(&id);
        if !path.exists() {
            fs::write(&path, data)?;
        }
        let key = blob_id_hex(&id);
        let mut idx = self.index.write().unwrap_or_else(|e| e.into_inner());
        idx.entries.insert(key, meta);
        self.persist_index(&idx)
    }

    pub fn get_meta(&self, id: &BlobId) -> Option<BlobMetaRecord> {
        let idx = self.index.read().unwrap_or_else(|e| e.into_inner());
        idx.entries.get(&blob_id_hex(id)).cloned()
    }

    /// Cheap existence check for the blob's data file (no read).
    pub fn has_bytes(&self, id: &BlobId) -> bool {
        self.data_path(id).exists()
    }

    pub fn get_bytes(&self, id: &BlobId) -> std::io::Result<Option<Vec<u8>>> {
        let path = self.data_path(id);
        if !path.exists() {
            return Ok(None);
        }
        Ok(Some(fs::read(path)?))
    }

    /// Apply PutBlob / RenewMediaBlobPin from a committed block.
    pub fn ingest_block_txs(&self, height: u64, txs: &[Transaction]) -> std::io::Result<usize> {
        let mut n = 0;
        for tx in txs {
            match &tx.payload {
                TransactionPayload::PutBlob {
                    retention,
                    content_type,
                    data,
                } => {
                    let blob_id = blob_id_of(data);
                    let meta = BlobMetaRecord {
                        blob_id,
                        size: data.len() as u32,
                        content_type: content_type.clone(),
                        retention: *retention,
                        publisher: tx.sender.0,
                        included_height: height,
                        tx_id: tx.id().0,
                        pin_until_height: None,
                    };
                    self.put(meta, data)?;
                    n += 1;
                }
                TransactionPayload::RenewMediaBlobPin {
                    blob_id,
                    size,
                    extend_blocks,
                } => {
                    if self.renew_pin(blob_id, *size, height, *extend_blocks, tx.id().0)? {
                        n += 1;
                    }
                }
                TransactionPayload::RegisterBlobSampleOffer {
                    blob_id,
                    chunk_index,
                    chunk_count,
                    ask_fee,
                    expires_at_height,
                } => {
                    let offer = MediaBlobSampleOffer {
                        blob_id: *blob_id,
                        chunk_index: *chunk_index,
                        chunk_count: *chunk_count,
                        provider: AccountId(tx.sender.0),
                        ask_fee: *ask_fee,
                        expires_at_height: *expires_at_height,
                    };
                    // Phase 8: Register lands only in the consensus-replay book.
                    self.upsert_onchain_sample_offer(offer)
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                    n += 1;
                }
                TransactionPayload::SettleBlobSampleMatch {
                    blob_id,
                    chunk_index,
                    provider,
                    ..
                } => {
                    let mut m = self
                        .onchain_sample_market
                        .write()
                        .unwrap_or_else(|e| e.into_inner());
                    if m.remove_on_settle(blob_id, *chunk_index, provider) {
                        self.persist_onchain_sample_market(&m)?;
                        n += 1;
                    }
                }
                TransactionPayload::GrantBlobDownload { blob_id, grantee } => {
                    match self.grant_download(blob_id, &AccountId(tx.sender.0), *grantee) {
                        Ok(true) => n += 1,
                        Ok(false) => {}
                        Err(e) => {
                            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e));
                        }
                    }
                }
                TransactionPayload::RevokeBlobDownload { blob_id, grantee } => {
                    match self.revoke_download(blob_id, &AccountId(tx.sender.0), grantee) {
                        Ok(true) => n += 1,
                        Ok(false) => {}
                        Err(e) => {
                            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e));
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(n)
    }

    /// Extend `pin_until_height` from `max(current_expiry_or_height, height) + extend_blocks`.
    pub fn renew_pin(
        &self,
        blob_id: &BlobId,
        size: u32,
        height: u64,
        extend_blocks: u64,
        renew_tx_id: [u8; 32],
    ) -> std::io::Result<bool> {
        let key = blob_id_hex(blob_id);
        let mut idx = self.index.write().unwrap_or_else(|e| e.into_inner());
        let Some(meta) = idx.entries.get_mut(&key) else {
            return Ok(false);
        };
        if meta.size != size {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "renew size mismatch",
            ));
        }
        let base = meta.expiry_height().unwrap_or(height).max(height);
        meta.pin_until_height = Some(base.saturating_add(extend_blocks));
        meta.tx_id = renew_tx_id;
        self.persist_index(&idx)?;
        Ok(true)
    }

    /// Drop pruneable blobs past TTL relative to `tip_height`. Forever blobs are never removed here.
    pub fn prune_expired(&self, tip_height: u64) -> std::io::Result<usize> {
        let mut idx = self.index.write().unwrap_or_else(|e| e.into_inner());
        let expired: Vec<String> = idx
            .entries
            .iter()
            .filter(|(_, m)| m.is_expired(tip_height))
            .map(|(k, _)| k.clone())
            .collect();
        let mut removed = 0;
        let mut access = self.access.write().unwrap_or_else(|e| e.into_inner());
        let mut access_changed = false;
        for key in expired {
            if let Some(meta) = idx.entries.remove(&key) {
                let path = self.data_path(&meta.blob_id);
                let _ = fs::remove_file(path);
                if access.entries.remove(&key).is_some() {
                    access_changed = true;
                }
                removed += 1;
            }
        }
        if removed > 0 {
            self.persist_index(&idx)?;
        }
        if access_changed {
            self.persist_access(&access)?;
        }
        Ok(removed)
    }

    pub fn len(&self) -> usize {
        self.index
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .entries
            .len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use boing_primitives::{AccessList, AccountId, Transaction};

    fn tmp_dir(name: &str) -> PathBuf {
        let p =
            std::env::temp_dir().join(format!("boing-media-blob-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn grant_revoke_download_acl() {
        let dir = tmp_dir("acl");
        let store = MediaBlobStore::open(&dir).unwrap();
        let data = b"acl-bytes";
        let id = blob_id_of(data);
        let publisher = AccountId([1u8; 32]);
        let grantee = AccountId([2u8; 32]);
        let meta = BlobMetaRecord {
            blob_id: id,
            size: data.len() as u32,
            content_type: "text/plain".into(),
            retention: boing_primitives::BlobRetention::Forever,
            publisher: publisher.0,
            included_height: 1,
            tx_id: [0u8; 32],
            pin_until_height: None,
        };
        store.put(meta, data).unwrap();
        assert!(store.may_download(&id, &publisher));
        assert!(!store.may_download(&id, &grantee));
        assert_eq!(
            store.grant_download(&id, &publisher, grantee).unwrap(),
            true
        );
        assert!(store.may_download(&id, &grantee));
        assert_eq!(store.get_access(&id).epoch, 0);
        assert!(store.revoke_download(&id, &publisher, &grantee).unwrap());
        assert!(!store.may_download(&id, &grantee));
        assert_eq!(store.get_access(&id).epoch, 1);
    }

    #[test]
    fn put_get_roundtrip() {
        let dir = tmp_dir("roundtrip");
        let store = MediaBlobStore::open(&dir).unwrap();
        let data = b"png-bytes-here".to_vec();
        let id = blob_id_of(&data);
        let meta = BlobMetaRecord {
            blob_id: id,
            size: data.len() as u32,
            content_type: "image/png".into(),
            retention: BlobRetention::Prunable { ttl_blocks: 10 },
            publisher: [1u8; 32],
            included_height: 5,
            tx_id: [2u8; 32],
            pin_until_height: None,
        };
        store.put(meta, &data).unwrap();
        assert_eq!(store.get_meta(&id).unwrap().content_type, "image/png");
        assert_eq!(store.get_bytes(&id).unwrap().unwrap(), data);
        assert!(store.prune_expired(20).unwrap() >= 1);
        assert!(store.get_meta(&id).is_none());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn ingest_put_blob_tx() {
        let dir = tmp_dir("ingest");
        let store = MediaBlobStore::open(&dir).unwrap();
        let sender = AccountId([7u8; 32]);
        let data = b"gif-frame".to_vec();
        let tx = Transaction {
            nonce: 0,
            sender,
            payload: TransactionPayload::PutBlob {
                retention: BlobRetention::Forever,
                content_type: "image/gif".into(),
                data: data.clone(),
            },
            access_list: AccessList::new(vec![sender], vec![sender]),
        };
        store.ingest_block_txs(3, &[tx]).unwrap();
        let id = blob_id_of(&data);
        assert_eq!(store.get_bytes(&id).unwrap().unwrap(), data);
        assert!(!store.get_meta(&id).unwrap().is_expired(1_000_000));
        let _ = fs::remove_dir_all(dir);
    }
}
