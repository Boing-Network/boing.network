//! Low base (micro) fees + optional priority tips: tip ordering, micro charge, legacy waive path.

use boing_execution::BlockExecutor;
use boing_node::mempool::{Mempool, MempoolError, DEFAULT_MAX_FREE_PENDING_PER_SENDER};
use boing_primitives::{
    set_tx_verify_policy, AccessList, Account, AccountId, AccountState, SignedTransaction,
    Transaction, TransactionPayload, TxSigPolicy,
};
use boing_state::StateStore;
use boing_tokenomics::{NetworkFeePolicy, GAS_PRICE_BASE_UNITS, MICRO_FEE_EXECUTION_GAS_BPS};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

fn key() -> SigningKey {
    SigningKey::generate(&mut OsRng)
}

fn id(k: &SigningKey) -> AccountId {
    AccountId(k.verifying_key().to_bytes())
}

fn transfer(sender: AccountId, nonce: u64, to: AccountId, amount: u128) -> Transaction {
    Transaction {
        nonce,
        sender,
        payload: TransactionPayload::Transfer { to, amount },
        access_list: AccessList::default(),
    }
}

#[test]
fn micro_fee_charges_small_base_not_zero() {
    let sk = key();
    let sender = id(&sk);
    let to = AccountId([2u8; 32]);
    let proposer = AccountId([0xfe; 32]);
    let mut state = StateStore::new();
    state.insert(Account {
        id: sender,
        state: AccountState {
            balance: 1_000 * GAS_PRICE_BASE_UNITS,
            nonce: 0,
            ..Default::default()
        },
    });
    state.insert(Account {
        id: to,
        state: AccountState {
            balance: 0,
            nonce: 0,
            ..Default::default()
        },
    });

    let mut exec = BlockExecutor::new();
    exec.set_fee_policy(NetworkFeePolicy::micro_fee_default());
    let tx = transfer(sender, 0, to, 10);
    let before = state.get(&sender).unwrap().balance;
    let (gas, receipts) = exec
        .execute_block(1, 0, &[tx.clone()], &mut state, proposer)
        .unwrap();
    assert!(receipts[0].success);
    assert_eq!(gas, 21_000);
    let expected_fee =
        21_000 * GAS_PRICE_BASE_UNITS * (MICRO_FEE_EXECUTION_GAS_BPS as u128) / 10_000;
    assert_eq!(expected_fee, 21_000 * GAS_PRICE_BASE_UNITS / 100);
    assert_eq!(
        state.get(&sender).unwrap().balance,
        before - 10 - expected_fee
    );
    assert_eq!(state.get(&to).unwrap().balance, 10);
}

#[test]
fn tip_credits_proposer_and_orders_mempool() {
    let sk_a = key();
    let sk_b = key();
    let a = id(&sk_a);
    let b = id(&sk_b);
    let to = AccountId([9u8; 32]);
    let chain_id = 6913u64;
    set_tx_verify_policy(TxSigPolicy {
        chain_id: Some(chain_id),
        require_v2: true,
    });

    let pool = Mempool::new();
    // Tip ordering works without enabling the legacy free-gas path.
    assert!(!pool.optional_fees_active());

    let tx_free =
        SignedTransaction::new_for_chain_with_tip(transfer(a, 0, to, 1), &sk_a, chain_id, 0);
    let tx_tip =
        SignedTransaction::new_for_chain_with_tip(transfer(b, 0, to, 1), &sk_b, chain_id, 50);
    pool.insert(tx_free).unwrap();
    pool.insert(tx_tip).unwrap();
    let drained = pool.drain_for_block(2);
    assert_eq!(drained.len(), 2);
    assert_eq!(drained[0].priority_tip, 50, "higher tip first");
    assert_eq!(drained[1].priority_tip, 0);

    let proposer = AccountId([0xfe; 32]);
    let mut state = StateStore::new();
    let funded = 1_000 * GAS_PRICE_BASE_UNITS;
    for acct in [a, b] {
        state.insert(Account {
            id: acct,
            state: AccountState {
                balance: funded,
                nonce: 0,
                ..Default::default()
            },
        });
    }
    state.insert(Account {
        id: to,
        state: AccountState {
            balance: 0,
            nonce: 0,
            ..Default::default()
        },
    });
    let mut exec = BlockExecutor::new();
    exec.set_fee_policy(NetworkFeePolicy::micro_fee_default());
    let tip_tx = transfer(b, 0, to, 1);
    let before_b = state.get(&b).unwrap().balance;
    let micro = 21_000 * GAS_PRICE_BASE_UNITS / 100;
    let (_g, r) = exec
        .execute_block_with_tips(1, 0, &[tip_tx], &mut state, proposer, &[50])
        .unwrap();
    assert!(r[0].success);
    assert_eq!(state.get(&b).unwrap().balance, before_b - 1 - micro - 50);
    assert_eq!(state.get(&proposer).unwrap().balance, 50);
}

