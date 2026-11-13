//! Consensus-ordered ReportLiveness burns stake only during VM execution (H2).

use boing_execution::{Vm, VmExecutionContext};
use boing_primitives::{
    AccessList, Account, AccountId, AccountState, LivenessEvidence, LivenessMissAttestation,
    Transaction, TransactionPayload,
};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

fn ctx(h: u64) -> VmExecutionContext {
    VmExecutionContext {
        block_height: h,
        block_timestamp: 0,
        native_value: false,
        vm_hardening: false,
        blake3_opcode: false,
        jump_strict: true,
        qa_canonical: false,
    }
}

#[test]
fn report_liveness_burns_ten_percent_when_activated() {
    let reporter_key = SigningKey::generate(&mut OsRng);
    let reporter = AccountId(reporter_key.verifying_key().to_bytes());
    let offender = AccountId([0x22u8; 32]);
    let attester_key = SigningKey::generate(&mut OsRng);

    let mut state = boing_state::StateStore::new();
    state.insert(Account {
        id: reporter,
        state: AccountState {
            balance: 1_000_000,
            ..Default::default()
        },
    });
    state.insert(Account {
        id: offender,
        state: AccountState {
            stake: 10_000,
            ..Default::default()
        },
    });

    let evidence = LivenessEvidence::try_from_attestations(vec![LivenessMissAttestation::sign(
        42, offender, 2, &attester_key,
    )])
    .unwrap();

    let slash_id = state.slash_appeals().peek_next_slash_id();
    let escrow = boing_tokenomics::slash_escrow_account(slash_id);
    let tx = Transaction {
        nonce: 0,
        sender: reporter,
        payload: TransactionPayload::ReportLiveness { evidence },
        access_list: AccessList::new(
            vec![reporter, offender, escrow],
            vec![reporter, offender, escrow],
        ),
    };

    let mut vm = Vm::new();
    // Inactive by default.
    assert!(vm.execute_with_context(&tx, &mut state.snapshot(), ctx(1)).is_err());

    vm.set_liveness_slash_from(Some(1));
    vm.execute_with_context(&tx, &mut state, ctx(1))
        .expect("ReportLiveness executes when activated");

    assert_eq!(state.get(&offender).unwrap().stake, 9_000); // 10% of 10_000
    assert_eq!(
        state.get(&boing_tokenomics::slash_escrow_account(slash_id)).unwrap().balance,
        1_000
    );
    let row = state
        .slash_appeals()
        .find_liveness_at_height(&offender, 42)
        .expect("liveness slash row");
    assert!(matches!(
        row.reason,
        boing_state::ConsensusSlashReason::Liveness
    ));
    assert_eq!(row.amount, 1_000);
}

#[test]
fn report_liveness_identical_roots_on_two_clones() {
    let reporter_key = SigningKey::generate(&mut OsRng);
    let reporter = AccountId(reporter_key.verifying_key().to_bytes());
    let offender = AccountId([0x33u8; 32]);
    let a_key = SigningKey::generate(&mut OsRng);
    let b_key = SigningKey::generate(&mut OsRng);
    let c_key = SigningKey::generate(&mut OsRng);

    let mut state_a = boing_state::StateStore::new();
    state_a.insert(Account {
        id: reporter,
        state: AccountState {
            balance: 1_000_000,
            ..Default::default()
        },
    });
    state_a.insert(Account {
        id: offender,
        state: AccountState {
            stake: 10_000,
            pending_unbond: 2_000,
            ..Default::default()
        },
    });
    let mut state_b = state_a.snapshot();

    let evidence = LivenessEvidence::try_from_attestations(vec![
        LivenessMissAttestation::sign(7, offender, 4, &a_key),
        LivenessMissAttestation::sign(7, offender, 4, &b_key),
        LivenessMissAttestation::sign(7, offender, 4, &c_key),
    ])
    .unwrap();

    let mk_tx = |nonce: u64, evidence: LivenessEvidence| {
        let slash_id = 1u64; // peek would match on fresh registries
        let escrow = boing_tokenomics::slash_escrow_account(slash_id);
        Transaction {
            nonce,
            sender: reporter,
            payload: TransactionPayload::ReportLiveness { evidence },
            access_list: AccessList::new(
                vec![reporter, offender, escrow],
                vec![reporter, offender, escrow],
            ),
        }
    };

    let mut vm = Vm::new();
    vm.set_liveness_slash_from(Some(0));
    vm.execute_with_context(&mk_tx(0, evidence.clone()), &mut state_a, ctx(10))
        .unwrap();
    vm.execute_with_context(&mk_tx(0, evidence), &mut state_b, ctx(10))
        .unwrap();

    assert_eq!(
        state_a.state_root_at_height(10),
        state_b.state_root_at_height(10),
        "identical ReportLiveness txs must yield identical roots"
    );
    // 10% of (stake 10k + pending 2k) = 1200 from stake first
    assert_eq!(state_a.get(&offender).unwrap().stake, 8_800);
    assert_eq!(state_a.get(&offender).unwrap().pending_unbond, 2_000);
}

#[test]
fn liveness_slash_activation_default_never() {
    let vm = Vm::new();
    assert_eq!(vm.liveness_slash_from(), None);
    assert!(!vm.liveness_slash_active(u64::MAX));
}
