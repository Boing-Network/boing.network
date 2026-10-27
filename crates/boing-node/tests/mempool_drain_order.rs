//! Drain ordering with priority tips (own binary: sets the process-wide signature policy).

use boing_node::mempool::Mempool;
use boing_primitives::{
    set_tx_verify_policy, AccessList, AccountId, SignedTransaction, Transaction,
    TransactionPayload, TxSigPolicy,
};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

const CHAIN: u64 = 6913;

fn tipped(key: &SigningKey, nonce: u64, tip: u128) -> SignedTransaction {
    let sender = AccountId(key.verifying_key().to_bytes());
    let tx = Transaction {
        nonce,
        sender,
        payload: TransactionPayload::Transfer {
            to: AccountId([7u8; 32]),
            amount: 1,
        },
        access_list: AccessList::default(),
    };
    SignedTransaction::new_for_chain_with_tip(tx, key, CHAIN, tip)
}

#[test]
fn drain_keeps_per_sender_nonce_order_even_with_a_higher_later_tip() {
    set_tx_verify_policy(TxSigPolicy {
        chain_id: Some(CHAIN),
        require_v2: false,
    });
    let pool = Mempool::new();
    let a = SigningKey::generate(&mut OsRng);
    let b = SigningKey::generate(&mut OsRng);
    // a: nonce 0 tip 0, nonce 1 tip 100 (must still come after nonce 0); b: nonce 0 tip 50.
    pool.insert(tipped(&a, 0, 0)).unwrap();
    pool.insert(tipped(&a, 1, 100)).unwrap();
    pool.insert(tipped(&b, 0, 50)).unwrap();
    let out = pool.drain_for_block(10);
    assert_eq!(out.len(), 3);
    let a_id = AccountId(a.verifying_key().to_bytes());
    let a_nonces: Vec<u64> = out
        .iter()
        .filter(|s| s.tx.sender == a_id)
        .map(|s| s.tx.nonce)
        .collect();
    assert_eq!(a_nonces, vec![0, 1]);
    // b (tip 50) outranks a's head (tip 0), so it goes first; then a:0, then a:1.
    assert_eq!(out[0].priority_tip, 50);
    assert_eq!(out[1].priority_tip, 0);
    assert_eq!(out[2].priority_tip, 100);
    assert!(pool.is_empty());
}

#[test]
fn higher_tip_senders_go_first_across_senders() {
    set_tx_verify_policy(TxSigPolicy {
        chain_id: Some(CHAIN),
        require_v2: false,
    });
    let pool = Mempool::new();
    let keys: Vec<SigningKey> = (0..4).map(|_| SigningKey::generate(&mut OsRng)).collect();
    for (i, k) in keys.iter().enumerate() {
        pool.insert(tipped(k, 0, (i as u128 + 1) * 10)).unwrap();
    }
    let out = pool.drain_for_block(3);
    let tips: Vec<u128> = out.iter().map(|s| s.priority_tip).collect();
    assert_eq!(tips, vec![40, 30, 20]);
    assert_eq!(pool.len(), 1);
}
