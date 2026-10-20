//! Hashing primitives for Boing.
//! Uses BLAKE3 for performance and security.

use blake3::Hasher as Blake3Hasher;
use serde::{Deserialize, Serialize};

/// 32-byte hash output.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(transparent)]
pub struct Hash(pub [u8; 32]);

impl Default for Hash {
    fn default() -> Self {
        Hash::ZERO
    }
}

impl Hash {
    pub const ZERO: Hash = Hash([0u8; 32]);

    pub fn from_slice(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != 32 {
            return None;
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(bytes);
        Some(Hash(arr))
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Hash({})", hex::encode(&self.0[..8]))
    }
}

impl std::fmt::Display for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

/// Hasher for Boing — BLAKE3.
pub fn hasher() -> Blake3Hasher {
    Blake3Hasher::new()
}
