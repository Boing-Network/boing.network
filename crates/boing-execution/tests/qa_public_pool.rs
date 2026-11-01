//! v0.1.24 canonical public QA pool (testnet gate `qa_canonical_from`, 1700): with no committed
//! `qa_pool_config`, the canonical pool is [`QaPoolGovernanceConfig::canonical_default`] (public
//! membership), so Unsure deploys are registered for review instead of rejected; reviews resolve
//! by 2 agreeing votes from non-deployer accounts, are capped, and expire (rejected) after
//! `review_window_blocks`. Each behaviour is checked on both sides of the gate.

use boing_execution::{canonical_qa_policy, BlockExecutor, ExecutionHistory};
use boing_primitives::{
    AccessList, Account, AccountId, AccountState, ExecutionReceipt, QaPoolVoteKind, Transaction,
    TransactionPayload, BASE_UNITS_PER_BOING as B,
};
use boing_qa::{QaPoolGovernanceConfig, RuleRegistry, CANONICAL_REVIEW_WINDOW_BLOCKS};
use boing_state::StateStore;

const GATE: u64 = 1_700;
const PROPOSER: AccountId = AccountId([0x99; 32]);
const DEPLOYER: AccountId = AccountId([0x11; 32]);
const V1: AccountId = AccountId([0x21; 32]);
const V2: AccountId = AccountId([0x22; 32]);
const V3: AccountId = AccountId([0x23; 32]);

fn funded() -> StateStore {
    let mut st = StateStore::new();
    for id in [DEPLOYER, V1, V2, V3] {
        st.insert(Account {
            id,
            state: AccountState {
                balance: 1_000_000 * B,
                ..Default::default()
            },
        });
    }
    // Committed registry that always reviews "meme" (scores Unsure).
    st.qa_policy_mut().registry_json = Some(
        serde_json::to_vec(&RuleRegistry::new().with_always_review_categories(vec!["meme".into()]))
            .unwrap(),
    );
    st
}

fn exec(gate: Option<u64>) -> BlockExecutor {
    let mut e = BlockExecutor::with_policies(
        RuleRegistry::new().with_always_review_categories(vec!["meme".into()]),
        QaPoolGovernanceConfig::development_default(),
        boing_tokenomics::NetworkFeePolicy::treasury_default(),
    );
    e.set_execution_history(Some(ExecutionHistory {
        qa_canonical_from: gate,
        ..Default::default()
    }));
    e
}

fn deploy(nonce: u64, byte: u8) -> Transaction {
    Transaction {
        nonce,
        sender: DEPLOYER,
        payload: TransactionPayload::ContractDeployWithPurposeAndMetadata {
            bytecode: vec![byte],
            purpose_category: "meme".into(),
            description_hash: None,
            asset_name: None,
            asset_symbol: None,
            create2_salt: None,
        },
        access_list: AccessList::default(),
    }
}

fn vote(sender: AccountId, nonce: u64, subject: &Transaction, v: QaPoolVoteKind) -> Transaction {
    Transaction {
        nonce,
        sender,
        payload: TransactionPayload::QaPoolVote {
            subject: subject.id(),
            vote: v,
        },
        access_list: AccessList::default(),
    }
}

fn run(e: &BlockExecutor, h: u64, st: &mut StateStore, tx: Transaction) -> ExecutionReceipt {
    e.execute_block(h, 0, &[tx], st, PROPOSER)
        .expect("block")
        .1
        .remove(0)
}

#[test]
fn canonical_default_is_public_and_valid() {
    let c = QaPoolGovernanceConfig::canonical_default();
    assert!(c.public_membership && !c.dev_open_voting);
    assert!(c.administrators.is_empty());
    assert_eq!(c.min_voter_stake, 0);
    assert_eq!(c.min_quorum_votes, 2);
    assert_eq!(c.reward_per_counted_vote, 0);
    assert_eq!(c.review_window_blocks, CANONICAL_REVIEW_WINDOW_BLOCKS);
    assert!(c.validate().is_ok());
    assert!(c.accepts_new_pending());
    // Old governance JSON without the new field still parses (window 0 = no on-chain expiry).
    let legacy = serde_json::to_value(QaPoolGovernanceConfig::development_default()).unwrap();
    let mut m = legacy.as_object().unwrap().clone();
    m.remove("review_window_blocks");
    let back: QaPoolGovernanceConfig = serde_json::from_value(m.into()).unwrap();
    assert_eq!(back.review_window_blocks, 0);
    assert_eq!(canonical_qa_policy(&StateStore::new()).1, c);
}

