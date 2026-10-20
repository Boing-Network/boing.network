//! Node sidecar for L1 media blob bytes (`data/blobs/`).
//!
//! Consensus PutBlob txs carry the payload in the block; this module persists bytes + metadata for
//! `boing_getBlob` / `boing_getBlobMeta`. Not part of the account SMT.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use boing_primitives::{
    blob_id_hex, blob_id_of, BlobId, BlobRetention, Transaction, TransactionPayload,
};
use serde::{Deserialize, Serialize};

const BLOBS_DIR: &str = "blobs";
const INDEX_FILE: &str = "index.bin";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlobMetaRecord {
    pub blob_id: BlobId,
    pub size: u32,
    pub content_type: String,
    pub retention: BlobRetention,
    pub publisher: [u8; 32],
    pub included_height: u64,
    pub tx_id: [u8; 32],
}

impl BlobMetaRecord {
    pub fn expiry_height(&self) -> Option<u64> {
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

/// Disk-backed media blob sidecar under `{data_dir}/blobs/`.
pub struct MediaBlobStore {
    dir: PathBuf,
    index: RwLock<BlobIndex>,
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
        Ok(Self {
            dir,
            index: RwLock::new(index),
        })
    }

    fn index_path(&self) -> PathBuf {
        self.dir.join(INDEX_FILE)
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

    pub fn get_bytes(&self, id: &BlobId) -> std::io::Result<Option<Vec<u8>>> {
        let path = self.data_path(id);
        if !path.exists() {
            return Ok(None);
        }
        Ok(Some(fs::read(path)?))
    }

    /// Apply PutBlob txs from a committed block (payload bytes are consensus-visible in the block).
    pub fn ingest_block_txs(&self, height: u64, txs: &[Transaction]) -> std::io::Result<usize> {
        let mut n = 0;
        for tx in txs {
            let TransactionPayload::PutBlob {
                retention,
                content_type,
                data,
            } = &tx.payload
            else {
                continue;
            };
            let blob_id = blob_id_of(data);
            let meta = BlobMetaRecord {
                blob_id,
                size: data.len() as u32,
                content_type: content_type.clone(),
                retention: *retention,
                publisher: tx.sender.0,
                included_height: height,
                tx_id: tx.id().0,
            };
            self.put(meta, data)?;
            n += 1;
        }
        Ok(n)
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
        for key in expired {
            if let Some(meta) = idx.entries.remove(&key) {
                let path = self.data_path(&meta.blob_id);
                let _ = fs::remove_file(path);
                removed += 1;
            }
        }
        if removed > 0 {
            self.persist_index(&idx)?;
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
        let p = std::env::temp_dir().join(format!("boing-media-blob-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
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
