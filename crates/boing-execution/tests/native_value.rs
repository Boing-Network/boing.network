//! Native value on contract calls (native-value rules, testnet height 1150): value transfer, revert
//! refunds, insufficient balance, contract payouts, the activation gate, wBOING and the u128 v10 pool.

use boing_execution::{
    constant_product_pool_bytecode_v10, constant_product_pool_bytecode_v8, encode_approve_calldata,
    encode_mint_first_calldata, encode_set_tokens_calldata, encode_transfer_calldata,
    encode_transfer_from_calldata, encode_v8_add_liquidity_calldata,
    encode_v8_remove_liquidity_calldata, encode_v8_swap_calldata,
    encode_wboing_balance_of_calldata, encode_wboing_deposit_calldata,
    encode_wboing_total_supply_calldata, encode_wboing_withdraw_all_calldata,
    encode_wboing_withdraw_calldata, encode_wboing_withdraw_to_calldata, failed_tx_fee_amount_in,
    reference_fungible_template_v2_bytecode, reserve_a_key, reserve_b_key, total_lp_supply_key,
    wrapped_boing_bytecode, BlockExecutor, ExecutionHistory, Vm, VmError, VmExecutionContext,
    REF_FUNGIBLE_BALANCE_XOR,
};
use boing_primitives::{
    nonce_derived_contract_address, AccessList, Account, AccountId, AccountState, BatchCall,
    Denomination, ExecutionReceipt, Transaction, TransactionPayload, BASE_UNITS_PER_BOING as B,
};
use boing_state::StateStore;

/// Activation height used by the executor tests.
const H: u64 = 5;
const PROPOSER: AccountId = AccountId([0x99; 32]);

fn acct(b: u8) -> AccountId {
    AccountId([b; 32])
}

fn ctx(h: u64) -> VmExecutionContext {
    VmExecutionContext {
        block_height: h,
        block_timestamp: 0,
        native_value: false,
        vm_hardening: false,
        blake3_opcode: false,
    }
}

fn tx(sender: AccountId, nonce: u64, payload: TransactionPayload) -> Transaction {
    let mut t = Transaction {
        nonce,
        sender,
        payload,
        access_list: AccessList::default(),
    };
    t.access_list = t.suggested_parallel_access_list();
    t
}

fn funded(ids: &[(AccountId, u128)]) -> StateStore {
    let mut st = StateStore::new();
    for (id, bal) in ids {
        st.insert(Account {
            id: *id,
            state: AccountState {
                balance: *bal,
                ..Default::default()
            },
        });
    }
    st
}

fn bal(st: &StateStore, id: &AccountId) -> u128 {
    st.get(id).map(|s| s.balance).unwrap_or(0)
}

fn word_u128(w: &[u8]) -> u128 {
    assert_eq!(w.len(), 32);
    assert!(w[..16].iter().all(|b| *b == 0));
    u128::from_be_bytes(w[16..].try_into().unwrap())
}

fn amount_word(v: u128) -> Vec<u8> {
    let mut w = vec![0u8; 32];
    w[16..].copy_from_slice(&v.to_be_bytes());
    w
}

fn deploy_payload(code: Vec<u8>, purpose: &str) -> TransactionPayload {
    TransactionPayload::ContractDeployWithPurposeAndMetadata {
        bytecode: code,
        purpose_category: purpose.into(),
        description_hash: None,
        asset_name: None,
        asset_symbol: None,
        create2_salt: None,
    }
}

fn call(contract: AccountId, calldata: Vec<u8>) -> TransactionPayload {
    TransactionPayload::ContractCall { contract, calldata }
}

fn call_value(contract: AccountId, calldata: Vec<u8>, value: u128) -> TransactionPayload {
    TransactionPayload::ContractCallWithValue {
        contract,
        calldata,
        value,
    }
}

// Hand-assembled programs (raw Boing VM bytes).
/// `CALLVALUE; mstore(0); return(0, 32)`.
const ECHO_VALUE: &[u8] = &[0x34, 0x60, 0x00, 0x52, 0x60, 0x20, 0x60, 0x00, 0xf3];
/// `SELFBALANCE; mstore(0); return(0, 32)`.
const SELF_BALANCE: &[u8] = &[0x47, 0x60, 0x00, 0x52, 0x60, 0x20, 0x60, 0x00, 0xf3];
/// `BALANCE(calldata word 0); mstore(0); return(0, 32)`.
const BALANCE_OF: &[u8] = &[
    0x60, 0x00, 0x51, 0x31, 0x60, 0x00, 0x52, 0x60, 0x20, 0x60, 0x00, 0xf3,
];
/// `NATIVE_TRANSFER(amount = calldata word 0, to = CALLER); stop`.
const PAYOUT: &[u8] = &[0x60, 0x00, 0x51, 0x33, 0xf2, 0x00];
/// `CALLVALUE; pop-free fault: jump to 0xff` (accepts value, then faults).
const FAULT: &[u8] = &[0x34, 0x60, 0xff, 0x56];
/// `stop` (accepts any value).
const SINK: &[u8] = &[0x00];