#[test]
fn unsure_goes_to_public_review_only_from_gate() {
    let e = exec(Some(GATE));
    // Below the gate: local dev pool (public off) => legacy path, deployed directly.
    let mut st = funded();
    let r = run(&e, GATE - 1, &mut st, deploy(0, 1));
    assert!(r.success);
    assert_ne!(r.return_data, b"qa_pending");
    // From the gate: canonical public pool => registered for review, nonce consumed.
    let mut st = funded();
    let d = deploy(0, 1);
    let r = run(&e, GATE, &mut st, d.clone());
    assert!(r.success, "{r:?}");
    assert_eq!(r.return_data, b"qa_pending");
    assert!(st.qa_pending(&d.id()).is_some());
    assert_eq!(st.get(&DEPLOYER).unwrap().nonce, 1);
    // Gate off (mainnet): unchanged legacy behaviour at any height.
    let e = exec(None);
    let mut st = funded();
    let r = run(&e, 1_000_000, &mut st, deploy(0, 1));
    assert_ne!(r.return_data, b"qa_pending");
}

#[test]
fn two_agreeing_votes_resolve_and_deployer_cannot_vote() {
    let e = exec(Some(GATE));
    for (kind, want) in [
        (QaPoolVoteKind::Allow, &b"qa_vote_allow"[..]),
        (QaPoolVoteKind::Reject, &b"qa_vote_reject"[..]),
    ] {
        let mut st = funded();
        let d = deploy(0, 1);
        assert_eq!(run(&e, GATE, &mut st, d.clone()).return_data, b"qa_pending");
        let own = run(&e, GATE + 1, &mut st, vote(DEPLOYER, 1, &d, kind));
        assert!(!own.success, "deployer conflict: {own:?}");
        let r1 = run(&e, GATE + 2, &mut st, vote(V1, 0, &d, kind));
        assert!(r1.success, "{r1:?}");
        assert_eq!(
            r1.return_data, b"qa_vote_pending",
            "1 vote < min_quorum_votes 2"
        );
        let r2 = run(&e, GATE + 3, &mut st, vote(V2, 0, &d, kind));
        assert!(r2.success, "{r2:?}");
        assert_eq!(r2.return_data, want);
        assert!(st.qa_pending(&d.id()).is_none());
        // No voter rewards from the canonical default.
        assert_eq!(st.get(&V1).unwrap().balance, st.get(&V2).unwrap().balance);
    }
    // Split 1-1 stays pending; a third vote decides 2-1.
    let mut st = funded();
    let d = deploy(0, 1);
    run(&e, GATE, &mut st, d.clone());
    run(
        &e,
        GATE + 1,
        &mut st,
        vote(V1, 0, &d, QaPoolVoteKind::Allow),
    );
    let r = run(
        &e,
        GATE + 2,
        &mut st,
        vote(V2, 0, &d, QaPoolVoteKind::Reject),
    );
    assert_eq!(r.return_data, b"qa_vote_pending");
    let r = run(
        &e,
        GATE + 3,
        &mut st,
        vote(V3, 0, &d, QaPoolVoteKind::Allow),
    );
    assert_eq!(r.return_data, b"qa_vote_allow");
}

#[test]
fn undecided_items_expire_after_window_only_from_gate() {
    let e = exec(Some(GATE));
    let mut st = funded();
    let d = deploy(0, 1);
    run(&e, GATE, &mut st, d.clone());
    // One block before expiry it is still pending.
    let w = CANONICAL_REVIEW_WINDOW_BLOCKS;
    run(
        &e,
        GATE + w - 1,
        &mut st,
        vote(V1, 0, &d, QaPoolVoteKind::Allow),
    );
    assert!(st.qa_pending(&d.id()).is_some());
    // At entered + window it is dropped at block start; a late vote finds nothing.
    let r = run(
        &e,
        GATE + w,
        &mut st,
        vote(V2, 0, &d, QaPoolVoteKind::Allow),
    );
    assert!(st.qa_pending(&d.id()).is_none());
    assert!(!r.success, "{r:?}");
    // Below the gate nothing expires (legacy records are untouched).
    let e = exec(Some(u64::MAX));
    let mut st = funded();
    st.insert_qa_pending(d.id(), boing_state::QaPendingRecord::new(d.clone(), 0));
    run(
        &e,
        10_000,
        &mut st,
        vote(V1, 0, &deploy(9, 9), QaPoolVoteKind::Allow),
    );
    assert!(st.qa_pending(&d.id()).is_some());
}

#[test]
fn queue_caps_apply_from_gate() {
    let e = exec(Some(GATE));
    let mut st = funded();
    // max_pending_per_deployer = 2 => third Unsure deploy from the same account is rejected.
    for n in 0..2 {
        assert_eq!(
            run(&e, GATE + n, &mut st, deploy(n, n as u8 + 1)).return_data,
            b"qa_pending"
        );
    }
    let r = run(&e, GATE + 2, &mut st, deploy(2, 3));
    assert!(!r.success);
    assert!(
        r.error.as_deref().unwrap_or("").contains("slots are full"),
        "{r:?}"
    );
    assert_eq!(st.get(&DEPLOYER).unwrap().nonce, 3, "nonce consumed");
}
