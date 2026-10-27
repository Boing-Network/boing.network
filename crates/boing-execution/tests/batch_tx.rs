//! Native atomic batch transactions (`TransactionPayload::Batch`).

use boing_execution::reference_fungible_secured::REF_SECURED_BALANCE_XOR;
use boing_execution::{
    encode_mint_first_calldata, encode_transfer_calldata,
    reference_fungible_secured_pinned_default_deploy_bytecode, BlockExecutor, Vm, VmError,
    VmExecutionContext, GAS_BATCH_BASE, GAS_PER_BATCH_CONTRACT_CALL_STEP, GAS_PER_BATCH_TRANSFER,
};
use boing_primitives::{
    nonce_derived_contract_address, validate_batch_calls, AccessList, Account, AccountId,
    AccountState, BatchCall, BatchValidationError, Transaction, TransactionPayload,
    MAX_BATCH_CALLDATA_BYTES, MAX_BATCH_CALLS,
};
use boing_state::StateStore;
use boing_tokenomics::NetworkFeePolicy;

/// Network fee in base units (chains without history use base units from genesis).
fn network_fee_amount(gas: u64, payload: &TransactionPayload, policy: &NetworkFeePolicy) -> u128 {
    boing_tokenomics::network_fee_amount_in(
        gas,
        payload,
        policy,
        boing_tokenomics::Denomination::BaseUnits,
    )
}

const CTX: VmExecutionContext = VmExecutionContext {
    block_height: 10,
    block_timestamp: 1_700_000_000,
    native_value: false,
};