/// Vm with the native-value rules active from genesis plus `sender` funded and `code` deployed at
/// nonce 0. Returns (vm, state, contract).
fn vm_with(code: &[u8], sender: AccountId, balance: u128) -> (Vm, StateStore, AccountId) {
    let vm = Vm::new();
    let mut st = funded(&[(sender, balance)]);
    vm.execute_with_context(
        &tx(sender, 0, deploy_payload(code.to_vec(), "dapp")),
        &mut st,
        ctx(1),
    )
    .expect("deploy");
    (vm, st, nonce_derived_contract_address(&sender, 0))
}

#[test]
fn value_moves_to_the_contract_and_callvalue_reads_it() {
    let alice = acct(0xa1);
    let (vm, mut st, c) = vm_with(ECHO_VALUE, alice, 100 * B);
    let out = vm
        .execute_with_context(
            &tx(alice, 1, call_value(c, vec![], 7 * B / 2)),
            &mut st,
            ctx(2),
        )
        .unwrap();
    assert_eq!(
        word_u128(&out.return_data),
        7 * B / 2,
        "CALLVALUE = attached value"
    );
    assert_eq!(bal(&st, &c), 7 * B / 2);
    assert_eq!(
        bal(&st, &alice),
        100 * B - 7 * B / 2,
        "VM debits the value (fees are the executor's)"
    );
    // A plain call sees CALLVALUE = 0.
    let out = vm
        .execute_with_context(&tx(alice, 2, call(c, vec![])), &mut st, ctx(3))
        .unwrap();
    assert_eq!(word_u128(&out.return_data), 0);
    assert_eq!(bal(&st, &c), 7 * B / 2);
}

#[test]
fn value_to_an_account_without_code_is_rejected() {
    let alice = acct(0xa1);
    let vm = Vm::new();
    let mut st = funded(&[(alice, 10 * B)]);
    let r = vm.execute_with_context(
        &tx(alice, 0, call_value(acct(0xee), vec![], B)),
        &mut st,
        ctx(1),
    );
    assert!(matches!(r, Err(VmError::AccountNotFound)), "{r:?}");
}

#[test]
fn selfbalance_and_balance_read_native_balances() {
    let alice = acct(0xa1);
    let (vm, mut st, c) = vm_with(SELF_BALANCE, alice, 100 * B);
    let out = vm
        .execute_with_context(&tx(alice, 1, call_value(c, vec![], 3 * B)), &mut st, ctx(2))
        .unwrap();
    assert_eq!(
        word_u128(&out.return_data),
        3 * B,
        "SELFBALANCE includes the value just attached"
    );
    let bob = acct(0xb0);
    vm.execute_with_context(
        &tx(alice, 2, deploy_payload(BALANCE_OF.to_vec(), "dapp")),
        &mut st,
        ctx(3),
    )
    .unwrap();
    let reader = nonce_derived_contract_address(&alice, 2);
    let out = vm
        .execute_with_context(
            &tx(alice, 3, call(reader, alice.0.to_vec())),
            &mut st,
            ctx(4),
        )
        .unwrap();
    assert_eq!(word_u128(&out.return_data), bal(&st, &alice));
    let out = vm
        .execute_with_context(&tx(alice, 4, call(reader, bob.0.to_vec())), &mut st, ctx(5))
        .unwrap();
    assert_eq!(word_u128(&out.return_data), 0, "missing account = 0");
}

// ---- Executor: fees, refunds, the gate ----

fn executor(native_from: u64) -> BlockExecutor {
    let mut e = BlockExecutor::new();
    e.set_execution_history(Some(ExecutionHistory {
        native_value_from: native_from,
        ..Default::default()
    }));
    e
}

fn failed_fee(e: &BlockExecutor) -> u128 {
    failed_tx_fee_amount_in(e.fee_policy(), Denomination::BaseUnits)
}

fn run(
    e: &BlockExecutor,
    h: u64,
    st: &mut StateStore,
    txs: &[Transaction],
) -> Vec<ExecutionReceipt> {
    e.execute_block(h, 0, txs, st, PROPOSER).expect("block").1
}

fn deploy_at(
    e: &BlockExecutor,
    h: u64,
    st: &mut StateStore,
    who: AccountId,
    nonce: u64,
    code: &[u8],
) -> AccountId {
    let r = run(
        e,
        h,
        st,
        &[tx(who, nonce, deploy_payload(code.to_vec(), "dapp"))],
    );
    assert!(r[0].success, "{:?}", r[0].error);
    nonce_derived_contract_address(&who, nonce)
}

