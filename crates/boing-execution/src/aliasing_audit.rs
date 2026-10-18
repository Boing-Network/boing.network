//! Aliasing audit (`src == dst`, `caller == recipient`, contract-as-party) across every token-like
//! template. These tests **pin current behaviour** of deployed bytecode (nothing here changes any
//! template). See `docs/TOKEN-ALIASING-AUDIT.md`.

#![cfg(test)]

use boing_primitives::{Account, AccountId};
use boing_state::StateStore;

use crate::interpreter::{Interpreter, VmExecutionContext};
use crate::native_amm_lp_vault::*;
use crate::native_lp_share_token::*;
use crate::reference_fungible_secured::*;
use crate::reference_nft::*;
use crate::reference_token::*;
use crate::reference_token_v2::*;
use crate::vm::Vm;

fn xor(a: &AccountId, x: &[u8; 32]) -> [u8; 32] {
    let mut k = [0u8; 32];
    for i in 0..32 {
        k[i] = a.0[i] ^ x[i];
    }
    k
}
fn tail(w: &[u8; 32]) -> u128 {
    u128::from_be_bytes(w[16..32].try_into().unwrap())
}
fn setup(ids: &[AccountId], code: &[(AccountId, Vec<u8>)]) -> StateStore {
    let mut st = StateStore::new();
    for id in ids {
        st.insert(Account {
            id: *id,
            state: Default::default(),
        });
    }
    for (id, c) in code {
        st.set_contract_code(*id, c.clone());
    }
    st
}
/// Runs like a transaction: a VM fault reverts all state. Returns `true` on success.
fn call(st: &mut StateStore, who: AccountId, to: AccountId, cd: &[u8]) -> bool {
    let cp = st.checkpoint();
    let code = st.get_contract_code(&to).unwrap().clone();
    let mut it = Interpreter::new(code, 30_000_000);
    match it.run(who, to, cd, st) {
        Ok(_) => true,
        Err(_) => {
            st.revert(cp);
            false
        }
    }
}

const ALICE: AccountId = AccountId([0xa1; 32]);
const BOB: AccountId = AccountId([0xb2; 32]);
const T: AccountId = AccountId([0x7e; 32]);

#[test]
fn reference_token_v1_self_transfer_doubles_balance_pinned_bug() {
    let mut st = setup(&[ALICE, T], &[(T, reference_fungible_template_bytecode())]);
    let bal =
        |s: &StateStore| tail(&s.get_contract_storage(&T, &xor(&ALICE, &REF_FUNGIBLE_BALANCE_XOR)));
    assert!(call(
        &mut st,
        ALICE,
        T,
        &encode_mint_first_calldata(&ALICE, 100)
    ));
    assert!(call(
        &mut st,
        ALICE,
        T,
        &encode_transfer_calldata(&ALICE, 100)
    ));
    assert_eq!(
        bal(&st),
        200,
        "KNOWN BUG (v1): self-transfer credits without debit"
    );
    // Repeating compounds it.
    assert!(call(
        &mut st,
        ALICE,
        T,
        &encode_transfer_calldata(&ALICE, 200)
    ));
    assert_eq!(bal(&st), 400);
}

#[test]
fn reference_token_v2_self_transfer_and_transfer_from_aliasing_are_safe() {
    let mut st = setup(
        &[ALICE, BOB, T],
        &[(T, reference_fungible_template_v2_bytecode())],
    );
    let bal = |s: &StateStore, a: &AccountId| {
        tail(&s.get_contract_storage(&T, &xor(a, &REF_FUNGIBLE_BALANCE_XOR)))
    };
    assert!(call(
        &mut st,
        ALICE,
        T,
        &encode_mint_first_calldata(&ALICE, 100)
    ));
    call(&mut st, ALICE, T, &encode_transfer_calldata(&ALICE, 100));
    assert_eq!(bal(&st, &ALICE), 100);
    // owner == spender: approve self, transfer_from(self -> self) must not mint.
    assert!(call(
        &mut st,
        ALICE,
        T,
        &encode_approve_calldata(&ALICE, 100)
    ));
    call(
        &mut st,
        ALICE,
        T,
        &encode_transfer_from_calldata(&ALICE, &ALICE, 100),
    );
    assert_eq!(bal(&st, &ALICE), 100);
    // spender pulling to itself (to == caller): ordinary move, conserved.
    assert!(call(&mut st, ALICE, T, &encode_approve_calldata(&BOB, 30)));
    call(
        &mut st,
        BOB,
        T,
        &encode_transfer_from_calldata(&ALICE, &BOB, 30),
    );
    assert_eq!((bal(&st, &ALICE), bal(&st, &BOB)), (70, 30));
    // Transfer to the token contract itself: conserved (tokens are stuck, not created).
    call(&mut st, ALICE, T, &encode_transfer_calldata(&T, 10));
    assert_eq!(bal(&st, &ALICE) + bal(&st, &BOB) + bal(&st, &T), 100);
}