fn acct(b: u8) -> AccountId {
    AccountId([b; 32])
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

fn bal_key(holder: &AccountId) -> [u8; 32] {
    let mut k = [0u8; 32];
    for i in 0..32 {
        k[i] = holder.0[i] ^ REF_SECURED_BALANCE_XOR[i];
    }
    k
}

fn token_balance(state: &StateStore, token: &AccountId, holder: &AccountId) -> u128 {
    let w = state.get_contract_storage(token, &bal_key(holder));
    u128::from_be_bytes(w[16..32].try_into().unwrap())
}

/// Funded sender + deployed secured token with 1_000 minted to the sender.
fn setup() -> (Vm, StateStore, AccountId, AccountId) {
    let alice = acct(0xa1);
    let mut state = StateStore::new();
    state.insert(Account {
        id: alice,
        state: AccountState {
            balance: 10_000_000 * boing_primitives::BASE_UNITS_PER_BOING,
            ..Default::default()
        },
    });
    let vm = Vm::new();
    vm.execute_with_context(
        &tx(
            alice,
            0,
            TransactionPayload::ContractDeployWithPurposeAndMetadata {
                bytecode: reference_fungible_secured_pinned_default_deploy_bytecode(),
                purpose_category: "token".into(),
                description_hash: None,
                asset_name: None,
                asset_symbol: None,
                create2_salt: None,
            },
        ),
        &mut state,
        CTX,
    )
    .unwrap();
    let token = nonce_derived_contract_address(&alice, 0);
    vm.execute_with_context(
        &tx(
            alice,
            1,
            TransactionPayload::ContractCall {
                contract: token,
                calldata: encode_mint_first_calldata(&alice, 1_000),
            },
        ),
        &mut state,
        CTX,
    )
    .unwrap();
    (vm, state, alice, token)
}

#[test]
fn batch_applies_boing_and_token_transfers_under_one_nonce() {
    let (vm, mut state, alice, token) = setup();
    let (bob, carol) = (acct(0xb0), acct(0xc0));
    let before = state.get(&alice).unwrap().balance;
    let batch = tx(
        alice,
        2,
        TransactionPayload::Batch {
            calls: vec![
                BatchCall::Transfer {
                    to: bob,
                    amount: 100,
                },
                BatchCall::Transfer {
                    to: carol,
                    amount: 250,
                },
                BatchCall::ContractCall {
                    contract: token,
                    calldata: encode_transfer_calldata(&bob, 40),
                },
                BatchCall::ContractCall {
                    contract: token,
                    calldata: encode_transfer_calldata(&carol, 60),
                },
            ],
        },
    );
    let out = vm.execute_with_context(&batch, &mut state, CTX).unwrap();
    assert_eq!(
        state.get(&alice).unwrap().nonce,
        3,
        "one nonce for the whole batch"
    );
    assert_eq!(state.get(&alice).unwrap().balance, before - 350);
    assert_eq!(state.get(&bob).unwrap().balance, 100);
    assert_eq!(state.get(&carol).unwrap().balance, 250);
    assert_eq!(token_balance(&state, &token, &bob), 40);
    assert_eq!(token_balance(&state, &token, &carol), 60);
    assert_eq!(token_balance(&state, &token, &alice), 900);
    let fixed = GAS_BATCH_BASE + 2 * GAS_PER_BATCH_TRANSFER + 2 * GAS_PER_BATCH_CONTRACT_CALL_STEP;
    assert!(
        out.gas_used > fixed,
        "includes interpreter gas of the token calls"
    );
}

#[test]
fn failing_step_reverts_the_whole_batch_in_block_execution() {
    let (_vm, mut state, alice, token) = setup();
    let bob = acct(0xb0);
    let before = state.get(&alice).unwrap().balance;
    let batch = tx(
        alice,
        2,
        TransactionPayload::Batch {
            calls: vec![
                BatchCall::Transfer {
                    to: bob,
                    amount: 100,
                },
                BatchCall::ContractCall {
                    contract: token,
                    calldata: encode_transfer_calldata(&bob, 40),
                },
                // Fails: more BOING than alice holds.
                BatchCall::Transfer {
                    to: bob,
                    amount: u128::MAX / 2,
                },
            ],
        },
    );
    let exec = BlockExecutor::new();
    let (_gas, receipts) = exec
        .execute_block(11, 1_700_000_001, &[batch], &mut state, acct(0xfe))
        .unwrap();
    assert!(!receipts[0].success);
    let err = receipts[0].error.clone().unwrap_or_default();
    assert!(
        err.contains("step 2"),
        "error names the failing step: {err}"
    );
    assert!(state.get(&bob).is_none(), "no BOING moved");
    assert_eq!(token_balance(&state, &token, &bob), 0, "no token moved");
    // Security review #3: the failed batch pays the failed-tx fee once (all steps reverted).
    let fee = boing_execution::failed_tx_fee_amount(exec.fee_policy());
    assert_eq!(state.get(&alice).unwrap().balance, before - fee);
    assert_eq!(receipts[0].gas_used, boing_execution::FAILED_TX_GAS);
    assert_eq!(
        state.get(&alice).unwrap().nonce,
        3,
        "failed tx still consumes the nonce"
    );
}

#[test]
fn missing_contract_step_fails_batch() {
    let (vm, mut state, alice, _token) = setup();
    let batch = tx(
        alice,
        2,
        TransactionPayload::Batch {
            calls: vec![
                BatchCall::Transfer {
                    to: acct(0xb0),
                    amount: 1,
                },
                BatchCall::ContractCall {
                    contract: acct(0xdd),
                    calldata: vec![],
                },
            ],
        },
    );
    let err = vm
        .execute_with_context(&batch, &mut state, CTX)
        .unwrap_err();
    assert!(
        matches!(err, VmError::BatchStepFailed { index: 1, .. }),
        "{err:?}"
    );
}

#[test]
fn successful_batch_charges_fee_once_in_block() {
    let (_vm, mut state, alice, _token) = setup();
    let before = state.get(&alice).unwrap().balance;
    let calls: Vec<BatchCall> = (0..MAX_BATCH_CALLS as u8)
        .map(|i| BatchCall::Transfer {
            to: acct(0x10 + i),
            amount: 10,
        })
        .collect();
    let batch = tx(alice, 2, TransactionPayload::Batch { calls });
    let exec = BlockExecutor::new();
    let (gas, receipts) = exec
        .execute_block(
            11,
            1_700_000_001,
            std::slice::from_ref(&batch),
            &mut state,
            acct(0xfe),
        )
        .unwrap();
    assert!(receipts[0].success, "{:?}", receipts[0].error);
    assert_eq!(
        gas,
        GAS_BATCH_BASE + MAX_BATCH_CALLS as u64 * GAS_PER_BATCH_TRANSFER
    );
    let fee = network_fee_amount(gas, &batch.payload, exec.fee_policy());
    let separate = MAX_BATCH_CALLS as u128
        * network_fee_amount(
            21_000,
            &TransactionPayload::Transfer {
                to: acct(1),
                amount: 10,
            },
            exec.fee_policy(),
        );
    assert!(
        fee < separate,
        "batch fee {fee} must undercut {separate} for separate txs"
    );
    assert_eq!(
        state.get(&alice).unwrap().balance,
        before - 10 * MAX_BATCH_CALLS as u128 - fee
    );
}

#[test]
fn transfer_bps_fee_counts_every_batch_transfer() {
    let mut policy = NetworkFeePolicy::treasury_default();
    policy.transfer_amount_bps = 100; // 1%
    let p = TransactionPayload::Batch {
        calls: vec![
            BatchCall::Transfer {
                to: acct(1),
                amount: 10_000,
            },
            BatchCall::Transfer {
                to: acct(2),
                amount: 10_000,
            },
        ],
    };
    let single = network_fee_amount(
        0,
        &TransactionPayload::Transfer {
            to: acct(1),
            amount: 20_000,
        },
        &policy,
    );
    assert_eq!(network_fee_amount(0, &p, &policy), single);
}

#[test]
fn batch_shape_limits() {
    assert_eq!(validate_batch_calls(&[]), Err(BatchValidationError::Empty));
    let too_many = vec![
        BatchCall::Transfer {
            to: acct(1),
            amount: 1
        };
        MAX_BATCH_CALLS + 1
    ];
    assert!(matches!(
        validate_batch_calls(&too_many),
        Err(BatchValidationError::TooManyCalls { .. })
    ));
    let big = vec![BatchCall::ContractCall {
        contract: acct(1),
        calldata: vec![0; MAX_BATCH_CALLDATA_BYTES + 1],
    }];
    assert!(matches!(
        validate_batch_calls(&big),
        Err(BatchValidationError::CalldataTooLarge { .. })
    ));
    let overflow = vec![
        BatchCall::Transfer {
            to: acct(1),
            amount: u128::MAX,
        },
        BatchCall::Transfer {
            to: acct(2),
            amount: 1,
        },
    ];
    assert_eq!(
        validate_batch_calls(&overflow),
        Err(BatchValidationError::AmountOverflow)
    );

    // VM enforces the same limits (blocks from a malicious proposer).
    let (vm, mut state, alice, _) = setup();
    let err = vm
        .execute_with_context(
            &tx(alice, 2, TransactionPayload::Batch { calls: vec![] }),
            &mut state,
            CTX,
        )
        .unwrap_err();
    assert!(matches!(err, VmError::InvalidBatch(_)));
}

#[test]
fn bincode_layout_is_backward_compatible() {
    // Existing variants keep their discriminants; Batch is appended as index 9.
    let t = bincode::serialize(&TransactionPayload::Transfer {
        to: acct(1),
        amount: 5,
    })
    .unwrap();
    assert_eq!(&t[..4], &0u32.to_le_bytes());
    let v = bincode::serialize(&TransactionPayload::ClaimUnbond).unwrap();
    assert_eq!(&v[..4], &7u32.to_le_bytes());
    let b = TransactionPayload::Batch {
        calls: vec![
            BatchCall::Transfer {
                to: acct(2),
                amount: 7,
            },
            BatchCall::ContractCall {
                contract: acct(3),
                calldata: vec![0xab, 0xcd],
            },
        ],
    };
    let enc = bincode::serialize(&b).unwrap();
    let mut want = Vec::new();
    want.extend_from_slice(&9u32.to_le_bytes());
    want.extend_from_slice(&2u64.to_le_bytes());
    want.extend_from_slice(&0u32.to_le_bytes());
    want.extend_from_slice(&[2u8; 32]);
    want.extend_from_slice(&7u128.to_le_bytes());
    want.extend_from_slice(&1u32.to_le_bytes());
    want.extend_from_slice(&[3u8; 32]);
    want.extend_from_slice(&2u64.to_le_bytes());
    want.extend_from_slice(&[0xab, 0xcd]);
    assert_eq!(
        enc, want,
        "layout mirrored by boing-sdk encodeTransactionPayload({{kind:'batch'}})"
    );
    let back: TransactionPayload = bincode::deserialize(&enc).unwrap();
    assert_eq!(back, b);
}

#[test]
fn access_list_and_signing_display_cover_every_step() {
    let alice = acct(0xa1);
    let t = tx(
        alice,
        0,
        TransactionPayload::Batch {
            calls: vec![
                BatchCall::Transfer {
                    to: acct(2),
                    amount: 7,
                },
                BatchCall::Transfer {
                    to: acct(2),
                    amount: 8,
                },
                BatchCall::ContractCall {
                    contract: acct(3),
                    calldata: vec![],
                },
            ],
        },
    );
    let ids: Vec<_> = t.access_list.all().copied().collect();
    for id in [alice, acct(2), acct(3)] {
        assert!(ids.contains(&id));
    }
    assert!(t.access_list_covers_parallel_suggestion());
    let d = t.display_for_signing();
    assert!(d.contains("Batch of 3 (atomic)"), "{d}");
}