#[test]
fn successful_value_call_pays_value_plus_fee() {
    let e = executor(H);
    let alice = acct(0xa1);
    let mut st = funded(&[(alice, 100 * B)]);
    let c = deploy_at(&e, H, &mut st, alice, 0, ECHO_VALUE);
    let before = bal(&st, &alice);
    let t = tx(alice, 1, call_value(c, vec![], 5 * B));
    let r = run(&e, H, &mut st, std::slice::from_ref(&t));
    assert!(r[0].success, "{:?}", r[0].error);
    let fee = e.network_fee_at(r[0].gas_used, &t.payload, H);
    assert!(fee > 0);
    assert_eq!(bal(&st, &alice), before - 5 * B - fee);
    assert_eq!(bal(&st, &c), 5 * B);
}

#[test]
fn revert_refunds_the_value_and_charges_the_failed_tx_fee() {
    let e = executor(H);
    let alice = acct(0xa1);
    let mut st = funded(&[(alice, 100 * B)]);
    let c = deploy_at(&e, H, &mut st, alice, 0, FAULT);
    let before = bal(&st, &alice);
    let r = run(
        &e,
        H,
        &mut st,
        &[tx(alice, 1, call_value(c, vec![], 5 * B))],
    );
    assert!(!r[0].success);
    assert_eq!(bal(&st, &c), 0, "value refunded");
    assert_eq!(
        bal(&st, &alice),
        before - failed_fee(&e),
        "only the failed-tx fee"
    );
    assert_eq!(st.get(&alice).unwrap().nonce, 2, "nonce consumed");
}

#[test]
fn insufficient_balance_for_the_value_fails_without_moving_it() {
    let e = executor(H);
    let alice = acct(0xa1);
    let mut st = funded(&[(alice, 10 * B)]);
    // ECHO_VALUE uses gas (a `STOP`-only contract costs 0 gas, so no fee).
    let c = deploy_at(&e, H, &mut st, alice, 0, ECHO_VALUE);
    let before = bal(&st, &alice);
    let r = run(
        &e,
        H,
        &mut st,
        &[tx(alice, 1, call_value(c, vec![], 11 * B))],
    );
    assert!(!r[0].success);
    assert!(
        r[0].error
            .as_deref()
            .unwrap_or("")
            .contains("Insufficient balance"),
        "{:?}",
        r[0].error
    );
    assert_eq!(bal(&st, &c), 0);
    assert_eq!(bal(&st, &alice), before - failed_fee(&e));
    // Exactly the balance: the value fits but the fee does not -> the whole call reverts.
    let all = bal(&st, &alice);
    let r = run(&e, H, &mut st, &[tx(alice, 2, call_value(c, vec![], all))]);
    assert!(!r[0].success, "value + fee must both be covered");
    assert_eq!(bal(&st, &c), 0, "value refunded when the fee fails");
    assert_eq!(bal(&st, &alice), all - failed_fee(&e).min(all));
}

#[test]
fn contract_pays_out_with_native_transfer() {
    let e = executor(H);
    let (alice, bob) = (acct(0xa1), acct(0xb0));
    let mut st = funded(&[(alice, 100 * B), (bob, 10 * B)]);
    let c = deploy_at(&e, H, &mut st, alice, 0, PAYOUT);
    // Fund it: value with calldata amount 0 (transfer of 0 is a no-op).
    let r = run(
        &e,
        H,
        &mut st,
        &[tx(alice, 1, call_value(c, amount_word(0), 8 * B))],
    );
    assert!(r[0].success, "{:?}", r[0].error);
    assert_eq!(bal(&st, &c), 8 * B);
    // Bob withdraws 3 BOING from it.
    let bob_before = bal(&st, &bob);
    let t = tx(bob, 0, call(c, amount_word(3 * B)));
    let r = run(&e, H, &mut st, std::slice::from_ref(&t));
    assert!(r[0].success, "{:?}", r[0].error);
    let fee = e.network_fee_at(r[0].gas_used, &t.payload, H);
    assert_eq!(bal(&st, &c), 5 * B);
    assert_eq!(bal(&st, &bob), bob_before + 3 * B - fee);
    // More than the contract holds: fault, nothing moves.
    let bob_before = bal(&st, &bob);
    let r = run(&e, H, &mut st, &[tx(bob, 1, call(c, amount_word(6 * B)))]);
    assert!(!r[0].success);
    assert_eq!(bal(&st, &c), 5 * B);
    assert_eq!(bal(&st, &bob), bob_before - failed_fee(&e));
    // Payout to a fresh account creates it.
    let carol = acct(0xc0);
    let mut st2 = st.snapshot();
    st2.insert(Account {
        id: carol,
        state: AccountState {
            balance: B,
            ..Default::default()
        },
    });
    let r = run(&e, H, &mut st2, &[tx(carol, 0, call(c, amount_word(B)))]);
    assert!(r[0].success, "{:?}", r[0].error);
    assert_eq!(bal(&st2, &c), 4 * B);
}

