//! Test execution verification.

use boing_automation::ExecutorAttestation;
use boing_primitives::{hasher, AccountId, Hash};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

#[test]
fn test_executor_attestation_sign_verify() {
    let key = SigningKey::generate(&mut OsRng);
    let executor = AccountId(key.verifying_key().to_bytes());
    let mut h = hasher();
    h.update(b"result");
    let result_hash = Hash(*h.finalize().as_bytes());

    let att = ExecutorAttestation::new(executor, 1, result_hash, 1000, &key);
    assert!(att.verify().is_ok());
}