fn secured_token(st: &mut StateStore, deployer: AccountId, bytecode: Vec<u8>) -> AccountId {
    use boing_primitives::{AccessList, Transaction, TransactionPayload};
    st.insert(Account {
        id: deployer,
        state: boing_primitives::AccountState {
            balance: 10_000_000,
            ..Default::default()
        },
    });
    let ctx = VmExecutionContext {
        block_height: 10,
        block_timestamp: 1_700_000_000,
        native_value: false,
    };
    let vm = Vm::new();
    let mk = |nonce, payload| Transaction {
        nonce,
        sender: deployer,
        payload,
        access_list: AccessList::default(),
    };
    vm.execute_with_context(
        &mk(
            0,
            TransactionPayload::ContractDeployWithPurposeAndMetadata {
                bytecode,
                purpose_category: "token".into(),
                description_hash: None,
                asset_name: None,
                asset_symbol: None,
                create2_salt: None,
            },
        ),
        st,
        ctx,
    )
    .unwrap();
    let token = boing_primitives::nonce_derived_contract_address(&deployer, 0);
    vm.execute_with_context(
        &mk(
            1,
            TransactionPayload::ContractCall {
                contract: token,
                calldata: encode_mint_first_calldata(&deployer, 1_000),
            },
        ),
        st,
        ctx,
    )
    .unwrap();
    let r = vm.execute_with_context(
        &mk(
            2,
            TransactionPayload::ContractCall {
                contract: token,
                calldata: encode_transfer_calldata(&deployer, 100),
            },
        ),
        st,
        ctx,
    );
    assert!(r.is_ok(), "{r:?}");
    token
}

#[test]
fn secured_template_v1_self_transfer_doubles_pinned_bug_and_v2_is_safe() {
    let deployer = AccountId([0x5e; 32]);
    let mut st = StateStore::new();
    let token = secured_token(
        &mut st,
        deployer,
        reference_fungible_secured_pinned_default_deploy_bytecode(),
    );
    let b = tail(&st.get_contract_storage(&token, &xor(&deployer, &REF_SECURED_BALANCE_XOR)));
    assert_eq!(
        b, 1_100,
        "KNOWN BUG (secured v1): self-transfer of 100 credits +100"
    );

    let mut st = StateStore::new();
    let token = secured_token(
        &mut st,
        deployer,
        reference_fungible_secured_pinned_default_deploy_bytecode_v2(),
    );
    let b = tail(&st.get_contract_storage(&token, &xor(&deployer, &REF_SECURED_BALANCE_XOR)));
    assert_eq!(b, 1_000, "secured v2: self-transfer is a no-op");
}

#[test]
fn secured_v2_differs_from_v1_only_by_the_guard_and_passes_qa() {
    use boing_qa::{check_contract_deploy_full, QaResult, RuleRegistry};
    let (v1, v2) = (
        reference_fungible_secured_runtime_bytecode(),
        reference_fungible_secured_runtime_bytecode_v2(),
    );
    assert_ne!(v1, v2);
    assert!(v2.len() > v1.len() && v2.len() - v1.len() < 120);
    let r = check_contract_deploy_full(
        &reference_fungible_secured_pinned_default_deploy_bytecode_v2(),
        Some("token"),
        None,
        &RuleRegistry::new(),
    );
    assert!(matches!(r, QaResult::Allow), "{r:?}");
}