#[test]
fn legacy_free_pending_cap_only_when_gas_waived() {
    let sk = key();
    let sender = id(&sk);
    let to = AccountId([2u8; 32]);
    let chain_id = 6913u64;
    set_tx_verify_policy(TxSigPolicy {
        chain_id: Some(chain_id),
        require_v2: true,
    });
    let pool = Mempool::new().with_max_free_pending_per_sender(DEFAULT_MAX_FREE_PENDING_PER_SENDER);
    pool.set_optional_fees_active(true);

    for nonce in 0..DEFAULT_MAX_FREE_PENDING_PER_SENDER as u64 {
        let signed = SignedTransaction::new_for_chain_with_tip(
            transfer(sender, nonce, to, 1),
            &sk,
            chain_id,
            0,
        );
        pool.insert(signed).unwrap();
    }
    let extra = SignedTransaction::new_for_chain_with_tip(
        transfer(sender, DEFAULT_MAX_FREE_PENDING_PER_SENDER as u64, to, 1),
        &sk,
        chain_id,
        0,
    );
    assert!(matches!(
        pool.insert(extra),
        Err(MempoolError::FreePendingLimitExceeded { .. })
    ));

    let tipped = SignedTransaction::new_for_chain_with_tip(
        transfer(sender, DEFAULT_MAX_FREE_PENDING_PER_SENDER as u64, to, 1),
        &sk,
        chain_id,
        1,
    );
    assert!(pool.insert(tipped).is_ok());
}

#[test]
fn tipped_signature_rejects_wrong_tip() {
    let sk = key();
    let sender = id(&sk);
    let to = AccountId([2u8; 32]);
    let chain_id = 6913u64;
    let signed =
        SignedTransaction::new_for_chain_with_tip(transfer(sender, 0, to, 1), &sk, chain_id, 7);
    assert!(signed
        .verify_with(TxSigPolicy {
            chain_id: Some(chain_id),
            require_v2: true,
        })
        .is_ok());
    let mut forged = signed.clone();
    forged.priority_tip = 8;
    assert!(forged
        .verify_with(TxSigPolicy {
            chain_id: Some(chain_id),
            require_v2: true,
        })
        .is_err());
}

#[test]
fn signed_tx_bincode_roundtrip_omits_zero_tip() {
    let sk = key();
    let sender = id(&sk);
    let tx = transfer(sender, 0, AccountId([2u8; 32]), 1);
    let untipped = SignedTransaction::new_for_chain(tx.clone(), &sk, 6913);
    let tipped = SignedTransaction::new_for_chain_with_tip(tx, &sk, 6913, 42);

    let enc0 = bincode::serialize(&untipped).unwrap();
    let enc_legacy = {
        #[derive(serde::Serialize)]
        struct Old {
            tx: Transaction,
            signature: boing_primitives::Signature,
        }
        bincode::serialize(&Old {
            tx: untipped.tx.clone(),
            signature: untipped.signature,
        })
        .unwrap()
    };
    assert_eq!(enc0, enc_legacy);
    assert_eq!(SignedTransaction::from_bincode(&enc0).unwrap(), untipped);

    let enc_tip = bincode::serialize(&tipped).unwrap();
    assert!(enc_tip.len() > enc0.len());
    assert_eq!(SignedTransaction::from_bincode(&enc_tip).unwrap(), tipped);
}