#[test]
fn native_value_rules_follow_the_activation_height() {
    let e = executor(H);
    let alice = acct(0xa1);
    let mut st = funded(&[(alice, 100 * B)]);
    let sink = deploy_at(&e, H - 2, &mut st, alice, 0, SINK);
    // H - 1: a value call fails (no value moves), deploys with the new opcodes are rejected by QA,
    // and the new bytes are invalid bytecode at run time (contract installed without QA here).
    let r = run(
        &e,
        H - 1,
        &mut st,
        &[tx(alice, 1, call_value(sink, vec![], B))],
    );
    assert!(!r[0].success);
    assert!(
        r[0].error.as_deref().unwrap().contains("not active"),
        "{:?}",
        r[0].error
    );
    assert_eq!(bal(&st, &sink), 0);
    let r = run(
        &e,
        H - 1,
        &mut st,
        &[tx(alice, 2, deploy_payload(ECHO_VALUE.to_vec(), "dapp"))],
    );
    assert!(!r[0].success);
    assert!(
        r[0].error.as_deref().unwrap().contains("INVALID_OPCODE"),
        "{:?}",
        r[0].error
    );
    let raw = acct(0x5b);
    st.insert(Account {
        id: raw,
        state: AccountState::default(),
    });
    st.set_contract_code(raw, SELF_BALANCE.to_vec());
    let r = run(&e, H - 1, &mut st, &[tx(alice, 3, call(raw, vec![]))]);
    assert!(!r[0].success);
    assert!(
        r[0].error.as_deref().unwrap().contains("Invalid bytecode"),
        "{:?}",
        r[0].error
    );
    // Batches with a value step fail too.
    let r = run(
        &e,
        H - 1,
        &mut st,
        &[tx(
            alice,
            4,
            TransactionPayload::Batch {
                calls: vec![BatchCall::ContractCallWithValue {
                    contract: sink,
                    calldata: vec![],
                    value: 1,
                }],
            },
        )],
    );
    assert!(!r[0].success);
    // H: all of it works.
    let r = run(&e, H, &mut st, &[tx(alice, 5, call_value(sink, vec![], B))]);
    assert!(r[0].success, "{:?}", r[0].error);
    assert_eq!(bal(&st, &sink), B);
    let echo = deploy_at(&e, H, &mut st, alice, 6, ECHO_VALUE);
    let r = run(&e, H, &mut st, &[tx(alice, 7, call_value(echo, vec![], 2))]);
    assert!(r[0].success && word_u128(&r[0].return_data) == 2);
    let r = run(&e, H, &mut st, &[tx(alice, 8, call(raw, vec![]))]);
    assert!(r[0].success, "{:?}", r[0].error);
}

#[test]
fn batch_value_steps_are_atomic() {
    let e = executor(0);
    let (alice, bob) = (acct(0xa1), acct(0xb0));
    let mut st = funded(&[(alice, 100 * B)]);
    let echo = deploy_at(&e, 1, &mut st, alice, 0, ECHO_VALUE);
    let fault = deploy_at(&e, 1, &mut st, alice, 1, FAULT);
    let r = run(
        &e,
        2,
        &mut st,
        &[tx(
            alice,
            2,
            TransactionPayload::Batch {
                calls: vec![
                    BatchCall::ContractCallWithValue {
                        contract: echo,
                        calldata: vec![],
                        value: 2 * B,
                    },
                    BatchCall::Transfer { to: bob, amount: B },
                ],
            },
        )],
    );
    assert!(r[0].success, "{:?}", r[0].error);
    assert_eq!(bal(&st, &echo), 2 * B);
    assert_eq!(bal(&st, &bob), B);
    // A later step faults: the earlier value step is refunded.
    let before = bal(&st, &alice);
    let r = run(
        &e,
        3,
        &mut st,
        &[tx(
            alice,
            3,
            TransactionPayload::Batch {
                calls: vec![
                    BatchCall::ContractCallWithValue {
                        contract: echo,
                        calldata: vec![],
                        value: 4 * B,
                    },
                    BatchCall::ContractCallWithValue {
                        contract: fault,
                        calldata: vec![],
                        value: 1,
                    },
                ],
            },
        )],
    );
    assert!(!r[0].success);
    assert_eq!(bal(&st, &echo), 2 * B);
    assert_eq!(bal(&st, &fault), 0);
    assert_eq!(bal(&st, &alice), before - failed_fee(&e));
}

// ---- wBOING ----

struct W {
    vm: Vm,
    st: StateStore,
    nonces: std::collections::HashMap<AccountId, u64>,
    w: AccountId,
}

