//! Bounded LRU cache of chunked media-blob bundles for the DAS RPCs.
//!
//! `boing_getBlobDasSample` / `boing_getBlobDasLightClientBatch` used to re-read the whole blob from
//! disk and re-chunk + re-hash it (up to 16 MiB) on every call. Blobs are content-addressed (the id is
//! the hash of the bytes), so a bundle for a given id can never go stale; the RPC still checks that the
//! blob file exists, so expired/pruned blobs keep returning `null`. The cache is bounded by total bytes.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};

use boing_primitives::{BlobId, MediaBlobErasureBundle};

/// Default byte budget (counted as data chunks + parity + hashes).
pub const DEFAULT_DAS_CACHE_MAX_BYTES: usize = 64 * 1024 * 1024;
/// Environment variable overriding the budget (`0` disables the cache).
pub const ENV_DAS_CACHE_MAX_BYTES: &str = "BOING_DAS_CACHE_MAX_BYTES";

fn bundle_bytes(b: &MediaBlobErasureBundle) -> usize {
    b.data_chunks.iter().map(|c| c.len()).sum::<usize>() + b.parity_chunk.len() + b.chunk_hashes.len() * 32 + 128
}

/// Byte-bounded LRU of `Arc<MediaBlobErasureBundle>` keyed by blob id.
pub struct DasBundleCache {
    max_bytes: usize,
    used: usize,
    map: HashMap<BlobId, (Arc<MediaBlobErasureBundle>, usize)>,
    /// Front = least recently used.
    order: VecDeque<BlobId>,
    hits: u64,
    misses: u64,
}

impl DasBundleCache {
    pub fn new(max_bytes: usize) -> Self {
        Self { max_bytes, used: 0, map: HashMap::new(), order: VecDeque::new(), hits: 0, misses: 0 }
    }

    fn touch(&mut self, id: &BlobId) {
        if let Some(pos) = self.order.iter().position(|x| x == id) {
            self.order.remove(pos);
        }
        self.order.push_back(*id);
    }

    pub fn get(&mut self, id: &BlobId) -> Option<Arc<MediaBlobErasureBundle>> {
        if let Some((b, _)) = self.map.get(id) {
            let b = b.clone();
            self.hits += 1;
            self.touch(id);
            Some(b)
        } else {
            self.misses += 1;
            None
        }
    }

    /// Insert (skipped when the cache is disabled or the bundle alone exceeds the budget).
    pub fn insert(&mut self, id: BlobId, bundle: Arc<MediaBlobErasureBundle>) {
        let size = bundle_bytes(&bundle);
        if self.max_bytes == 0 || size > self.max_bytes {
            return;
        }
        if let Some((_, old)) = self.map.remove(&id) {
            self.used -= old;
            if let Some(pos) = self.order.iter().position(|x| *x == id) {
                self.order.remove(pos);
            }
        }
        while self.used + size > self.max_bytes {
            let Some(victim) = self.order.pop_front() else { break };
            if let Some((_, s)) = self.map.remove(&victim) {
                self.used -= s;
            }
        }
        self.used += size;
        self.map.insert(id, (bundle, size));
        self.order.push_back(id);
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn used_bytes(&self) -> usize {
        self.used
    }

    /// `(hits, misses)` since creation.
    pub fn stats(&self) -> (u64, u64) {
        (self.hits, self.misses)
    }
}

fn global() -> &'static Mutex<DasBundleCache> {
    static C: OnceLock<Mutex<DasBundleCache>> = OnceLock::new();
    C.get_or_init(|| {
        let max = std::env::var(ENV_DAS_CACHE_MAX_BYTES)
            .ok()
            .and_then(|s| s.trim().parse::<usize>().ok())
            .unwrap_or(DEFAULT_DAS_CACHE_MAX_BYTES);
        Mutex::new(DasBundleCache::new(max))
    })
}

/// Return the cached bundle for `id`, or build it with `build` (disk read + chunking) and cache it.
/// `build` runs outside the lock so slow reads never block other RPC threads.
pub fn get_or_build<E>(
    id: &BlobId,
    build: impl FnOnce() -> Result<Option<MediaBlobErasureBundle>, E>,
) -> Result<Option<Arc<MediaBlobErasureBundle>>, E> {
    if let Some(b) = global().lock().ok().and_then(|mut c| c.get(id)) {
        return Ok(Some(b));
    }
    let Some(bundle) = build()? else { return Ok(None) };
    let arc = Arc::new(bundle);
    if let Ok(mut c) = global().lock() {
        c.insert(*id, arc.clone());
    }
    Ok(Some(arc))
}

/// Global cache stats `(hits, misses, entries, used_bytes)` (diagnostics / tests).
pub fn global_stats() -> (u64, u64, usize, usize) {
    global()
        .lock()
        .map(|c| {
            let (h, m) = c.stats();
            (h, m, c.len(), c.used_bytes())
        })
        .unwrap_or((0, 0, 0, 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use boing_primitives::{chunk_media_blob, MAX_MEDIA_BLOB_BYTES};

    fn bundle(n: usize, seed: u8) -> Arc<MediaBlobErasureBundle> {
        let data: Vec<u8> = (0..n).map(|i| (i as u8).wrapping_add(seed)).collect();
        Arc::new(chunk_media_blob(&data).unwrap())
    }

    #[test]
    fn hit_miss_and_lru_eviction() {
        let a = bundle(10_000, 1);
        let b = bundle(10_000, 2);
        let c = bundle(10_000, 3);
        let each = bundle_bytes(&a);
        let mut cache = DasBundleCache::new(each * 2 + each / 2);
        assert!(cache.get(&a.blob_id).is_none());
        cache.insert(a.blob_id, a.clone());
        cache.insert(b.blob_id, b.clone());
        assert!(cache.get(&a.blob_id).is_some()); // a is now MRU
        cache.insert(c.blob_id, c.clone()); // evicts b (LRU)
        assert!(cache.get(&b.blob_id).is_none());
        assert!(cache.get(&a.blob_id).is_some());
        assert!(cache.get(&c.blob_id).is_some());
        assert!(cache.used_bytes() <= each * 2 + each / 2);
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn disabled_and_oversize_are_not_cached() {
        let a = bundle(5_000, 9);
        let mut off = DasBundleCache::new(0);
        off.insert(a.blob_id, a.clone());
        assert!(off.is_empty());
        let mut tiny = DasBundleCache::new(16);
        tiny.insert(a.blob_id, a);
        assert!(tiny.is_empty());
        let _ = MAX_MEDIA_BLOB_BYTES;
    }

    #[test]
    fn get_or_build_builds_once() {
        let a = bundle(20_000, 77);
        let id = a.blob_id;
        let mut builds = 0;
        for _ in 0..3 {
            let got = get_or_build::<()>(&id, || {
                builds += 1;
                Ok(Some((*a).clone()))
            })
            .unwrap()
            .unwrap();
            assert_eq!(got.chunks_root, a.chunks_root);
        }
        assert_eq!(builds, 1);
        let none = get_or_build::<()>(&[0xEE; 32], || Ok(None)).unwrap();
        assert!(none.is_none());
    }
}
