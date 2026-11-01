//! v0.1.23 canonical on-chain QA policy gate (`qa_canonical_from`, testnet height 1700): block
//! execution uses the committed QA policy instead of the node-local one, Unsure deploys are
//! rejected while public QA membership is off, and in-contract CREATE2 uses the registry's
//! `max_bytecode_size`. Each behaviour is checked on both sides of the gate.

use boing_execution::{
    apply_due_qa_governance, canonical_qa_policy, BlockExecutor, ExecutionHistory, Vm, VmError,
    VmExecutionContext,
};
use boing_primitives::{
    AccessList, Account, AccountId, AccountState, ExecutionReceipt, Transaction,
    TransactionPayload, BASE_UNITS_PER_BOING as B,
};
use boing_qa::{QaPoolGovernanceConfig, RuleRegistry, DEFAULT_MAX_BYTECODE_SIZE};
use boing_state::StateStore;

const GATE: u64 = 1_700;
const PROPOSER: AccountId = AccountId([0x99; 32]);
const DEPLOYER: AccountId = AccountId([0x11; 32]);

fn funded() -> StateStore {
    let mut st = StateStore::new();
    st.insert(Account {
        id: DEPLOYER,
        state: AccountState {
            balance: 1_000_000 * B,
            ..Default::default()
        },
    });
    st
}

fn history(gate: Option<u64>) -> ExecutionHistory {
    ExecutionHistory {
        qa_canonical_from: gate,
        ..Default::default()
    }
}

fn exec_with(registry: RuleRegistry, pool: QaPoolGovernanceConfig) -> BlockExecutor {
    let mut e = BlockExecutor::with_policies(
        registry,
        pool,
        boing_tokenomics::NetworkFeePolicy::treasury_default(),
    );
    e.set_execution_history(Some(history(Some(GATE))));
    e
}

fn deploy(nonce: u64, bytecode: Vec<u8>, purpose: &str) -> Transaction {
    Transaction {
        nonce,
        sender: DEPLOYER,
        payload: TransactionPayload::ContractDeployWithPurposeAndMetadata {
            bytecode,
            purpose_category: purpose.into(),
            description_hash: None,
            asset_name: None,
            asset_symbol: None,
            create2_salt: None,
        },
        access_list: AccessList::default(),
    }
}

fn run(
    e: &BlockExecutor,
    h: u64,
    ts: u64,
    st: &mut StateStore,
    tx: Transaction,
) -> ExecutionReceipt {
    e.execute_block(h, ts, &[tx], st, PROPOSER)
        .expect("block")
        .1
        .remove(0)
}

fn meme_review_registry() -> RuleRegistry {
    RuleRegistry::new().with_always_review_categories(vec!["meme".into()])
}

#[test]
fn local_registry_governs_execution_only_below_gate() {
    // Local node registry caps bytecode at 1 byte; the canonical default allows 2 bytes.
    let local = RuleRegistry::new().with_max_bytecode_size(1);
    let e = exec_with(local, QaPoolGovernanceConfig::development_default());
    let mut st = funded();
    let r = run(
        &e,
        GATE - 1,
        0,
        &mut st,
        deploy(0, vec![0x00, 0x00], "dapp"),
    );
    assert!(
        !r.success,
        "below the gate the local registry applies: {r:?}"
    );
    let mut st = funded();
    let r = run(&e, GATE, 0, &mut st, deploy(0, vec![0x00, 0x00], "dapp"));
    assert!(
        r.success,
        "from the gate the committed default applies: {r:?}"
    );
}

#[test]
fn local_public_pool_ignored_from_gate() {
    // Local pool says public membership on; from the gate the canonical (default) registry decides.
    let e = exec_with(
        meme_review_registry(),
        QaPoolGovernanceConfig::production_default(),
    );
    let mut st = funded();
    let r = run(&e, GATE - 1, 0, &mut st, deploy(0, vec![0x00], "meme"));
    assert!(r.success);
    assert_eq!(
        r.return_data, b"qa_pending",
        "local public pool below the gate"
    );
    let mut st = funded();
    let r = run(&e, GATE, 0, &mut st, deploy(0, vec![0x00], "meme"));
    assert!(
        r.success,
        "canonical default registry has no always-review: {r:?}"
    );
    assert_ne!(r.return_data, b"qa_pending");
}

#[test]
fn unsure_rejected_from_gate_when_public_membership_off() {
    let e = exec_with(
        RuleRegistry::new(),
        QaPoolGovernanceConfig::development_default(),
    );
    let reg_json = serde_json::to_vec(&meme_review_registry()).unwrap();
    let committed = || {
        let mut st = funded();
        st.qa_policy_mut().registry_json = Some(reg_json.clone());
        st
    };
    // Below the gate the committed policy is ignored (local default: Allow).
    let mut st = committed();
    let r = run(&e, GATE - 1, 0, &mut st, deploy(0, vec![0x00], "meme"));
    assert!(r.success, "{r:?}");
    // From the gate with the protocol-default (public, v0.1.24) pool: goes to review.
    let mut st = committed();
    let r = run(&e, GATE, 0, &mut st, deploy(0, vec![0x00], "meme"));
    assert!(r.success, "{r:?}");
    assert_eq!(r.return_data, b"qa_pending");
    // From the gate: committed registry scores Unsure, committed pool public off => rejected.
    let mut st = committed();
    st.qa_policy_mut().pool_config_json =
        Some(serde_json::to_vec(&QaPoolGovernanceConfig::development_default()).unwrap());
    let r = run(&e, GATE, 0, &mut st, deploy(0, vec![0x00], "meme"));
    assert!(!r.success);
    assert!(
        r.error
            .as_deref()
            .unwrap_or("")
            .contains("Unsure with public QA membership off"),
        "{r:?}"
    );
    assert_eq!(st.get(&DEPLOYER).unwrap().nonce, 1, "nonce consumed");
    // With a committed public pool the Unsure deploy goes to review instead.
    let mut st = committed();
    st.qa_policy_mut().pool_config_json =
        Some(serde_json::to_vec(&QaPoolGovernanceConfig::production_default()).unwrap());
    let r = run(&e, GATE, 0, &mut st, deploy(0, vec![0x00], "meme"));
    assert!(r.success, "{r:?}");
    assert_eq!(r.return_data, b"qa_pending");
}