impl W {
    fn new(holders: &[AccountId]) -> Self {
        let deployer = acct(0xd0);
        let mut ids: Vec<(AccountId, u128)> = holders.iter().map(|h| (*h, 1_000_000 * B)).collect();
        ids.push((deployer, 1_000 * B));
        let vm = Vm::new();
        let mut st = funded(&ids);
        vm.execute_with_context(
            &tx(
                deployer,
                0,
                deploy_payload(wrapped_boing_bytecode(), "token"),
            ),
            &mut st,
            ctx(1),
        )
        .expect("wBOING passes deploy QA once native value is active");
        let mut nonces = std::collections::HashMap::new();
        nonces.insert(deployer, 1);
        Self {
            vm,
            st,
            nonces,
            w: nonce_derived_contract_address(&deployer, 0),
        }
    }
    fn exec(&mut self, who: AccountId, p: TransactionPayload) -> Result<Vec<u8>, VmError> {
        let n = self.nonces.entry(who).or_insert(0);
        let t = tx(who, *n, p);
        *n += 1;
        // Like the executor: a failed tx reverts its writes and only consumes the nonce.
        let cp = self.st.checkpoint();
        match self.vm.execute_with_context(&t, &mut self.st, ctx(2)) {
            Ok(o) => Ok(o.return_data),
            Err(e) => {
                self.st.revert(cp);
                self.st.get_mut(&who).unwrap().nonce += 1;
                Err(e)
            }
        }
    }
    fn tok(&self, who: &AccountId) -> u128 {
        let mut k = [0u8; 32];
        for i in 0..32 {
            k[i] = who.0[i] ^ REF_FUNGIBLE_BALANCE_XOR[i];
        }
        let w = self.st.get_contract_storage(&self.w, &k);
        u128::from_be_bytes(w[16..].try_into().unwrap())
    }
    fn supply(&mut self) -> u128 {
        let r = self
            .exec(
                acct(0xd0),
                call(self.w, encode_wboing_total_supply_calldata()),
            )
            .unwrap();
        word_u128(&r)
    }
}

#[test]
fn wboing_wraps_and_unwraps_one_to_one() {
    let (alice, bob, carol) = (acct(0xa1), acct(0xb0), acct(0xc0));
    let mut w = W::new(&[alice, bob, carol]);
    let wb = w.w;
    let a0 = bal(&w.st, &alice);
    let r = w
        .exec(
            alice,
            call_value(wb, encode_wboing_deposit_calldata(), 5 * B + 1),
        )
        .unwrap();
    assert_eq!(word_u128(&r), 5 * B + 1);
    assert_eq!(w.tok(&alice), 5 * B + 1, "1:1 in base units");
    assert_eq!(bal(&w.st, &wb), 5 * B + 1);
    assert_eq!(bal(&w.st, &alice), a0 - 5 * B - 1);
    assert_eq!(w.supply(), 5 * B + 1);
    // Token v2 transfer / approve / transfer_from.
    w.exec(alice, call(wb, encode_transfer_calldata(&bob, 2 * B)))
        .unwrap();
    assert_eq!((w.tok(&alice), w.tok(&bob)), (3 * B + 1, 2 * B));
    let r = w
        .exec(alice, call(wb, encode_approve_calldata(&carol, B)))
        .unwrap();
    assert_eq!(word_u128(&r), 1);
    let r = w
        .exec(
            carol,
            call(wb, encode_transfer_from_calldata(&alice, &carol, B)),
        )
        .unwrap();
    assert_eq!(word_u128(&r), 1);
    let r = w
        .exec(
            carol,
            call(wb, encode_transfer_from_calldata(&alice, &carol, 1)),
        )
        .unwrap();
    assert!(
        r.is_empty(),
        "allowance used up: token-v2 failure = no return data"
    );
    assert!(
        w.exec(alice, call(wb, encode_transfer_calldata(&bob, 100 * B)))
            .is_err(),
        "short balance faults"
    );
    // withdraw / withdraw_to / withdraw_all.
    let b0 = bal(&w.st, &bob);
    let r = w
        .exec(bob, call(wb, encode_wboing_withdraw_calldata(B / 2)))
        .unwrap();
    assert_eq!(word_u128(&r), B / 2);
    assert_eq!(bal(&w.st, &bob), b0 + B / 2);
    let c0 = bal(&w.st, &carol);
    w.exec(
        bob,
        call(wb, encode_wboing_withdraw_to_calldata(B / 4, &carol)),
    )
    .unwrap();
    assert_eq!(bal(&w.st, &carol), c0 + B / 4);
    let b1 = bal(&w.st, &bob);
    let r = w
        .exec(bob, call(wb, encode_wboing_withdraw_all_calldata()))
        .unwrap();
    assert_eq!(
        word_u128(&r),
        2 * B - B / 2 - B / 4,
        "the rest of bob's wBOING"
    );
    assert_eq!((w.tok(&bob), bal(&w.st, &bob)), (0, b1 + 5 * B / 4));
    assert!(
        w.exec(bob, call(wb, encode_wboing_withdraw_all_calldata()))
            .is_err(),
        "nothing to unwrap"
    );
    assert!(
        w.exec(alice, call(wb, encode_wboing_withdraw_calldata(3 * B)))
            .is_err(),
        "more than held"
    );
    // Rejections: deposit without value, value on any other selector.
    assert!(w
        .exec(alice, call(wb, encode_wboing_deposit_calldata()))
        .is_err());
    assert!(w
        .exec(alice, call_value(wb, encode_wboing_withdraw_calldata(1), 1))
        .is_err());
    let r = w
        .exec(alice, call(wb, encode_wboing_balance_of_calldata(&alice)))
        .unwrap();
    assert_eq!(word_u128(&r), w.tok(&alice));
    // Invariant: supply == sum of balances == native balance of the contract.
    let sum = [alice, bob, carol].iter().map(|h| w.tok(h)).sum::<u128>();
    let s = w.supply();
    assert_eq!(s, sum);
    assert_eq!(bal(&w.st, &wb), s);
}