fn lp_share_self_transfer(code: Vec<u8>) -> (bool, u128, u128) {
    let minter = AccountId([0xc3; 32]);
    let mut st = setup(&[ALICE, BOB, minter, T], &[(T, code)]);
    assert!(call(
        &mut st,
        ALICE,
        T,
        &encode_lp_share_set_minter_once_calldata(&minter)
    ));
    assert!(call(
        &mut st,
        minter,
        T,
        &encode_lp_share_mint_calldata(&ALICE, 1_000)
    ));
    let ok = call(
        &mut st,
        ALICE,
        T,
        &encode_lp_share_transfer_calldata(&ALICE, 400),
    );
    let a = tail(&st.get_contract_storage(&T, &xor(&ALICE, &LP_SHARE_BALANCE_XOR)));
    // Normal transfers still work.
    assert!(call(
        &mut st,
        ALICE,
        T,
        &encode_lp_share_transfer_calldata(&BOB, 100)
    ));
    let b = tail(&st.get_contract_storage(&T, &xor(&BOB, &LP_SHARE_BALANCE_XOR)));
    (ok, a, b)
}

#[test]
fn lp_share_v1_self_transfer_doubles_pinned_bug_and_v2_is_safe() {
    let (_, a, b) = lp_share_self_transfer(lp_share_token_bytecode());
    assert_eq!(
        (a, b),
        (1_400, 100),
        "KNOWN BUG (lp share v1): self-transfer credits without debit"
    );
    let (ok, a, b) = lp_share_self_transfer(lp_share_token_bytecode_v2());
    assert!(ok);
    assert_eq!(
        (a, b),
        (1_000, 100),
        "lp share v2: self-transfer is a no-op"
    );
    use boing_qa::{check_contract_deploy_full, QaResult, RuleRegistry};
    let r = check_contract_deploy_full(
        &lp_share_token_bytecode_v2(),
        Some("dapp"),
        None,
        &RuleRegistry::new(),
    );
    assert!(matches!(r, QaResult::Allow), "{r:?}");
}

#[test]
fn reference_nft_self_and_foreign_transfers_are_safe() {
    let mut st = setup(
        &[ALICE, BOB, T],
        &[(T, reference_nft_collection_template_bytecode())],
    );
    let tid = [7u8; 32];
    let owner = |s: &StateStore| {
        AccountId(s.get_contract_storage(&T, &xor(&AccountId(tid), &REF_NFT_OWNER_STORAGE_XOR)))
    };
    assert!(call(&mut st, ALICE, T, &encode_owner_of_calldata(&tid))); // lazy admin = ALICE
    assert!(call(
        &mut st,
        ALICE,
        T,
        &encode_transfer_nft_calldata(&BOB, &tid)
    ));
    assert_eq!(owner(&st), BOB);
    call(&mut st, BOB, T, &encode_transfer_nft_calldata(&BOB, &tid));
    assert_eq!(owner(&st), BOB, "self-transfer keeps the single owner slot");
    // A non-owner (not admin) cannot move it.
    call(
        &mut st,
        AccountId([0xcc; 32]),
        T,
        &encode_transfer_nft_calldata(&AccountId([0xcc; 32]), &tid),
    );
    assert_eq!(owner(&st), BOB);
    // The collection admin cannot take a token another account owns.
    call(
        &mut st,
        ALICE,
        T,
        &encode_transfer_nft_calldata(&ALICE, &tid),
    );
    assert_eq!(owner(&st), BOB);
}

