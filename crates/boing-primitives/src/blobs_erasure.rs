//! Reed–Solomon erasure coding for media blob chunks (Phase 5).
//!
//! Upgrades the Phase 4 XOR (single-parity) path: with [`MEDIA_BLOB_RS_PARITY_SHARDS`] parity
//! shards, any [`data_shards`] of the `data + parity` set can reconstruct the payload.
//! Uses the `reed-solomon-erasure` GF(2⁸) codec. Not a paid sampling market by itself.

use reed_solomon_erasure::galois_8::ReedSolomon;
use serde::{Deserialize, Serialize};

use crate::blobs::{
    blob_id_of, media_blob_chunks_merkle_root, media_blob_das_chunk_count, BlobError, BlobId,
    MediaBlobChunkRef, MEDIA_BLOB_DAS_CHUNK_BYTES, MAX_MEDIA_BLOB_BYTES,
};
use crate::Hash;

/// Default parity shards for the Phase 5 RS path (recover up to this many missing data shards).
pub const MEDIA_BLOB_RS_PARITY_SHARDS: usize = 2;

/// Minimum data shards when padding a tiny payload for RS (codec requires ≥1 data shard).
pub const MEDIA_BLOB_RS_MIN_DATA_SHARDS: usize = 1;

/// Max data shards (= max DAS chunks for live 16 MiB / 64 KiB).
pub const MEDIA_BLOB_RS_MAX_DATA_SHARDS: usize = 256;

/// Reed–Solomon coded bundle over DAS-sized shards.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaBlobRsBundle {
    pub blob_id: BlobId,
    pub original_len: u32,
    pub data_shard_count: u32,
    pub parity_shard_count: u32,
    /// Length of each shard (padded to [`MEDIA_BLOB_DAS_CHUNK_BYTES`] except when empty).
    pub shard_len: u32,
    pub data_shards: Vec<Vec<u8>>,
    pub parity_shards: Vec<Vec<u8>>,
    pub chunk_hashes: Vec<[u8; 32]>,
    pub chunks_root: Hash,
}

fn shard_hash(bytes: &[u8]) -> [u8; 32] {
    let mut h = crate::hasher();
    h.update(bytes);
    *h.finalize().as_bytes()
}

fn make_codec(data: usize, parity: usize) -> Result<ReedSolomon, BlobError> {
    if data == 0 || parity == 0 {
        return Err(BlobError::RsInvalidParams { data, parity });
    }
    if data > MEDIA_BLOB_RS_MAX_DATA_SHARDS {
        return Err(BlobError::RsInvalidParams { data, parity });
    }
    ReedSolomon::new(data, parity).map_err(|_| BlobError::RsInvalidParams { data, parity })
}

/// Split `data` into DAS chunks and add [`MEDIA_BLOB_RS_PARITY_SHARDS`] Reed–Solomon parity shards.
pub fn chunk_media_blob_rs(data: &[u8]) -> Result<MediaBlobRsBundle, BlobError> {
    chunk_media_blob_rs_with_parity(data, MEDIA_BLOB_RS_PARITY_SHARDS)
}

/// Like [`chunk_media_blob_rs`] with an explicit parity count (tests / experiments).
pub fn chunk_media_blob_rs_with_parity(
    data: &[u8],
    parity_count: usize,
) -> Result<MediaBlobRsBundle, BlobError> {
    if data.len() > MAX_MEDIA_BLOB_BYTES {
        return Err(BlobError::TooLarge { size: data.len() });
    }
    let blob_id = blob_id_of(data);
    if data.is_empty() {
        return Ok(MediaBlobRsBundle {
            blob_id,
            original_len: 0,
            data_shard_count: 0,
            parity_shard_count: 0,
            shard_len: 0,
            data_shards: vec![],
            parity_shards: vec![],
            chunk_hashes: vec![],
            chunks_root: Hash::ZERO,
        });
    }
    let mut data_shards: Vec<Vec<u8>> = data
        .chunks(MEDIA_BLOB_DAS_CHUNK_BYTES)
        .map(|c| {
            let mut v = c.to_vec();
            if v.len() < MEDIA_BLOB_DAS_CHUNK_BYTES {
                v.resize(MEDIA_BLOB_DAS_CHUNK_BYTES, 0);
            }
            v
        })
        .collect();
    let data_count = data_shards.len().max(MEDIA_BLOB_RS_MIN_DATA_SHARDS);
    while data_shards.len() < data_count {
        data_shards.push(vec![0u8; MEDIA_BLOB_DAS_CHUNK_BYTES]);
    }
    let codec = make_codec(data_count, parity_count)?;
    let mut shards: Vec<Vec<u8>> = data_shards;
    for _ in 0..parity_count {
        shards.push(vec![0u8; MEDIA_BLOB_DAS_CHUNK_BYTES]);
    }
    codec
        .encode(&mut shards)
        .map_err(|_| BlobError::RsEncodeFailed)?;
    let parity_shards = shards.split_off(data_count);
    let data_shards = shards;
    let chunk_hashes: Vec<[u8; 32]> = data_shards.iter().map(|s| shard_hash(s)).collect();
    let chunks_root = media_blob_chunks_merkle_root(&chunk_hashes);
    Ok(MediaBlobRsBundle {
        blob_id,
        original_len: data.len() as u32,
        data_shard_count: data_count as u32,
        parity_shard_count: parity_count as u32,
        shard_len: MEDIA_BLOB_DAS_CHUNK_BYTES as u32,
        data_shards,
        parity_shards,
        chunk_hashes,
        chunks_root,
    })
}