// ---- v10 pool ----

struct P {
    w: W,
    pool: AccountId,
    usd: AccountId,
}

/// wBOING / 18-decimal token pool (v10 by default), tokens set, `lp` funded with both.
fn pool_env(code: Vec<u8>, lp: AccountId, trader: AccountId) -> P {
    let mut w = W::new(&[lp, trader]);
    let admin = acct(0xd0);
    w.exec(
        admin,
        deploy_payload(reference_fungible_template_v2_bytecode(), "token"),
    )
    .unwrap();
    let usd = nonce_derived_contract_address(&admin, 1);
    // The whole u128 range: lp gets 2^127 - 12345, the trader the rest (~2^127).
    w.exec(
        admin,
        call(usd, encode_mint_first_calldata(&admin, u128::MAX)),
    )
    .unwrap();
    w.exec(
        admin,
        call(usd, encode_transfer_calldata(&lp, (1u128 << 127) - 12345)),
    )
    .unwrap();
    w.exec(
        admin,
        call(
            usd,
            encode_transfer_calldata(&trader, u128::MAX - ((1u128 << 127) - 12345)),
        ),
    )
    .unwrap();
    w.exec(admin, deploy_payload(code, "dapp")).unwrap();
    let pool = nonce_derived_contract_address(&admin, 5);
    w.exec(
        admin,
        call(pool, encode_set_tokens_calldata(&w.w.clone(), &usd)),
    )
    .unwrap();
    P { w, pool, usd }
}

impl P {
    fn slot(&self, k: [u8; 32]) -> u128 {
        let w = self.w.st.get_contract_storage(&self.pool, &k);
        assert!(w[..16].iter().all(|b| *b == 0), "slot above u128");
        u128::from_be_bytes(w[16..].try_into().unwrap())
    }
    fn reserves(&self) -> (u128, u128, u128) {
        (
            self.slot(reserve_a_key()),
            self.slot(reserve_b_key()),
            self.slot(total_lp_supply_key()),
        )
    }
    fn usd_bal(&self, who: &AccountId) -> u128 {
        let mut k = [0u8; 32];
        for i in 0..32 {
            k[i] = who.0[i] ^ REF_FUNGIBLE_BALANCE_XOR[i];
        }
        let w = self.w.st.get_contract_storage(&self.usd, &k);
        u128::from_be_bytes(w[16..].try_into().unwrap())
    }
    /// Wrap `boing`, approve both sides exactly, add liquidity in one atomic batch.
    fn add(&mut self, who: AccountId, boing: u128, usd: u128) -> Result<Vec<u8>, VmError> {
        let (wb, pool, usd_t) = (self.w.w, self.pool, self.usd);
        self.w.exec(
            who,
            TransactionPayload::Batch {
                calls: vec![
                    BatchCall::ContractCallWithValue {
                        contract: wb,
                        calldata: encode_wboing_deposit_calldata(),
                        value: boing,
                    },
                    BatchCall::ContractCall {
                        contract: wb,
                        calldata: encode_approve_calldata(&pool, boing),
                    },
                    BatchCall::ContractCall {
                        contract: usd_t,
                        calldata: encode_approve_calldata(&pool, usd),
                    },
                    BatchCall::ContractCall {
                        contract: pool,
                        calldata: encode_v8_add_liquidity_calldata(boing, usd, 1, 0),
                    },
                ],
            },
        )
    }
}

/// Pool math (v8 family): fee from the input first, then x*y=k, both rounded down.
fn quote(amount_in: u128, r_in: u128, r_out: u128, fee_bps: u128) -> u128 {
    use num_bigint::BigUint;
    let ainf =
        BigUint::from(amount_in) * BigUint::from(10_000 - fee_bps) / BigUint::from(10_000u32);
    let out = BigUint::from(r_out) * &ainf / (BigUint::from(r_in) + &ainf);
    u128::try_from(out).unwrap()
}