#[test]
fn lp_vault_aliasing_is_safe() {
    use crate::native_amm::{
        constant_product_pool_bytecode, encode_add_liquidity_calldata, lp_balance_storage_key,
    };
    let (pool, vault, share) = (
        AccountId([0xb1; 32]),
        AccountId([0xb3; 32]),
        AccountId([0xb4; 32]),
    );
    // Normal deposit: shares == LP credited to the vault.
    let mut st = setup(
        &[ALICE, pool, vault, share],
        &[
            (pool, constant_product_pool_bytecode()),
            (vault, native_amm_lp_vault_bytecode()),
            (share, lp_share_token_bytecode()),
        ],
    );
    assert!(call(
        &mut st,
        ALICE,
        vault,
        &encode_native_amm_lp_vault_configure_calldata(&pool, &share)
    ));
    assert!(call(
        &mut st,
        ALICE,
        share,
        &encode_lp_share_set_minter_once_calldata(&vault)
    ));
    let inner = encode_add_liquidity_calldata(1_000, 2_000, 0);
    assert!(call(
        &mut st,
        ALICE,
        vault,
        &encode_native_amm_lp_vault_deposit_add_calldata(&inner, 0)
    ));
    let lp = tail(&st.get_contract_storage(&pool, &lp_balance_storage_key(&vault.0)));
    let sh = tail(&st.get_contract_storage(&share, &xor(&ALICE, &LP_SHARE_BALANCE_XOR)));
    assert_eq!(lp, sh);
    // A vault configured with itself as the pool (operator mistake) must not mint shares.
    let v2 = AccountId([0xb5; 32]);
    st.insert(Account {
        id: v2,
        state: Default::default(),
    });
    st.set_contract_code(v2, native_amm_lp_vault_bytecode());
    assert!(call(
        &mut st,
        ALICE,
        v2,
        &encode_native_amm_lp_vault_configure_calldata(&v2, &share)
    ));
    call(
        &mut st,
        ALICE,
        v2,
        &encode_native_amm_lp_vault_deposit_add_calldata(&inner, 0),
    );
    let sh2 = tail(&st.get_contract_storage(&share, &xor(&ALICE, &LP_SHARE_BALANCE_XOR)));
    assert_eq!(sh2, sh, "a vault pointed at itself must not mint shares");
}

#[test]
fn pool_v8_recipient_is_pool_or_token_aliasing_cannot_create_value() {
    use crate::native_amm::{encode_set_tokens_calldata, reserve_a_key, reserve_b_key};
    use crate::native_amm_v8::*;
    let (ta, tb, pool) = (
        AccountId([0x0a; 32]),
        AccountId([0x0b; 32]),
        AccountId([0x90; 32]),
    );
    let tok = reference_fungible_template_v2_bytecode();
    let mut st = setup(
        &[ALICE, ta, tb, pool],
        &[
            (ta, tok.clone()),
            (tb, tok),
            (pool, constant_product_pool_bytecode_v8()),
        ],
    );
    let bal = |s: &StateStore, t: &AccountId, w: &AccountId| {
        tail(&s.get_contract_storage(t, &xor(w, &REF_FUNGIBLE_BALANCE_XOR)))
    };
    for t in [ta, tb] {
        assert!(call(
            &mut st,
            ALICE,
            t,
            &encode_mint_first_calldata(&ALICE, 1_000_000)
        ));
        assert!(call(
            &mut st,
            ALICE,
            t,
            &encode_approve_calldata(&pool, 1_000_000)
        ));
    }
    // set_tokens(pool, pool) / (ta, ta) are rejected.
    assert!(!call(
        &mut st,
        ALICE,
        pool,
        &encode_set_tokens_calldata(&ta, &ta)
    ));
    assert!(call(
        &mut st,
        ALICE,
        pool,
        &encode_set_tokens_calldata(&ta, &tb)
    ));
    assert!(call(
        &mut st,
        ALICE,
        pool,
        &encode_v8_add_liquidity_calldata(100_000, 100_000, 1, 0)
    ));
    // swap_to with recipient == pool: the payout is a self-transfer (v2 no-op), so the pool
    // keeps the tokens; reserves never exceed balances.
    assert!(call(
        &mut st,
        ALICE,
        pool,
        &encode_v8_swap_to_calldata(0, 1_000, 1, &pool, 0)
    ));
    let ra = tail(&st.get_contract_storage(&pool, &reserve_a_key()));
    let rb = tail(&st.get_contract_storage(&pool, &reserve_b_key()));
    assert!(bal(&st, &ta, &pool) >= ra && bal(&st, &tb, &pool) >= rb);
    // Alice cannot get more than she put in by routing the payout to a token contract either.
    let before = bal(&st, &ta, &ALICE) + bal(&st, &tb, &ALICE);
    call(
        &mut st,
        ALICE,
        pool,
        &encode_v8_swap_to_calldata(1, 1_000, 1, &ta, 0),
    );
    assert!(bal(&st, &ta, &ALICE) + bal(&st, &tb, &ALICE) <= before);
}
