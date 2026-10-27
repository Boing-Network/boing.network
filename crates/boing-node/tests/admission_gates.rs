//! Mempool admission gates: cheap account-level checks run before any state clone, and the
//! clone-free Transfer fast path must decide exactly like the snapshot + VM dry run.

use boing_node::mempool::MempoolError;
use boing_node::node::BoingNode;
use boing_primitives::{
    AccessList, Account, AccountId, AccountState, SignedTransaction, Transaction,
    TransactionPayload,
};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

fn key() -> (SigningKey, AccountId) {
    let sk = SigningKey::generate(&mut OsRng);
    let id = AccountId(sk.verifying_key().to_bytes());
    (sk, id)
}

fn fund(node: &mut BoingNode, id: AccountId, balance: u128, nonce: u64) {
    node.state.insert(Account {
        id,
        state: AccountState {
            balance,
            nonce,
            ..Default::default()
        },
    });
}

fn transfer(
    sk: &SigningKey,
    sender: AccountId,
    nonce: u64,
    to: AccountId,
    amount: u128,
    tip: u128,
) -> SignedTransaction {
    let tx = Transaction {
        nonce,
        sender,
        payload: TransactionPayload::Transfer { to, amount },
        access_list: AccessList::new(vec![sender, to], vec![sender, to]),
    };
    if tip == 0 {
        SignedTransaction::new(tx, sk)
    } else {
        // Only used by the differential test, which does not verify signatures.
        SignedTransaction::new_for_chain_with_tip(tx, sk, 6913, tip)
    }
}

fn node() -> BoingNode {
    let (_, proposer) = key();
    BoingNode::with_validators(vec![proposer], proposer, None)
}

#[test]
fn unknown_sender_is_rejected_without_touching_the_pool() {
    let mut n = node();
    let (sk, id) = key();
    let (_, to) = key();
    let _ = &mut n;
    let r = n.submit_transaction(transfer(&sk, id, 0, to, 1, 0));
    assert!(matches!(r, Err(MempoolError::UnknownSender)), "{r:?}");
    assert_eq!(n.mempool.len(), 0);
}

#[test]
fn stale_nonce_rejected_current_and_future_admitted() {
    let mut n = node();
    let (sk, id) = key();
    let (_, to) = key();
    fund(&mut n, id, 1_000_000_000_000_000_000_000, 5);
    let r = n.submit_transaction(transfer(&sk, id, 4, to, 1, 0));
    assert!(
        matches!(
            r,
            Err(MempoolError::StaleNonce {
                expected: 5,
                got: 4
            })
        ),
        "{r:?}"
    );
    n.submit_transaction(transfer(&sk, id, 5, to, 1, 0))
        .expect("current nonce admitted");
    n.submit_transaction(transfer(&sk, id, 7, to, 1, 0))
        .expect("future nonce admitted (queues)");
    assert_eq!(n.mempool.len(), 2);
}

#[test]
fn balance_below_failed_tx_fee_is_rejected() {
    let mut n = node();
    let (sk, id) = key();
    let (_, to) = key();
    fund(&mut n, id, 1, 0); // 1 base unit: cannot pay even a failed tx
    let r = n.submit_transaction(transfer(&sk, id, 0, to, 1_000, 0));
    assert!(
        matches!(r, Err(MempoolError::InsufficientFee { have: 1, .. })),
        "{r:?}"
    );
    assert_eq!(n.mempool.len(), 0);
}

#[test]
fn bad_signature_rejected_before_state_checks() {
    let mut n = node();
    let (_, id) = key();
    let (other, _) = key();
    let (_, to) = key();
    // Sender is unknown AND the signature is wrong: the signature error must win (checked first).
    let r = n.submit_transaction(transfer(&other, id, 0, to, 1, 0));
    assert!(matches!(r, Err(MempoolError::InvalidSignature)), "{r:?}");
    let _ = &mut n;
}

/// The clone-free Transfer path and the snapshot + VM dry run must agree on every input.
#[test]
fn transfer_fast_path_matches_vm_dry_run() {
    let fee_21k =
        boing_tokenomics::fee_for_gas_in(21_000, boing_tokenomics::Denomination::BaseUnits);
    assert!(fee_21k > 0);
    let balances = [
        0u128,
        1,
        fee_21k - 1,
        fee_21k,
        fee_21k + 1,
        10 * fee_21k,
        1_000 * fee_21k,
    ];
    let amounts = [
        0u128,
        1,
        fee_21k,
        5 * fee_21k,
        10 * fee_21k,
        2_000 * fee_21k,
    ];
    let tips = [0u128, 1, fee_21k];
    let nonces = [(0u64, 0u64), (0, 1), (3, 3), (3, 2), (3, 4)]; // (account nonce, tx nonce)
    let mut checked = 0;
    for &bal in &balances {
        for &amt in &amounts {
            for &tip in &tips {
                for &(acct_nonce, tx_nonce) in &nonces {
                    for self_transfer in [false, true] {
                        let mut n = node();
                        let (sk, id) = key();
                        let (_, other) = key();
                        let to = if self_transfer { id } else { other };
                        fund(&mut n, id, bal, acct_nonce);
                        let signed = transfer(&sk, id, tx_nonce, to, amt, tip);
                        let fast = n.precheck_transfer_fast(&signed, &to, amt);
                        let full = n.precheck_sender_can_pay_fee_full(&signed);
                        assert_eq!(
                            format!("{fast:?}"),
                            format!("{full:?}"),
                            "bal={bal} amt={amt} tip={tip} nonce=({acct_nonce},{tx_nonce}) self={self_transfer}"
                        );
                        checked += 1;
                    }
                }
            }
        }
    }
    assert!(checked > 1000);
}