#[test]
fn v10_pool_swaps_boing_both_ways_in_one_batch() {
    let (lp, trader) = (acct(0x11), acct(0x22));
    let mut p = pool_env(constant_product_pool_bytecode_v10(), lp, trader);
    // 100 BOING : 10_000 USD (18 decimals each): far above the v8 2^64 cap.
    let r = p.add(lp, 100 * B, 10_000 * B).expect("add");
    assert_eq!(word_u128(&r), 100 * B, "first mint = min(a, b)");
    assert_eq!(p.reserves(), (100 * B, 10_000 * B, 100 * B));
    assert_eq!(bal(&p.w.st, &p.w.w), 100 * B, "wBOING is fully backed");

    // 1 BOING -> USD: wrap, approve, swap (direction 0 = A -> B) in one tx.
    let (wb, pool) = (p.w.w, p.pool);
    let want = quote(B, 100 * B, 10_000 * B, 30);
    let min_out = want * 9_950 / 10_000;
    let usd0 = p.usd_bal(&trader);
    let boing0 = bal(&p.w.st, &trader);
    let r =
        p.w.exec(
            trader,
            TransactionPayload::Batch {
                calls: vec![
                    BatchCall::ContractCallWithValue {
                        contract: wb,
                        calldata: encode_wboing_deposit_calldata(),
                        value: B,
                    },
                    BatchCall::ContractCall {
                        contract: wb,
                        calldata: encode_approve_calldata(&pool, B),
                    },
                    BatchCall::ContractCall {
                        contract: pool,
                        calldata: encode_v8_swap_calldata(0, B, min_out, 0),
                    },
                ],
            },
        )
        .expect("wrap + swap");
    assert_eq!(word_u128(&r), want, "v10 returns amount_out");
    assert_eq!(p.usd_bal(&trader), usd0 + want);
    assert_eq!(bal(&p.w.st, &trader), boing0 - B);
    assert_eq!(p.w.tok(&trader), 0, "nothing left wrapped");

    // USD -> BOING: approve, swap (direction 1), unwrap everything, in one tx.
    let (ra, rb, _) = p.reserves();
    let back = quote(want, rb, ra, 30);
    let usd_t = p.usd;
    let boing1 = bal(&p.w.st, &trader);
    p.w.exec(
        trader,
        TransactionPayload::Batch {
            calls: vec![
                BatchCall::ContractCall {
                    contract: usd_t,
                    calldata: encode_approve_calldata(&pool, want),
                },
                BatchCall::ContractCall {
                    contract: pool,
                    calldata: encode_v8_swap_calldata(1, want, back, 0),
                },
                BatchCall::ContractCall {
                    contract: wb,
                    calldata: encode_wboing_withdraw_all_calldata(),
                },
            ],
        },
    )
    .expect("swap + unwrap");
    assert_eq!(
        bal(&p.w.st, &trader),
        boing1 + back,
        "received native BOING"
    );
    assert!(back < B, "round trip pays the fee twice");
    assert_eq!(p.w.tok(&trader), 0);
    // min_out one above the quote: the whole batch reverts (wrap included).
    let boing2 = bal(&p.w.st, &trader);
    let q = quote(B, p.reserves().0, p.reserves().1, 30);
    assert!(p
        .w
        .exec(
            trader,
            TransactionPayload::Batch {
                calls: vec![
                    BatchCall::ContractCallWithValue {
                        contract: wb,
                        calldata: encode_wboing_deposit_calldata(),
                        value: B
                    },
                    BatchCall::ContractCall {
                        contract: wb,
                        calldata: encode_approve_calldata(&pool, B)
                    },
                    BatchCall::ContractCall {
                        contract: pool,
                        calldata: encode_v8_swap_calldata(0, B, q + 1, 0)
                    },
                ],
            }
        )
        .is_err());
    assert_eq!(
        bal(&p.w.st, &trader),
        boing2,
        "value refunded with the failed batch"
    );
    // Custody invariant.
    let (ra, rb, _) = p.reserves();
    assert_eq!(ra, p.w.tok(&pool));
    assert_eq!(rb, p.usd_bal(&pool));
}