/// Reconstruct original bytes from a mix of present data/parity shards (≤ `parity_count` erasures).
///
/// `shards` length must be `data_count + parity_count`; missing slots are `None`.
pub fn recover_media_blob_rs(
    blob_id: &BlobId,
    original_len: usize,
    data_count: usize,
    parity_count: usize,
    shards: &[Option<Vec<u8>>],
) -> Result<Vec<u8>, BlobError> {
    if original_len == 0 {
        let empty = Vec::new();
        if blob_id_of(&empty) != *blob_id {
            return Err(BlobError::RecoveryIdMismatch);
        }
        return Ok(empty);
    }
    if shards.len() != data_count + parity_count {
        return Err(BlobError::RsShardCountMismatch {
            expected: data_count + parity_count,
            got: shards.len(),
        });
    }
    let present = shards.iter().filter(|s| s.is_some()).count();
    if present < data_count {
        return Err(BlobError::RecoveryInsufficient);
    }
    let codec = make_codec(data_count, parity_count)?;
    let mut working: Vec<Option<Vec<u8>>> = shards.to_vec();
    codec
        .reconstruct(&mut working)
        .map_err(|_| BlobError::RsReconstructFailed)?;
    let mut out = Vec::with_capacity(original_len);
    for i in 0..data_count {
        let shard = working[i]
            .as_ref()
            .ok_or(BlobError::RsReconstructFailed)?;
        out.extend_from_slice(shard);
    }
    out.truncate(original_len);
    if blob_id_of(&out) != *blob_id {
        return Err(BlobError::RecoveryIdMismatch);
    }
    Ok(out)
}

/// Convenience: chunk refs for data shards of an RS bundle.
pub fn media_blob_rs_chunk_refs(bundle: &MediaBlobRsBundle) -> Vec<MediaBlobChunkRef> {
    bundle
        .chunk_hashes
        .iter()
        .enumerate()
        .map(|(i, h)| MediaBlobChunkRef {
            blob_id: bundle.blob_id,
            chunk_index: i as u32,
            chunk_count: bundle.data_shard_count,
            chunk_hash: *h,
        })
        .collect()
}

/// Suggested data-shard count for a payload size (DAS ceil).
#[must_use]
pub fn media_blob_rs_data_shards_for_size(size: usize) -> usize {
    media_blob_das_chunk_count(size).max(if size == 0 {
        0
    } else {
        MEDIA_BLOB_RS_MIN_DATA_SHARDS
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blobs::MEDIA_BLOB_DAS_CHUNK_BYTES;

    #[test]
    fn rs_recovers_two_missing_data_shards() {
        let data: Vec<u8> = (0..(MEDIA_BLOB_DAS_CHUNK_BYTES * 3 + 17))
            .map(|i| (i % 251) as u8)
            .collect();
        let bundle = chunk_media_blob_rs(&data).unwrap();
        assert_eq!(bundle.data_shard_count, 4);
        assert_eq!(bundle.parity_shard_count as usize, MEDIA_BLOB_RS_PARITY_SHARDS);

        let mut shards: Vec<Option<Vec<u8>>> = bundle
            .data_shards
            .iter()
            .cloned()
            .map(Some)
            .chain(bundle.parity_shards.iter().cloned().map(Some))
            .collect();
        shards[0] = None;
        shards[2] = None;
        let recovered = recover_media_blob_rs(
            &bundle.blob_id,
            data.len(),
            bundle.data_shard_count as usize,
            bundle.parity_shard_count as usize,
            &shards,
        )
        .unwrap();
        assert_eq!(recovered, data);
    }

    #[test]
    fn rs_fails_when_too_many_erasures() {
        let data = vec![1u8; MEDIA_BLOB_DAS_CHUNK_BYTES + 1];
        let bundle = chunk_media_blob_rs(&data).unwrap();
        let mut shards: Vec<Option<Vec<u8>>> = bundle
            .data_shards
            .iter()
            .cloned()
            .map(Some)
            .chain(bundle.parity_shards.iter().cloned().map(Some))
            .collect();
        // erase 3 > parity 2
        shards[0] = None;
        shards[1] = None;
        shards[2] = None;
        assert!(recover_media_blob_rs(
            &bundle.blob_id,
            data.len(),
            bundle.data_shard_count as usize,
            bundle.parity_shard_count as usize,
            &shards,
        )
        .is_err());
    }
}