#[test]
fn unsure_at_vm_level_is_gated() {
    let mut vm = Vm::with_qa_registry(meme_review_registry());
    vm.set_qa_canonical_from(Some(GATE));
    let ctx = |h| VmExecutionContext {
        block_height: h,
        ..Default::default()
    };
    let mut st = funded();
    assert!(vm
        .execute_with_context(&deploy(0, vec![0x00], "meme"), &mut st, ctx(GATE - 1))
        .is_ok());
    let mut st = funded();
    assert!(matches!(
        vm.execute_with_context(&deploy(0, vec![0x00], "meme"), &mut st, ctx(GATE)),
        Err(VmError::QaRejected { .. })
    ));
    // Gate off (mainnet / dev): legacy behaviour at any height.
    let vm = Vm::with_qa_registry(meme_review_registry());
    let mut st = funded();
    assert!(vm
        .execute_with_context(&deploy(0, vec![0x00], "meme"), &mut st, ctx(u64::MAX))
        .is_ok());
}

fn push32(v: u64) -> Vec<u8> {
    let mut out = vec![0x7f];
    let mut w = [0u8; 32];
    w[24..].copy_from_slice(&v.to_be_bytes());
    out.extend_from_slice(&w);
    out
}

#[test]
fn create2_init_size_uses_registry_max_from_gate() {
    let size = DEFAULT_MAX_BYTECODE_SIZE as u64 + 1;
    // offset, size, salt, CREATE2 (pops salt, size, offset).
    let mut code = push32(0);
    code.extend(push32(size));
    code.extend(push32(7));
    code.push(0xf5);
    code.push(0x00);
    let contract = AccountId([0x42; 32]);
    let big = RuleRegistry::new().with_max_bytecode_size(DEFAULT_MAX_BYTECODE_SIZE * 2);
    let mut vm = Vm::with_qa_registry(big);
    vm.set_qa_canonical_from(Some(GATE));
    let call = |nonce| Transaction {
        nonce,
        sender: DEPLOYER,
        payload: TransactionPayload::ContractCall {
            contract,
            calldata: vec![],
        },
        access_list: AccessList::default(),
    };
    let ctx = |h| VmExecutionContext {
        block_height: h,
        vm_hardening: true,
        ..Default::default()
    };
    let mut st = funded();
    st.set_contract_code(contract, code.clone());
    let below = vm.execute_with_context(&call(0), &mut st, ctx(GATE - 1));
    assert!(
        matches!(below, Err(VmError::CallBufferTooLarge)),
        "below the gate the default cap applies: {below:?}"
    );
    let mut st = funded();
    st.set_contract_code(contract, code);
    let at = vm.execute_with_context(&call(0), &mut st, ctx(GATE));
    assert!(
        !matches!(at, Err(VmError::CallBufferTooLarge)),
        "from the gate the registry cap applies: {at:?}"
    );
}

#[test]
fn due_qa_governance_is_applied_only_from_gate() {
    let reg_json = serde_json::to_vec(&meme_review_registry()).unwrap();
    let voter = [0x55u8; 32];
    let make = || {
        let mut st = funded();
        let eng = st.governance_mut().engine_mut();
        let id = eng
            .propose_at(
                "qa_registry".into(),
                reg_json.clone(),
                std::time::UNIX_EPOCH,
            )
            .unwrap();
        eng.cast_vote(id, voter, true, 10).unwrap();
        eng.advance_to_cooling(id, std::time::UNIX_EPOCH).unwrap();
        let ends = eng.get(id).unwrap().cooling_ends_at.unwrap();
        let ts = ends
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        (st, ts)
    };
    let mut e = BlockExecutor::new();
    e.set_execution_history(Some(history(Some(GATE))));
    e.set_governance_consensus_from(Some(0));
    // Cooling not ended: nothing applied.
    let (mut st, ts) = make();
    assert_eq!(apply_due_qa_governance(&mut st, ts - 1), 0);
    assert!(st.qa_policy().is_empty());
    // Below the gate the block does not execute it.
    let (mut st, ts) = make();
    let r = run(&e, GATE - 1, ts, &mut st, deploy(0, vec![0x00], "meme"));
    assert!(r.success);
    assert!(st.qa_policy().is_empty());
    assert_eq!(st.governance().engine().list().len(), 1);
    // From the gate: executed at block start, so this very block already uses it.
    let (mut st, ts) = make();
    let root_before = st.state_root();
    let r = run(&e, GATE, ts, &mut st, deploy(0, vec![0x00], "meme"));
    assert_eq!(st.qa_policy().registry_json.as_deref(), Some(&reg_json[..]));
    assert!(st.governance().engine().list().is_empty());
    assert!(r.success, "meme now Unsure, default public pool: {r:?}");
    assert_eq!(r.return_data, b"qa_pending");
    assert_ne!(st.state_root(), root_before);
    let (reg, _) = canonical_qa_policy(&st);
    assert_eq!(
        serde_json::to_vec(&reg).unwrap(),
        reg_json,
        "canonical policy reads the committed registry"
    );
}