#[test]
fn v10_handles_reserves_near_2_pow_128_without_overflow() {
    let (lp, trader) = (acct(0x11), acct(0x22));
    let mut p = pool_env(constant_product_pool_bytecode_v10(), lp, trader);
    let (usd_t, pool) = (p.usd, p.pool);
    // USD side close to the u128 bound, wBOING side 900k BOING (lp holds 1M).
    let big = (1u128 << 127) - 12345;
    p.add(lp, 900_000 * B, big).expect("large add");
    let (ra, rb, t) = p.reserves();
    assert_eq!((ra, rb, t), (900_000 * B, big, 900_000 * B));
    // Swap a huge USD amount in: products stay exact (checked against a big-integer quote).
    let ain = (1u128 << 125) + 777;
    let want = quote(ain, rb, ra, 30);
    let r =
        p.w.exec(
            trader,
            TransactionPayload::Batch {
                calls: vec![
                    BatchCall::ContractCall {
                        contract: usd_t,
                        calldata: encode_approve_calldata(&pool, ain),
                    },
                    BatchCall::ContractCall {
                        contract: pool,
                        calldata: encode_v8_swap_calldata(1, ain, want, 0),
                    },
                ],
            },
        )
        .expect("huge swap");
    assert_eq!(word_u128(&r), want);
    let (ra2, rb2, _) = p.reserves();
    assert_eq!((ra2, rb2), (ra - want, rb + ain));
    // The trader swaps in its whole remaining balance: the pool then holds the entire token supply,
    // u128::MAX = 2^128 - 1 (the largest reserve a real token can reach), and the math stays exact.
    let rest = p.usd_bal(&trader);
    let want = quote(rest, rb2, ra2, 30);
    let r =
        p.w.exec(
            trader,
            TransactionPayload::Batch {
                calls: vec![
                    BatchCall::ContractCall {
                        contract: usd_t,
                        calldata: encode_approve_calldata(&pool, rest),
                    },
                    BatchCall::ContractCall {
                        contract: pool,
                        calldata: encode_v8_swap_calldata(1, rest, want, 0),
                    },
                ],
            },
        )
        .expect("swap up to the u128 limit");
    assert_eq!(word_u128(&r), want);
    assert_eq!(p.reserves().1, u128::MAX, "reserve = whole supply");
    assert_eq!(p.usd_bal(&pool), u128::MAX);
    // Remove all liquidity: exact proportional payout.
    let (ra3, rb3, t3) = p.reserves();
    let usd0 = p.usd_bal(&lp);
    p.w.exec(
        lp,
        call(pool, encode_v8_remove_liquidity_calldata(t3, 1, 1, 0)),
    )
    .expect("remove");
    assert_eq!(
        p.w.tok(&lp),
        ra3,
        "burn * reserve / total, with burn = total"
    );
    assert_eq!(p.usd_bal(&lp), usd0 + rb3);
    assert_eq!(p.reserves(), (0, 0, 0));
}

#[test]
fn v8_still_caps_at_2_pow_64_and_v10_bytecode_is_distinct() {
    let (lp, trader) = (acct(0x11), acct(0x22));
    let mut p = pool_env(constant_product_pool_bytecode_v8(), lp, trader);
    assert!(
        p.add(lp, 100 * B, 100 * B).is_err(),
        "100e18 > 2^64: v8 rejects (why v10 exists)"
    );
    assert!(p.add(lp, 10 * B, 10 * B).is_ok());
    assert_ne!(
        constant_product_pool_bytecode_v10(),
        constant_product_pool_bytecode_v8()
    );
}

#[test]
fn v10_rejects_amounts_at_or_above_2_pow_128() {
    let (lp, trader) = (acct(0x11), acct(0x22));
    let mut p = pool_env(constant_product_pool_bytecode_v10(), lp, trader);
    p.add(lp, 10 * B, 10 * B).unwrap();
    let pool = p.pool;
    // amount_in word = 2^128 (only expressible by hand-built calldata).
    let mut cd = encode_v8_swap_calldata(1, 0, 1, 0);
    cd[64 + 15] = 1;
    assert!(p.w.exec(trader, call(pool, cd)).is_err());
}

#[test]
fn bincode_layout_of_value_calls_is_appended() {
    // Old variants keep their indices; the value variants are appended (payload 20, batch step 2).
    let contract = AccountId([0x11; 32]);
    let value: u128 = 5 * B;
    let mut want = 20u32.to_le_bytes().to_vec();
    want.extend_from_slice(&contract.0);
    want.extend_from_slice(&2u64.to_le_bytes());
    want.extend_from_slice(&[0xaa, 0xbb]);
    want.extend_from_slice(&value.to_le_bytes());
    let got = bincode::serialize(&call_value(contract, vec![0xaa, 0xbb], value)).unwrap();
    assert_eq!(hex::encode(&got), hex::encode(&want));

    let batch = TransactionPayload::Batch {
        calls: vec![BatchCall::ContractCallWithValue {
            contract,
            calldata: vec![0xaa, 0xbb],
            value,
        }],
    };
    let mut want = 9u32.to_le_bytes().to_vec();
    want.extend_from_slice(&1u64.to_le_bytes());
    want.extend_from_slice(&2u32.to_le_bytes());
    want.extend_from_slice(&contract.0);
    want.extend_from_slice(&2u64.to_le_bytes());
    want.extend_from_slice(&[0xaa, 0xbb]);
    want.extend_from_slice(&value.to_le_bytes());
    assert_eq!(
        hex::encode(bincode::serialize(&batch).unwrap()),
        hex::encode(&want)
    );
}
