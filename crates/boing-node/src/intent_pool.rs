//! Intent pool — stores signed intents for solver/executor fulfillment.
//! Stub implementation; production would persist and integrate with solver network.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};

use boing_primitives::{Hash, SignedIntent};

/// Default cap so `boing_submitIntent` cannot grow the process forever (audit #61 item 13).
pub const DEFAULT_MAX_INTENTS: usize = 10_000;

/// In-memory intent pool.
#[derive(Clone, Debug)]
pub struct IntentPool {
    inner: Arc<RwLock<IntentPoolInner>>,
    max_intents: usize,
}

#[derive(Debug, Default)]
struct IntentPoolInner {
    intents: HashMap<[u8; 32], SignedIntent>,
    /// Insertion order for FIFO eviction when at capacity.
    order: VecDeque<[u8; 32]>,
}

impl Default for IntentPool {
    fn default() -> Self {
        Self::new()
    }
}

impl IntentPool {
    pub fn new() -> Self {
        Self::with_max_intents(DEFAULT_MAX_INTENTS)
    }

    pub fn with_max_intents(max_intents: usize) -> Self {
        Self {
            inner: Arc::new(RwLock::new(IntentPoolInner::default())),
            max_intents: max_intents.max(1),
        }
    }

    /// Submit a signed intent. Returns intent ID (hash) if accepted.
    pub fn submit(&self, signed: SignedIntent) -> Result<Hash, IntentPoolError> {
        if signed.verify().is_err() {
            return Err(IntentPoolError::InvalidSignature);
        }
        let id = signed.intent.id();
        let mut inner = self.inner.write().unwrap();
        if inner.intents.contains_key(&id.0) {
            inner.intents.insert(id.0, signed);
            return Ok(id);
        }
        while inner.intents.len() >= self.max_intents {
            let Some(old) = inner.order.pop_front() else {
                break;
            };
            inner.intents.remove(&old);
        }
        inner.order.push_back(id.0);
        inner.intents.insert(id.0, signed);
        Ok(id)
    }

    pub fn len(&self) -> usize {
        self.inner.read().unwrap().intents.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn max_intents(&self) -> usize {
        self.max_intents
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IntentPoolError {
    #[error("Invalid intent signature")]
    InvalidSignature,
}

#[cfg(test)]
mod tests {
    use super::*;
    use boing_primitives::{hasher, AccountId, Intent, IntentKind};
    use ed25519_dalek::{Signer, SigningKey};
    use rand::rngs::OsRng;

    fn signed_intent(n: u8) -> SignedIntent {
        let key = SigningKey::generate(&mut OsRng);
        let intent = Intent {
            signer: AccountId(key.verifying_key().to_bytes()),
            kind: IntentKind::Custom(vec![n]),
            nonce: n as u64,
            deadline_block: 100,
        };
        let msg = bincode::serialize(&intent).unwrap();
        let mut h = hasher();
        h.update(&msg);
        let digest = h.finalize();
        let sig = key.sign(digest.as_bytes());
        SignedIntent {
            intent,
            signature: boing_primitives::signature::Signature(sig.to_bytes()),
        }
    }

    #[test]
    fn caps_and_evicts_oldest() {
        let pool = IntentPool::with_max_intents(2);
        let a = signed_intent(1);
        let b = signed_intent(2);
        let c = signed_intent(3);
        let id_a = a.intent.id();
        pool.submit(a).unwrap();
        pool.submit(b).unwrap();
        assert_eq!(pool.len(), 2);
        pool.submit(c).unwrap();
        assert_eq!(pool.len(), 2);
        assert!(!pool.inner.read().unwrap().intents.contains_key(&id_a.0));
    }
}
