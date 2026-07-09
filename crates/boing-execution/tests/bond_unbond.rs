//! Tests for Bond, Unbond (delayed), and ClaimUnbond staking transactions.

use boing_execution::{Vm, VmError, VmExecutionContext};
use boing_primitives::{AccessList, Account, AccountId, AccountState, Transaction, TransactionPayload};
use boing_state::StateStore;
use boing_tokenomics::UNBONDING_DELAY_BLOCKS;

#[test]
fn test_bond_unbond_claim_with_delay() {
    let vm = Vm::new();
    let a = AccountId::from_bytes([1u8; 32]);
    let mut state = StateStore::new();
    state.insert(Account {
        id: a,
        state: AccountState {
            balance: 1000,
            nonce: 0,
            stake: 0,
            ..Default::default()
        },
    });

    let bond_tx = Transaction {
        nonce: 0,
        sender: a,
        payload: TransactionPayload::Bond { amount: 300 },
        access_list: AccessList::new(vec![a], vec![a]),
    };
    vm.execute(&bond_tx, &mut state).unwrap();
    assert_eq!(state.get(&a).unwrap().balance, 700);
    assert_eq!(state.get(&a).unwrap().stake, 300);
    assert_eq!(state.get(&a).unwrap().nonce, 1);

    let unbond_height = 10u64;
    let unbond_tx = Transaction {
        nonce: 1,
        sender: a,
        payload: TransactionPayload::Unbond { amount: 100 },
        access_list: AccessList::new(vec![a], vec![a]),
    };
    vm.execute_with_context(
        &unbond_tx,
        &mut state,
        VmExecutionContext {
            block_height: unbond_height,
            block_timestamp: 0,
        },
    )
    .unwrap();
    let st = state.get(&a).unwrap();
    assert_eq!(st.balance, 700);
    assert_eq!(st.stake, 200);
    assert_eq!(st.pending_unbond, 100);
    assert_eq!(
        st.unbond_unlock_height,
        unbond_height + UNBONDING_DELAY_BLOCKS
    );

    let claim_early = Transaction {
        nonce: 2,
        sender: a,
        payload: TransactionPayload::ClaimUnbond,
        access_list: AccessList::new(vec![a], vec![a]),
    };
    let err = vm
        .execute_with_context(
            &claim_early,
            &mut state,
            VmExecutionContext {
                block_height: unbond_height + UNBONDING_DELAY_BLOCKS - 1,
                block_timestamp: 0,
            },
        )
        .unwrap_err();
    assert!(matches!(err, VmError::UnbondNotMature { .. }));

    let claim = Transaction {
        nonce: 2,
        sender: a,
        payload: TransactionPayload::ClaimUnbond,
        access_list: AccessList::new(vec![a], vec![a]),
    };
    vm.execute_with_context(
        &claim,
        &mut state,
        VmExecutionContext {
            block_height: unbond_height + UNBONDING_DELAY_BLOCKS,
            block_timestamp: 0,
        },
    )
    .unwrap();
    let st = state.get(&a).unwrap();
    assert_eq!(st.balance, 800);
    assert_eq!(st.stake, 200);
    assert_eq!(st.pending_unbond, 0);
    assert_eq!(st.unbond_unlock_height, 0);
}
