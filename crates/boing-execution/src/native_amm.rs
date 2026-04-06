//! Minimal **constant-product pool** bytecode (in-contract reserves) + calldata encoders.
//!
//! Matches `docs/NATIVE-AMM-CALLDATA.md`. Pool reserves and swap amounts are **u64** in the ledger
//! (reference-style u128 words with value in the low **16** bytes). The VM **`Mul`** opcode is
//! **256×256 → 256** (low limb; see `docs/TECHNICAL-SPECIFICATION.md` §7.2), so fee and CP steps are
//! exact at full word range, not low-**64** truncation.
//! **Swap** applies [`NATIVE_CP_SWAP_FEE_BPS`] on the CP **output** (see calldata doc § Swap fee).
//! Successful **`swap` / `add_liquidity` / `remove_liquidity`** each emit **`Log2`**: fixed **topic0**
//! (`NATIVE_AMM_TOPIC_SWAP` and siblings), **topic1 = caller**, **data** = three 32-byte words (see calldata doc § Logs).
//!
//! **v1** [`constant_product_pool_bytecode`] is ledger-only. **v2** [`constant_product_pool_bytecode_v2`] adds
//! one-time **`set_tokens`** plus optional reference-token **`CALL`** on swap output and remove-liquidity payouts
//! (see `docs/NATIVE-AMM-CALLDATA.md` § v2). **v5** [`constant_product_pool_bytecode_v5`] is v4 plus **`swap_to` (`0x15`)**:
//! 160-byte swap calldata with an explicit **output recipient** for token payouts (router-safe; still Boing VM only).
//! The VM **`Call` (`0xf1`)** semantics: `docs/TECHNICAL-SPECIFICATION.md` §7.2.

use boing_primitives::AccountId;

use crate::bytecode::Opcode;
use crate::reference_token::{amount_word, selector_word, SELECTOR_TRANSFER};

/// `swap` — word1 = direction (0 = A→B, 1 = B→A), word2 = amount_in, word3 = min_out.
pub const SELECTOR_SWAP: u8 = 0x10;
/// `add_liquidity` — word1 = amount_a, word2 = amount_b, word3 = min_liquidity (ignored in MVP).
pub const SELECTOR_ADD_LIQUIDITY: u8 = 0x11;
/// `remove_liquidity` — burn LP pro-rata; `min_a` / `min_b` slippage guards.
pub const SELECTOR_REMOVE_LIQUIDITY: u8 = 0x12;
/// **v2 only:** one-time `set_tokens(token_a, token_b)` — 96-byte calldata (selector + two 32-byte ids; zero = no token for that side).
pub const SELECTOR_SET_TOKENS: u8 = 0x13;
/// **v3/v4 only:** `set_swap_fee_bps(fee)` — **64-byte** calldata (selector + one amount word). **Only** when **total LP supply == 0**; **`fee`** must satisfy **1 ≤ fee ≤ 10_000**.
pub const SELECTOR_SET_SWAP_FEE_BPS: u8 = 0x14;
/// **v5 only (token-hook pools):** `swap_to` — same as [`SELECTOR_SWAP`] plus **word4** = output recipient `AccountId` (**160-byte** calldata). Reference-token `transfer` uses that account instead of [`Opcode::Caller`].
pub const SELECTOR_SWAP_TO: u8 = 0x15;
/// **v5 only:** `remove_liquidity_to` — same as [`SELECTOR_REMOVE_LIQUIDITY`] plus **word4** / **word5** = payout recipients for token A / B (**192-byte** calldata).
pub const SELECTOR_REMOVE_LIQUIDITY_TO: u8 = 0x16;

/// XOR mask for per-signer LP balance slot: `storage_key = caller_id ^ LP_BALANCE_STORAGE_XOR`.
pub const LP_BALANCE_STORAGE_XOR: [u8; 32] = *b"BOING_NATIVEAMM_LPRV1\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

/// CREATE2 salt (`create2_salt: Some(...)`) for a **canonical** native constant-product pool (v1).
///
/// Pool `AccountId` = [`boing_primitives::create2_contract_address`] `(deployer, NATIVE_CP_POOL_CREATE2_SALT_V1, constant_product_pool_bytecode())`.
/// Label is UTF-8 left-padded with zeros (same style as log topic strings).
pub const NATIVE_CP_POOL_CREATE2_SALT_V1: [u8; 32] =
    *b"BOING_NATIVECP_C2V1\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

/// CREATE2 salt for **v2** pool bytecode ([`constant_product_pool_bytecode_v2`] — token `CALL` hooks).
pub const NATIVE_CP_POOL_CREATE2_SALT_V2: [u8; 32] =
    *b"BOING_NATIVECP_C2V2\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

/// CREATE2 salt for **v3** — ledger-only + on-chain [`swap_fee_bps_key`] (see [`constant_product_pool_bytecode_v3`]).
pub const NATIVE_CP_POOL_CREATE2_SALT_V3: [u8; 32] =
    *b"BOING_NATIVECP_C2V3\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

/// CREATE2 salt for **v4** — **v2** token hooks + configurable swap fee ([`constant_product_pool_bytecode_v4`]).
pub const NATIVE_CP_POOL_CREATE2_SALT_V4: [u8; 32] =
    *b"BOING_NATIVECP_C2V4\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

/// CREATE2 salt for **v5** — v4 + [`SELECTOR_SWAP_TO`] ([`constant_product_pool_bytecode_v5`]).
pub const NATIVE_CP_POOL_CREATE2_SALT_V5: [u8; 32] =
    *b"BOING_NATIVECP_C2V5\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

/// `Log2` **topic0** after a successful **`swap`** (data: direction, `amount_in`, `amount_out` words).
pub const NATIVE_AMM_TOPIC_SWAP: [u8; 32] = *b"BOING_NATIVEAMM_SWAP_V1\x00\x00\x00\x00\x00\x00\x00\x00\x00";
/// `Log2` **topic0** after **`add_liquidity`** (data: `amount_a`, `amount_b`, `lp_mint` words).
pub const NATIVE_AMM_TOPIC_ADD_LIQUIDITY: [u8; 32] =
    *b"BOING_NATIVEAMM_ADDLP_V1\x00\x00\x00\x00\x00\x00\x00\x00";
/// `Log2` **topic0** after **`remove_liquidity`** (data: `burn`, `amount_a_out`, `amount_b_out` words).
pub const NATIVE_AMM_TOPIC_REMOVE_LIQUIDITY: [u8; 32] =
    *b"BOING_NATIVEAMM_RMLP_V1\x00\x00\x00\x00\x00\x00\x00\x00\x00";

/// Storage key for reserve A.
#[must_use]
pub fn reserve_a_key() -> [u8; 32] {
    let mut k = [0u8; 32];
    k[31] = 0x01;
    k
}

/// Storage key for reserve B.
#[must_use]
pub fn reserve_b_key() -> [u8; 32] {
    let mut k = [0u8; 32];
    k[31] = 0x02;
    k
}

/// Total LP supply key (`k[31] == 0x03`).
#[must_use]
pub fn total_lp_supply_key() -> [u8; 32] {
    let mut k = [0u8; 32];
    k[31] = 0x03;
    k
}

/// **v2:** Reference-token contract id for side A (`0` = ledger-only for payouts on this side).
#[must_use]
pub fn token_a_key() -> [u8; 32] {
    let mut k = [0u8; 32];
    k[31] = 0x04;
    k
}

/// **v2:** Reference-token contract id for side B.
#[must_use]
pub fn token_b_key() -> [u8; 32] {
    let mut k = [0u8; 32];
    k[31] = 0x05;
    k
}

/// **v2:** Non-zero after successful `set_tokens` (immutable).
#[must_use]
pub fn tokens_configured_key() -> [u8; 32] {
    let mut k = [0u8; 32];
    k[31] = 0x06;
    k
}

/// **v3/v4:** Swap fee in basis points on **output** (after CP step). **0** = unset → default **[`NATIVE_CP_SWAP_FEE_BPS`]** on first `add_liquidity`; **`set_swap_fee_bps`** writes **1..=10_000**.
#[must_use]
pub fn swap_fee_bps_key() -> [u8; 32] {
    let mut k = [0u8; 32];
    k[31] = 0x07;
    k
}

/// Derived storage key for `caller`'s LP balance in the pool contract.
#[must_use]
pub fn lp_balance_storage_key(caller: &[u8; 32]) -> [u8; 32] {
    let mut k = [0u8; 32];
    for i in 0..32 {
        k[i] = caller[i] ^ LP_BALANCE_STORAGE_XOR[i];
    }
    k
}

#[must_use]
pub fn word_u64(n: u64) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[24..32].copy_from_slice(&n.to_be_bytes());
    w
}

/// 128 bytes: `swap` calldata per NATIVE-AMM-CALLDATA.md.
#[must_use]
pub fn encode_swap_calldata(direction: u128, amount_in: u128, min_out: u128) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_SWAP).to_vec();
    v.extend_from_slice(&amount_word(direction));
    v.extend_from_slice(&amount_word(amount_in));
    v.extend_from_slice(&amount_word(min_out));
    debug_assert_eq!(v.len(), 128);
    v
}

/// **160 bytes:** `swap_to` calldata (v5 pool) — word4 = **`recipient`** for on-chain token output (`transfer` target).
#[must_use]
pub fn encode_swap_to_calldata(
    direction: u128,
    amount_in: u128,
    min_out: u128,
    recipient: &AccountId,
) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_SWAP_TO).to_vec();
    v.extend_from_slice(&amount_word(direction));
    v.extend_from_slice(&amount_word(amount_in));
    v.extend_from_slice(&amount_word(min_out));
    v.extend_from_slice(&recipient.0);
    debug_assert_eq!(v.len(), 160);
    v
}

/// **192 bytes:** `remove_liquidity_to` (v5) — explicit **recipient A** and **recipient B** for reference-token payouts.
#[must_use]
pub fn encode_remove_liquidity_to_calldata(
    liquidity_burn: u128,
    min_a: u128,
    min_b: u128,
    recipient_a: &AccountId,
    recipient_b: &AccountId,
) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_REMOVE_LIQUIDITY_TO).to_vec();
    v.extend_from_slice(&amount_word(liquidity_burn));
    v.extend_from_slice(&amount_word(min_a));
    v.extend_from_slice(&amount_word(min_b));
    v.extend_from_slice(&recipient_a.0);
    v.extend_from_slice(&recipient_b.0);
    debug_assert_eq!(v.len(), 192);
    v
}

/// 128 bytes: `add_liquidity`.
#[must_use]
pub fn encode_add_liquidity_calldata(amount_a: u128, amount_b: u128, min_liquidity: u128) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_ADD_LIQUIDITY).to_vec();
    v.extend_from_slice(&amount_word(amount_a));
    v.extend_from_slice(&amount_word(amount_b));
    v.extend_from_slice(&amount_word(min_liquidity));
    debug_assert_eq!(v.len(), 128);
    v
}

/// Decode **32-byte** successful `add_liquidity` return data: **LP minted** this call (**u128** in the
/// low **16** bytes, high **16** bytes must be zero — same layout as calldata amount words).
#[must_use]
pub fn decode_add_liquidity_return_lp_minted(return_data: &[u8]) -> Option<u128> {
    if return_data.len() != 32 {
        return None;
    }
    if return_data[0..16] != [0u8; 16] {
        return None;
    }
    Some(u128::from_be_bytes(return_data[16..32].try_into().ok()?))
}

/// 128 bytes: `remove_liquidity` — `liquidity_burn`, `min_a`, `min_b` (slippage).
#[must_use]
pub fn encode_remove_liquidity_calldata(liquidity_burn: u128, min_a: u128, min_b: u128) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_REMOVE_LIQUIDITY).to_vec();
    v.extend_from_slice(&amount_word(liquidity_burn));
    v.extend_from_slice(&amount_word(min_a));
    v.extend_from_slice(&amount_word(min_b));
    debug_assert_eq!(v.len(), 128);
    v
}

/// **v2:** 96 bytes — `set_tokens(token_a, token_b)` (`AccountId` words; all-zero side = no on-chain payout for that asset).
#[must_use]
pub fn encode_set_tokens_calldata(token_a: &AccountId, token_b: &AccountId) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_SET_TOKENS).to_vec();
    v.extend_from_slice(&token_a.0);
    v.extend_from_slice(&token_b.0);
    debug_assert_eq!(v.len(), 96);
    v
}

/// **v3/v4:** 64 bytes — `set_swap_fee_bps(fee)` with **1 ≤ fee ≤ 10_000** (only when **total LP == 0**).
#[must_use]
pub fn encode_set_swap_fee_bps_calldata(fee_bps: u128) -> Vec<u8> {
    assert!((1..=10_000).contains(&fee_bps), "fee_bps must be in 1..=10_000");
    let mut v = selector_word(SELECTOR_SET_SWAP_FEE_BPS).to_vec();
    v.extend_from_slice(&amount_word(fee_bps));
    debug_assert_eq!(v.len(), 64);
    v
}

/// Integer constant-product out (no fee): \( \Delta_{out} = \lfloor r_{out} \cdot \Delta_{in} / (r_{in} + \Delta_{in}) \rfloor \).
#[must_use]
pub const fn constant_product_amount_out(reserve_in: u64, reserve_out: u64, amount_in: u64) -> u64 {
    let rin = reserve_in as u128;
    let rout = reserve_out as u128;
    let dx = amount_in as u128;
    let denom = rin.saturating_add(dx);
    if denom == 0 {
        return 0;
    }
    let num = rout.saturating_mul(dx);
    (num / denom) as u64
}

/// Swap fee in basis points (on **output** after the no-fee CP step). **30** = 0.30 %. Matches pool bytecode.
pub const NATIVE_CP_SWAP_FEE_BPS: u16 = 30;

/// Amount out after [`NATIVE_CP_SWAP_FEE_BPS`] is applied: \( \lfloor \Delta_{out,\text{raw}} \cdot (10^4 - \text{fee}) / 10^4 \rfloor \).
/// **Output-side** fee keeps `r_in * fee` off the hot path; **Mul** in the VM is full **256-bit** so
/// `dy * (10^4 - fee)` is not truncated to **64** bits. See `docs/NATIVE-AMM-CALLDATA.md` § Swap fee.
#[must_use]
pub const fn constant_product_amount_out_after_fee(
    reserve_in: u64,
    reserve_out: u64,
    amount_in: u64,
) -> u64 {
    constant_product_amount_out_after_fee_with_bps(reserve_in, reserve_out, amount_in, NATIVE_CP_SWAP_FEE_BPS)
}

/// Same as [`constant_product_amount_out_after_fee`] but with an explicit **`fee_bps`** (**0..=10_000**).
#[must_use]
pub const fn constant_product_amount_out_after_fee_with_bps(
    reserve_in: u64,
    reserve_out: u64,
    amount_in: u64,
    fee_bps: u16,
) -> u64 {
    let dy = constant_product_amount_out(reserve_in, reserve_out, amount_in);
    let keep = 10_000u64 - fee_bps as u64;
    ((dy as u128 * keep as u128) / 10_000u128) as u64
}

fn push32(code: &mut Vec<u8>, w: &[u8; 32]) {
    code.push(Opcode::Push32 as u8);
    code.extend_from_slice(w);
}

fn patch_push32_dest(code: &mut [u8], push32_opcode_at: usize, dest: usize) {
    code[push32_opcode_at + 1..push32_opcode_at + 33].copy_from_slice(&word_u64(dest as u64));
}

/// Scratch for **v3/v4** swap fee (`SLoad` / `keep`); below v2 `CALL` scratch (**1152**).
const MEM_NATIVE_AMM_FEE_BPS: u64 = 1024;
const MEM_NATIVE_AMM_FEE_KEEP: u64 = 1056;

fn append_mstore_word(code: &mut Vec<u8>, mem_off: u64, w: &[u8; 32]) {
    push32(code, w);
    push32(code, &word_u64(mem_off));
    code.push(Opcode::MStore as u8);
}

/// **v2 / v5:** After swap math, `CALL` output reference token with `transfer(to, dy)` if configured.
/// `recipient_mem`: **`Some(offset)`** — load 32-byte `to` from memory (v5 `swap_to`); **`None`** — use [`Opcode::Caller`] (legacy `swap`).
fn append_v2_swap_output_reference_call(
    code: &mut Vec<u8>,
    mem_dir: u64,
    mem_dy: u64,
    cd_base: u64,
    tmp_tok: u64,
    recipient_mem: Option<u64>,
) {
    // Pick token id: dir==1 (B→A) → token A, else → token B.
    push32(code, &word_u64(mem_dir));
    code.push(Opcode::MLoad as u8);
    push32(code, &amount_word(1));
    code.push(Opcode::Eq as u8);
    let fix_pick_a = code.len();
    push32(code, &[0u8; 32]);
    code.push(Opcode::JumpI as u8);
    push32(code, &token_b_key());
    code.push(Opcode::SLoad as u8);
    let fix_after_pick = code.len();
    push32(code, &[0u8; 32]);
    code.push(Opcode::Jump as u8);
    let off_pick_a = code.len();
    patch_push32_dest(code, fix_pick_a, off_pick_a);
    push32(code, &token_a_key());
    code.push(Opcode::SLoad as u8);
    let off_after_pick = code.len();
    patch_push32_dest(code, fix_after_pick, off_after_pick);

    // Stack: output token `AccountId` word.
    push32(code, &word_u64(tmp_tok));
    code.push(Opcode::MStore as u8);

    push32(code, &word_u64(tmp_tok));
    code.push(Opcode::MLoad as u8);
    code.push(Opcode::IsZero as u8);
    let fix_skip = code.len();
    push32(code, &[0u8; 32]);
    code.push(Opcode::JumpI as u8);

    append_mstore_word(code, cd_base, &selector_word(SELECTOR_TRANSFER));
    match recipient_mem {
        Some(off) => {
            push32(code, &word_u64(off));
            code.push(Opcode::MLoad as u8);
        }
        None => {
            code.push(Opcode::Caller as u8);
        }
    }
    push32(code, &word_u64(cd_base + 32));
    code.push(Opcode::MStore as u8);
    push32(code, &word_u64(mem_dy));
    code.push(Opcode::MLoad as u8);
    push32(code, &word_u64(cd_base + 64));
    code.push(Opcode::MStore as u8);

    push32(code, &word_u64(tmp_tok));
    code.push(Opcode::MLoad as u8);
    push32(code, &word_u64(cd_base));
    push32(code, &word_u64(96));
    code.push(Opcode::Push1 as u8);
    code.push(0);
    code.push(Opcode::Push1 as u8);
    code.push(0);
    code.push(Opcode::Call as u8);

    let off_skip = code.len();
    patch_push32_dest(code, fix_skip, off_skip);
}

/// **v2 / v5:** `CALL` `transfer(to, amount)` if `token_key` slot is non-zero.
/// `recipient_mem`: **`Some(off)`** — load `to` from memory; **`None`** — [`Opcode::Caller`].
fn append_v2_payout_one_side(
    code: &mut Vec<u8>,
    token_key: &[u8; 32],
    amount_mem: u64,
    cd_base: u64,
    tmp_tok: u64,
    recipient_mem: Option<u64>,
) {
    push32(code, token_key);
    code.push(Opcode::SLoad as u8);
    push32(code, &word_u64(tmp_tok));
    code.push(Opcode::MStore as u8);

    push32(code, &word_u64(tmp_tok));
    code.push(Opcode::MLoad as u8);
    code.push(Opcode::IsZero as u8);
    let fix_skip = code.len();
    push32(code, &[0u8; 32]);
    code.push(Opcode::JumpI as u8);

    append_mstore_word(code, cd_base, &selector_word(SELECTOR_TRANSFER));
    match recipient_mem {
        Some(off) => {
            push32(code, &word_u64(off));
            code.push(Opcode::MLoad as u8);
        }
        None => {
            code.push(Opcode::Caller as u8);
        }
    }
    push32(code, &word_u64(cd_base + 32));
    code.push(Opcode::MStore as u8);
    push32(code, &word_u64(amount_mem));
    code.push(Opcode::MLoad as u8);
    push32(code, &word_u64(cd_base + 64));
    code.push(Opcode::MStore as u8);

    push32(code, &word_u64(tmp_tok));
    code.push(Opcode::MLoad as u8);
    push32(code, &word_u64(cd_base));
    push32(code, &word_u64(96));
    code.push(Opcode::Push1 as u8);
    code.push(0);
    code.push(Opcode::Push1 as u8);
    code.push(0);
    code.push(Opcode::Call as u8);

    let off_skip = code.len();
    patch_push32_dest(code, fix_skip, off_skip);
}

/// **v2:** `set_tokens` — once; aborts to `abort_pc` if already configured.
fn append_set_tokens_handler(code: &mut Vec<u8>, abort_pc: usize) {
    push32(code, &tokens_configured_key());
    code.push(Opcode::SLoad as u8);
    code.push(Opcode::IsZero as u8);
    let fix_allow = code.len();
    push32(code, &[0u8; 32]);
    code.push(Opcode::JumpI as u8);
    push32(code, &word_u64(abort_pc as u64));
    code.push(Opcode::Jump as u8);
    let off_allow = code.len();
    patch_push32_dest(code, fix_allow, off_allow);

    push32(code, &word_u64(32));
    code.push(Opcode::MLoad as u8);
    push32(code, &token_a_key());
    code.push(Opcode::SStore as u8);
    push32(code, &word_u64(64));
    code.push(Opcode::MLoad as u8);
    push32(code, &token_b_key());
    code.push(Opcode::SStore as u8);
    push32(code, &amount_word(1));
    push32(code, &tokens_configured_key());
    code.push(Opcode::SStore as u8);
    code.push(Opcode::Stop as u8);
}

/// **v3/v4:** On first LP mint, if [`swap_fee_bps_key`] is unset (**0**), store [`NATIVE_CP_SWAP_FEE_BPS`].
fn append_first_mint_default_swap_fee(c: &mut Vec<u8>, storage_swap_fee: bool) {
    if !storage_swap_fee {
        return;
    }
    push32(c, &swap_fee_bps_key());
    c.push(Opcode::SLoad as u8);
    c.push(Opcode::IsZero as u8);
    let fix_init = c.len();
    push32(c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    let fix_skip_init = c.len();
    push32(c, &[0u8; 32]);
    c.push(Opcode::Jump as u8);
    let off_init = c.len();
    patch_push32_dest(c, fix_init, off_init);
    push32(c, &word_u64(u64::from(NATIVE_CP_SWAP_FEE_BPS)));
    push32(c, &swap_fee_bps_key());
    c.push(Opcode::SStore as u8);
    let off_after_init = c.len();
    patch_push32_dest(c, fix_skip_init, off_after_init);
}

/// **v3/v4:** `set_swap_fee_bps` — only when **total LP == 0**; **1 ≤ fee ≤ 10_000**.
fn append_set_swap_fee_handler(c: &mut Vec<u8>, abort_pc: usize) {
    push32(c, &total_lp_supply_key());
    c.push(Opcode::SLoad as u8);
    c.push(Opcode::IsZero as u8);
    let fix_lp0 = c.len();
    push32(c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    push32(c, &word_u64(abort_pc as u64));
    c.push(Opcode::Jump as u8);
    let off_lp0 = c.len();
    patch_push32_dest(c, fix_lp0, off_lp0);

    push32(c, &word_u64(32));
    c.push(Opcode::MLoad as u8);
    push32(c, &word_u64(MEM_NATIVE_AMM_FEE_BPS));
    c.push(Opcode::MStore as u8);

    // `fee == 0` is invalid for this selector (use default path via first `add_liquidity` instead).
    push32(c, &word_u64(MEM_NATIVE_AMM_FEE_BPS));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::IsZero as u8);
    push32(c, &word_u64(abort_pc as u64));
    c.push(Opcode::JumpI as u8);

    push32(c, &word_u64(MEM_NATIVE_AMM_FEE_BPS));
    c.push(Opcode::MLoad as u8);
    push32(c, &word_u64(10_000u64));
    c.push(Opcode::Gt as u8);
    push32(c, &word_u64(abort_pc as u64));
    c.push(Opcode::JumpI as u8);

    // `SStore` pops **key** then **value** (stack top = key).
    push32(c, &word_u64(MEM_NATIVE_AMM_FEE_BPS));
    c.push(Opcode::MLoad as u8);
    push32(c, &swap_fee_bps_key());
    c.push(Opcode::SStore as u8);
    c.push(Opcode::Stop as u8);
}

/// Apply output-side fee to **MEM_DY** (**384**) after raw CP amount is stored.
fn append_swap_apply_output_fee(c: &mut Vec<u8>, mem_dy: u64, storage_swap_fee: bool, abort_pc: usize) {
    if !storage_swap_fee {
        let fee_keep = 10_000u64 - u64::from(NATIVE_CP_SWAP_FEE_BPS);
        push32(c, &word_u64(mem_dy));
        c.push(Opcode::MLoad as u8);
        push32(c, &word_u64(fee_keep));
        c.push(Opcode::Mul as u8);
        push32(c, &word_u64(10_000));
        c.push(Opcode::Div as u8);
        push32(c, &word_u64(mem_dy));
        c.push(Opcode::MStore as u8);
        return;
    }

    push32(c, &swap_fee_bps_key());
    c.push(Opcode::SLoad as u8);
    push32(c, &word_u64(MEM_NATIVE_AMM_FEE_BPS));
    c.push(Opcode::MStore as u8);

    push32(c, &word_u64(MEM_NATIVE_AMM_FEE_BPS));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::IsZero as u8);
    let fix_def = c.len();
    push32(c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    let fix_past_def = c.len();
    push32(c, &[0u8; 32]);
    c.push(Opcode::Jump as u8);
    let off_def = c.len();
    patch_push32_dest(c, fix_def, off_def);
    push32(c, &word_u64(u64::from(NATIVE_CP_SWAP_FEE_BPS)));
    push32(c, &word_u64(MEM_NATIVE_AMM_FEE_BPS));
    c.push(Opcode::MStore as u8);
    let off_after_def = c.len();
    patch_push32_dest(c, fix_past_def, off_after_def);

    push32(c, &word_u64(MEM_NATIVE_AMM_FEE_BPS));
    c.push(Opcode::MLoad as u8);
    push32(c, &word_u64(10_000u64));
    c.push(Opcode::Gt as u8);
    push32(c, &word_u64(abort_pc as u64));
    c.push(Opcode::JumpI as u8);

    push32(c, &word_u64(10_000));
    push32(c, &word_u64(MEM_NATIVE_AMM_FEE_BPS));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Sub as u8);
    push32(c, &word_u64(MEM_NATIVE_AMM_FEE_KEEP));
    c.push(Opcode::MStore as u8);

    push32(c, &word_u64(mem_dy));
    c.push(Opcode::MLoad as u8);
    push32(c, &word_u64(MEM_NATIVE_AMM_FEE_KEEP));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Mul as u8);
    push32(c, &word_u64(10_000));
    c.push(Opcode::Div as u8);
    push32(c, &word_u64(mem_dy));
    c.push(Opcode::MStore as u8);
}

const LOG_DATA_3_WORDS: u64 = 96;

/// `Log2`: **topic0** fixed, **topic1** = `Caller`; **data** = `MLoad(m_src_i)` × 3 packed at `m_log_base`.
fn emit_log2_caller_three_words(
    c: &mut Vec<u8>,
    topic0: &[u8; 32],
    m_src0: u64,
    m_src1: u64,
    m_src2: u64,
    m_log_base: u64,
) {
    for (slot, src) in [(0u64, m_src0), (32, m_src1), (64, m_src2)] {
        push32(c, &word_u64(src));
        c.push(Opcode::MLoad as u8);
        push32(c, &word_u64(m_log_base + slot));
        c.push(Opcode::MStore as u8);
    }
    // `Log2` pops **offset** first (stack top), then **size**, then topics — push offset last.
    push32(c, topic0);
    c.push(Opcode::Caller as u8);
    push32(c, &word_u64(LOG_DATA_3_WORDS));
    push32(c, &word_u64(m_log_base));
    c.push(Opcode::Log2 as u8);
}

/// Add-liquidity: LP mint + reserve update (scratch `448..736`, key at `704`).
fn append_add_liquidity_lp(c: &mut Vec<u8>, abort_pc: usize, storage_swap_fee: bool) {
    const DA: u64 = 448;
    const DB: u64 = 480;
    const T: u64 = 512;
    const RA: u64 = 544;
    const RB: u64 = 576;
    const LP1: u64 = 608;
    const LP2: u64 = 640;
    const LP: u64 = 672;
    const KEY: u64 = 704;

    push32(c, &word_u64(32));
    c.push(Opcode::MLoad as u8);
    push32(c, &word_u64(DA));
    c.push(Opcode::MStore as u8);

    push32(c, &word_u64(64));
    c.push(Opcode::MLoad as u8);
    push32(c, &word_u64(DB));
    c.push(Opcode::MStore as u8);

    push32(c, &word_u64(DA));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::IsZero as u8);
    let fix_da0 = c.len();
    push32(c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(c, &word_u64(DB));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::IsZero as u8);
    let fix_db0 = c.len();
    push32(c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(c, &total_lp_supply_key());
    c.push(Opcode::SLoad as u8);
    push32(c, &word_u64(T));
    c.push(Opcode::MStore as u8);

    push32(c, &word_u64(T));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::IsZero as u8);
    let fix_first = c.len();
    push32(c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(c, &reserve_a_key());
    c.push(Opcode::SLoad as u8);
    push32(c, &word_u64(RA));
    c.push(Opcode::MStore as u8);

    push32(c, &reserve_b_key());
    c.push(Opcode::SLoad as u8);
    push32(c, &word_u64(RB));
    c.push(Opcode::MStore as u8);

    push32(c, &word_u64(RA));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::IsZero as u8);
    let fix_ra0 = c.len();
    push32(c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(c, &word_u64(RB));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::IsZero as u8);
    let fix_rb0 = c.len();
    push32(c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(c, &word_u64(DA));
    c.push(Opcode::MLoad as u8);
    push32(c, &word_u64(T));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Mul as u8);
    push32(c, &word_u64(RA));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Div as u8);
    push32(c, &word_u64(LP1));
    c.push(Opcode::MStore as u8);

    push32(c, &word_u64(DB));
    c.push(Opcode::MLoad as u8);
    push32(c, &word_u64(T));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Mul as u8);
    push32(c, &word_u64(RB));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Div as u8);
    push32(c, &word_u64(LP2));
    c.push(Opcode::MStore as u8);

    push32(c, &word_u64(LP1));
    c.push(Opcode::MLoad as u8);
    push32(c, &word_u64(LP2));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Lt as u8);
    let fix_pick_lp2 = c.len();
    push32(c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(c, &word_u64(LP2));
    c.push(Opcode::MLoad as u8);
    push32(c, &word_u64(LP));
    c.push(Opcode::MStore as u8);
    let fix_after_min = c.len();
    push32(c, &[0u8; 32]);
    c.push(Opcode::Jump as u8);

    let off_pick_lp1 = c.len();
    patch_push32_dest(c, fix_pick_lp2, off_pick_lp1);
    push32(c, &word_u64(LP1));
    c.push(Opcode::MLoad as u8);
    push32(c, &word_u64(LP));
    c.push(Opcode::MStore as u8);

    let off_after_ratio_min = c.len();
    patch_push32_dest(c, fix_after_min, off_after_ratio_min);

    push32(c, &word_u64(LP));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::IsZero as u8);
    let fix_lp0_ratio = c.len();
    push32(c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    let fix_ratio_to_merge = c.len();
    push32(c, &[0u8; 32]);
    c.push(Opcode::Jump as u8);

    let off_first_mint = c.len();
    patch_push32_dest(c, fix_first, off_first_mint);
    append_first_mint_default_swap_fee(c, storage_swap_fee);

    push32(c, &word_u64(DA));
    c.push(Opcode::MLoad as u8);
    push32(c, &word_u64(DB));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Lt as u8);
    let fix_pick_db = c.len();
    push32(c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(c, &word_u64(DB));
    c.push(Opcode::MLoad as u8);
    push32(c, &word_u64(LP));
    c.push(Opcode::MStore as u8);
    let fix_after_fm_min = c.len();
    push32(c, &[0u8; 32]);
    c.push(Opcode::Jump as u8);

    let off_pick_da = c.len();
    patch_push32_dest(c, fix_pick_db, off_pick_da);
    push32(c, &word_u64(DA));
    c.push(Opcode::MLoad as u8);
    push32(c, &word_u64(LP));
    c.push(Opcode::MStore as u8);

    let off_after_fm = c.len();
    patch_push32_dest(c, fix_after_fm_min, off_after_fm);

    let fix_fm_to_merge = c.len();
    push32(c, &[0u8; 32]);
    c.push(Opcode::Jump as u8);

    let off_merge = c.len();
    patch_push32_dest(c, fix_ratio_to_merge, off_merge);
    patch_push32_dest(c, fix_fm_to_merge, off_merge);

    push32(c, &reserve_a_key());
    c.push(Opcode::SLoad as u8);
    push32(c, &word_u64(DA));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Add as u8);
    push32(c, &reserve_a_key());
    c.push(Opcode::SStore as u8);

    push32(c, &reserve_b_key());
    c.push(Opcode::SLoad as u8);
    push32(c, &word_u64(DB));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Add as u8);
    push32(c, &reserve_b_key());
    c.push(Opcode::SStore as u8);

    push32(c, &total_lp_supply_key());
    c.push(Opcode::SLoad as u8);
    push32(c, &word_u64(LP));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Add as u8);
    push32(c, &total_lp_supply_key());
    c.push(Opcode::SStore as u8);

    c.push(Opcode::Caller as u8);
    push32(c, &LP_BALANCE_STORAGE_XOR);
    c.push(Opcode::Xor as u8);
    push32(c, &word_u64(KEY));
    c.push(Opcode::MStore as u8);

    push32(c, &word_u64(KEY));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::SLoad as u8);
    push32(c, &word_u64(LP));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Add as u8);
    push32(c, &word_u64(KEY));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::SStore as u8);

    const LIQ_LOG: u64 = 800;
    emit_log2_caller_three_words(c, &NATIVE_AMM_TOPIC_ADD_LIQUIDITY, DA, DB, LP, LIQ_LOG);

    // `Return` 32-byte word: LP minted this call (u128 in low 16 bytes, same layout as amount_word).
    const MEM_RET_LP_MINT: u64 = 928;
    push32(c, &word_u64(LP));
    c.push(Opcode::MLoad as u8);
    push32(c, &word_u64(MEM_RET_LP_MINT));
    c.push(Opcode::MStore as u8);
    push32(c, &word_u64(32));
    push32(c, &word_u64(MEM_RET_LP_MINT));
    c.push(Opcode::Return as u8);

    patch_push32_dest(c, fix_da0, abort_pc);
    patch_push32_dest(c, fix_db0, abort_pc);
    patch_push32_dest(c, fix_ra0, abort_pc);
    patch_push32_dest(c, fix_rb0, abort_pc);
    patch_push32_dest(c, fix_lp0_ratio, abort_pc);
}

/// Remove-liquidity tail: pro-rata withdrawal + slippage (scratch `448..736`).
/// `payout_recipient_slots`: **`Some((a,b))**` — memory words for `transfer` **to** on each token; **`None`** — [`Opcode::Caller`].
fn append_remove_liquidity_tail(
    c: &mut Vec<u8>,
    abort_pc: usize,
    token_hooks: bool,
    payout_recipient_slots: Option<(u64, u64)>,
) {
    const BURN: u64 = 448;
    const MINA: u64 = 480;
    const MINB: u64 = 512;
    const RA: u64 = 544;
    const RB: u64 = 576;
    const TT: u64 = 608;
    const DAO: u64 = 640;
    const DBO: u64 = 672;
    const KEY: u64 = 704;

    push32(c, &word_u64(32));
    c.push(Opcode::MLoad as u8);
    push32(c, &word_u64(BURN));
    c.push(Opcode::MStore as u8);

    push32(c, &word_u64(64));
    c.push(Opcode::MLoad as u8);
    push32(c, &word_u64(MINA));
    c.push(Opcode::MStore as u8);

    push32(c, &word_u64(96));
    c.push(Opcode::MLoad as u8);
    push32(c, &word_u64(MINB));
    c.push(Opcode::MStore as u8);

    push32(c, &word_u64(BURN));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::IsZero as u8);
    let fix_b0 = c.len();
    push32(c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(c, &total_lp_supply_key());
    c.push(Opcode::SLoad as u8);
    push32(c, &word_u64(TT));
    c.push(Opcode::MStore as u8);

    push32(c, &word_u64(TT));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::IsZero as u8);
    let fix_t0 = c.len();
    push32(c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    c.push(Opcode::Caller as u8);
    push32(c, &LP_BALANCE_STORAGE_XOR);
    c.push(Opcode::Xor as u8);
    push32(c, &word_u64(KEY));
    c.push(Opcode::MStore as u8);

    push32(c, &word_u64(BURN));
    c.push(Opcode::MLoad as u8);
    push32(c, &word_u64(KEY));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::SLoad as u8);
    c.push(Opcode::Gt as u8);
    let fix_burn_gt = c.len();
    push32(c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(c, &reserve_a_key());
    c.push(Opcode::SLoad as u8);
    push32(c, &word_u64(RA));
    c.push(Opcode::MStore as u8);

    push32(c, &reserve_b_key());
    c.push(Opcode::SLoad as u8);
    push32(c, &word_u64(RB));
    c.push(Opcode::MStore as u8);

    push32(c, &word_u64(BURN));
    c.push(Opcode::MLoad as u8);
    push32(c, &word_u64(RA));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Mul as u8);
    push32(c, &word_u64(TT));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Div as u8);
    push32(c, &word_u64(DAO));
    c.push(Opcode::MStore as u8);

    push32(c, &word_u64(BURN));
    c.push(Opcode::MLoad as u8);
    push32(c, &word_u64(RB));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Mul as u8);
    push32(c, &word_u64(TT));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Div as u8);
    push32(c, &word_u64(DBO));
    c.push(Opcode::MStore as u8);

    push32(c, &word_u64(DAO));
    c.push(Opcode::MLoad as u8);
    push32(c, &word_u64(MINA));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Lt as u8);
    let fix_sa = c.len();
    push32(c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(c, &word_u64(DBO));
    c.push(Opcode::MLoad as u8);
    push32(c, &word_u64(MINB));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Lt as u8);
    let fix_sb = c.len();
    push32(c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(c, &word_u64(RA));
    c.push(Opcode::MLoad as u8);
    push32(c, &word_u64(DAO));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Sub as u8);
    push32(c, &reserve_a_key());
    c.push(Opcode::SStore as u8);

    push32(c, &word_u64(RB));
    c.push(Opcode::MLoad as u8);
    push32(c, &word_u64(DBO));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Sub as u8);
    push32(c, &reserve_b_key());
    c.push(Opcode::SStore as u8);

    push32(c, &word_u64(KEY));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::SLoad as u8);
    push32(c, &word_u64(BURN));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Sub as u8);
    push32(c, &word_u64(KEY));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::SStore as u8);

    push32(c, &word_u64(TT));
    c.push(Opcode::MLoad as u8);
    push32(c, &word_u64(BURN));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Sub as u8);
    push32(c, &total_lp_supply_key());
    c.push(Opcode::SStore as u8);

    const V2_RM_CALLDATA: u64 = 1248;
    const V2_RM_TMP: u64 = 1152;
    if token_hooks {
        let (ra, rb) = match payout_recipient_slots {
            Some((a, b)) => (Some(a), Some(b)),
            None => (None, None),
        };
        append_v2_payout_one_side(
            c,
            &token_a_key(),
            DAO,
            V2_RM_CALLDATA,
            V2_RM_TMP,
            ra,
        );
        append_v2_payout_one_side(
            c,
            &token_b_key(),
            DBO,
            V2_RM_CALLDATA,
            V2_RM_TMP,
            rb,
        );
    }

    const RM_LOG: u64 = 800;
    emit_log2_caller_three_words(c, &NATIVE_AMM_TOPIC_REMOVE_LIQUIDITY, BURN, DAO, DBO, RM_LOG);

    c.push(Opcode::Stop as u8);

    patch_push32_dest(c, fix_b0, abort_pc);
    patch_push32_dest(c, fix_t0, abort_pc);
    patch_push32_dest(c, fix_burn_gt, abort_pc);
    patch_push32_dest(c, fix_sa, abort_pc);
    patch_push32_dest(c, fix_sb, abort_pc);
}

#[derive(Clone, Copy)]
struct CpPoolBuildOpts {
    token_hooks: bool,
    storage_swap_fee: bool,
    /// v5: [`SELECTOR_SWAP_TO`] + scratch at offset **1088** for router-safe token payout.
    swap_with_recipient: bool,
}

/// Assembled pool: dispatch + `add_liquidity` (LP mint) + `swap` + `remove_liquidity` (pro-rata).
///
/// Scratch memory: swap `128..384`, swap log buffer `416..512`; add/remove `448..736`, liq log `800..896`.
/// **v2** (`token_hooks`): `set_tokens` dispatch, `CALL` on swap out + remove payouts; scratch `1152`, `1248`.
/// **v3/v4** (`storage_swap_fee`): [`swap_fee_bps_key`], scratch **`1024` / `1056`**, [`SELECTOR_SET_SWAP_FEE_BPS`].
/// **v5** (`swap_with_recipient`): scratch **`1088`** + [`SELECTOR_SWAP_TO`].
fn build_cp_pool(opts: CpPoolBuildOpts) -> Vec<u8> {
    let token_hooks = opts.token_hooks;
    let storage_swap_fee = opts.storage_swap_fee;
    let swap_with_recipient = opts.swap_with_recipient;
    // Memory scratch (offsets ≥ 128 to stay past 128-byte calldata).
    const MEM_DIR: u64 = 128;
    const MEM_RA: u64 = 160;
    const MEM_RB: u64 = 192;
    const MEM_DX: u64 = 224;
    const MEM_MIN: u64 = 256;
    const MEM_RIN: u64 = 288;
    const MEM_ROUT: u64 = 320;
    const MEM_RIN_P: u64 = 352;
    const MEM_DY: u64 = 384;
    /// Output recipient for **`swap_to`** (v5); loaded into transfer calldata instead of `Caller`.
    const MEM_SWAP_RECIP: u64 = 1088;

    let mut c: Vec<u8> = Vec::new();

    // --- dispatch: compare word0 to selectors ---
    // SWAP
    push32(&mut c, &word_u64(0));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &selector_word(SELECTOR_SWAP));
    c.push(Opcode::Eq as u8);
    let fix_j_swap = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    let fix_j_swap_to = if token_hooks && swap_with_recipient {
        push32(&mut c, &word_u64(0));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &selector_word(SELECTOR_SWAP_TO));
        c.push(Opcode::Eq as u8);
        let at = c.len();
        push32(&mut c, &[0u8; 32]);
        c.push(Opcode::JumpI as u8);
        Some(at)
    } else {
        None
    };

    // ADD_LIQ
    push32(&mut c, &word_u64(0));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &selector_word(SELECTOR_ADD_LIQUIDITY));
    c.push(Opcode::Eq as u8);
    let fix_j_add = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    // REMOVE
    push32(&mut c, &word_u64(0));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &selector_word(SELECTOR_REMOVE_LIQUIDITY));
    c.push(Opcode::Eq as u8);
    let fix_j_rm = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    let fix_j_rm_liq_to = if token_hooks && swap_with_recipient {
        push32(&mut c, &word_u64(0));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &selector_word(SELECTOR_REMOVE_LIQUIDITY_TO));
        c.push(Opcode::Eq as u8);
        let at = c.len();
        push32(&mut c, &[0u8; 32]);
        c.push(Opcode::JumpI as u8);
        Some(at)
    } else {
        None
    };

    let fix_j_set = if token_hooks {
        push32(&mut c, &word_u64(0));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &selector_word(SELECTOR_SET_TOKENS));
        c.push(Opcode::Eq as u8);
        let at = c.len();
        push32(&mut c, &[0u8; 32]);
        c.push(Opcode::JumpI as u8);
        Some(at)
    } else {
        None
    };

    let fix_j_set_fee = if storage_swap_fee {
        push32(&mut c, &word_u64(0));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &selector_word(SELECTOR_SET_SWAP_FEE_BPS));
        c.push(Opcode::Eq as u8);
        let at = c.len();
        push32(&mut c, &[0u8; 32]);
        c.push(Opcode::JumpI as u8);
        Some(at)
    } else {
        None
    };

    let off_stop_unknown = c.len();
    c.push(Opcode::Stop as u8);

    if let Some(at) = fix_j_set {
        let off_set = c.len();
        patch_push32_dest(&mut c, at, off_set);
        append_set_tokens_handler(&mut c, off_stop_unknown);
    }

    if let Some(at) = fix_j_set_fee {
        let off_fee = c.len();
        patch_push32_dest(&mut c, at, off_fee);
        append_set_swap_fee_handler(&mut c, off_stop_unknown);
    }

    // --- add_liquidity (LP mint + reserves) ---
    let off_add = c.len();
    patch_push32_dest(&mut c, fix_j_add, off_add);
    append_add_liquidity_lp(&mut c, off_stop_unknown, storage_swap_fee);

    // --- remove_liquidity (pro-rata burn) ---
    const MEM_RM_RECIP_A: u64 = 960;
    const MEM_RM_RECIP_B: u64 = 992;
    if token_hooks && swap_with_recipient {
        let off_rm_liq_to_entry = c.len();
        if let Some(at) = fix_j_rm_liq_to {
            patch_push32_dest(&mut c, at, off_rm_liq_to_entry);
        }
        push32(&mut c, &word_u64(128));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &word_u64(MEM_RM_RECIP_A));
        c.push(Opcode::MStore as u8);
        push32(&mut c, &word_u64(160));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &word_u64(MEM_RM_RECIP_B));
        c.push(Opcode::MStore as u8);
        let fix_jump_rm_to_common = c.len();
        push32(&mut c, &[0u8; 32]);
        c.push(Opcode::Jump as u8);

        let off_rm_legacy_entry = c.len();
        patch_push32_dest(&mut c, fix_j_rm, off_rm_legacy_entry);

        c.push(Opcode::Caller as u8);
        push32(&mut c, &word_u64(MEM_RM_RECIP_A));
        c.push(Opcode::MStore as u8);
        c.push(Opcode::Caller as u8);
        push32(&mut c, &word_u64(MEM_RM_RECIP_B));
        c.push(Opcode::MStore as u8);

        let off_rm_common = c.len();
        patch_push32_dest(&mut c, fix_jump_rm_to_common, off_rm_common);
        append_remove_liquidity_tail(
            &mut c,
            off_stop_unknown,
            token_hooks,
            Some((MEM_RM_RECIP_A, MEM_RM_RECIP_B)),
        );
    } else {
        let off_rm = c.len();
        patch_push32_dest(&mut c, fix_j_rm, off_rm);
        append_remove_liquidity_tail(&mut c, off_stop_unknown, token_hooks, None);
    }

    // --- swap ---
    if token_hooks && swap_with_recipient {
        let off_swap_to_entry = c.len();
        if let Some(at) = fix_j_swap_to {
            patch_push32_dest(&mut c, at, off_swap_to_entry);
        }
        push32(&mut c, &word_u64(128));
        c.push(Opcode::MLoad as u8);
        push32(&mut c, &word_u64(MEM_SWAP_RECIP));
        c.push(Opcode::MStore as u8);
        let fix_jump_swap_to_to_common = c.len();
        push32(&mut c, &[0u8; 32]);
        c.push(Opcode::Jump as u8);

        let off_swap_10_entry = c.len();
        patch_push32_dest(&mut c, fix_j_swap, off_swap_10_entry);

        c.push(Opcode::Caller as u8);
        push32(&mut c, &word_u64(MEM_SWAP_RECIP));
        c.push(Opcode::MStore as u8);

        let off_swap_common = c.len();
        patch_push32_dest(&mut c, fix_jump_swap_to_to_common, off_swap_common);
    } else {
        let off_swap_common = c.len();
        patch_push32_dest(&mut c, fix_j_swap, off_swap_common);
    }

    // Load ra, rb, dx, min; store scratch
    push32(&mut c, &reserve_a_key());
    c.push(Opcode::SLoad as u8);
    push32(&mut c, &word_u64(MEM_RA));
    c.push(Opcode::MStore as u8);

    push32(&mut c, &reserve_b_key());
    c.push(Opcode::SLoad as u8);
    push32(&mut c, &word_u64(MEM_RB));
    c.push(Opcode::MStore as u8);

    push32(&mut c, &word_u64(64));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_DX));
    c.push(Opcode::MStore as u8);

    push32(&mut c, &word_u64(96));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_MIN));
    c.push(Opcode::MStore as u8);

    // dx == 0 → abort
    push32(&mut c, &word_u64(MEM_DX));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::IsZero as u8);
    let fix_dx0 = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    // dir at calldata 32 → mem MEM_DIR
    push32(&mut c, &word_u64(32));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_DIR));
    c.push(Opcode::MStore as u8);

    // if dir == 1 → B→A path (direction word equals `amount_word(1)`)
    push32(&mut c, &word_u64(MEM_DIR));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &amount_word(1));
    c.push(Opcode::Eq as u8);
    let fix_j_b2a = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    // A→B: rin=ra, rout=rb
    push32(&mut c, &word_u64(MEM_RA));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_RIN));
    c.push(Opcode::MStore as u8);
    push32(&mut c, &word_u64(MEM_RB));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_ROUT));
    c.push(Opcode::MStore as u8);
    let fix_after_dir = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::Jump as u8);

    // B→A: rin=rb, rout=ra
    let off_b2a = c.len();
    patch_push32_dest(&mut c, fix_j_b2a, off_b2a);
    push32(&mut c, &word_u64(MEM_RB));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_RIN));
    c.push(Opcode::MStore as u8);
    push32(&mut c, &word_u64(MEM_RA));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_ROUT));
    c.push(Opcode::MStore as u8);

    let off_swap_math = c.len();
    patch_push32_dest(&mut c, fix_after_dir, off_swap_math);

    // rin_p = rin + dx
    push32(&mut c, &word_u64(MEM_RIN));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_DX));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Add as u8);
    push32(&mut c, &word_u64(MEM_RIN_P));
    c.push(Opcode::MStore as u8);

    // rin_p == 0 → abort
    push32(&mut c, &word_u64(MEM_RIN_P));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::IsZero as u8);
    let fix_rp0 = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    // dy = rout * dx / rin_p
    push32(&mut c, &word_u64(MEM_ROUT));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_DX));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Mul as u8);
    push32(&mut c, &word_u64(MEM_RIN_P));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Div as u8);
    push32(&mut c, &word_u64(MEM_DY));
    c.push(Opcode::MStore as u8);

    // Output-side LP fee: `dy = dy * (10_000 - fee_bps) / 10_000` (constant or from [`swap_fee_bps_key`]).
    append_swap_apply_output_fee(&mut c, MEM_DY, storage_swap_fee, off_stop_unknown);

    // dy == 0 after fee → abort (dust / rounding)
    push32(&mut c, &word_u64(MEM_DY));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::IsZero as u8);
    let fix_dy_fee0 = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    // slippage: if dy < min_out → abort (`Lt`: top = b, next = a → a < b)
    push32(&mut c, &word_u64(MEM_DY));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_MIN));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Lt as u8);
    let fix_slip = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    // rin_new = rin_p (already in MEM_RIN_P)
    // rout_new = rout - dy
    push32(&mut c, &word_u64(MEM_ROUT));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_DY));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Sub as u8);
    push32(&mut c, &word_u64(MEM_ROUT));
    c.push(Opcode::MStore as u8);

    // Write back: if we came from A→B, rin was ra, rout was rb. From B→A, rin was rb, rout was ra.
    // Re-dispatch using dir word: if dir==1, MEM_RA gets rout_new, MEM_RB gets rin_new? Actually after math:
    // MEM_RIN_P is new "in" reserve, MEM_ROUT holds new "out" reserve (we overwrote rout in MEM_ROUT with rout_new)

    // Reload dir
    push32(&mut c, &word_u64(MEM_DIR));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &amount_word(1));
    c.push(Opcode::Eq as u8);
    let fix_store_b2a = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    // A→B store: ra = MEM_RIN_P, rb = MEM_ROUT
    push32(&mut c, &word_u64(MEM_RIN_P));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &reserve_a_key());
    c.push(Opcode::SStore as u8);
    push32(&mut c, &word_u64(MEM_ROUT));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &reserve_b_key());
    c.push(Opcode::SStore as u8);
    let fix_after_store = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::Jump as u8);

    // B→A store: rb = MEM_RIN_P, ra = MEM_ROUT
    let off_store_b2a = c.len();
    patch_push32_dest(&mut c, fix_store_b2a, off_store_b2a);
    push32(&mut c, &word_u64(MEM_RIN_P));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &reserve_b_key());
    c.push(Opcode::SStore as u8);
    push32(&mut c, &word_u64(MEM_ROUT));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &reserve_a_key());
    c.push(Opcode::SStore as u8);

    let off_swap_done = c.len();
    patch_push32_dest(&mut c, fix_after_store, off_swap_done);

    const V2_SWAP_TMP: u64 = 1152;
    const V2_SWAP_CALLDATA: u64 = 1248;
    if token_hooks {
        append_v2_swap_output_reference_call(
            &mut c,
            MEM_DIR,
            MEM_DY,
            V2_SWAP_CALLDATA,
            V2_SWAP_TMP,
            if swap_with_recipient {
                Some(MEM_SWAP_RECIP)
            } else {
                None
            },
        );
    }

    const MEM_SWAP_LOG: u64 = 416;
    emit_log2_caller_three_words(
        &mut c,
        &NATIVE_AMM_TOPIC_SWAP,
        MEM_DIR,
        MEM_DX,
        MEM_DY,
        MEM_SWAP_LOG,
    );

    c.push(Opcode::Stop as u8);

    // abort labels → STOP
    patch_push32_dest(&mut c, fix_dx0, off_stop_unknown);
    patch_push32_dest(&mut c, fix_rp0, off_stop_unknown);
    patch_push32_dest(&mut c, fix_dy_fee0, off_stop_unknown);
    patch_push32_dest(&mut c, fix_slip, off_stop_unknown);

    c
}

/// Ledger-only native CP pool (v1). CREATE2 salt: [`NATIVE_CP_POOL_CREATE2_SALT_V1`].
#[must_use]
pub fn constant_product_pool_bytecode() -> Vec<u8> {
    build_cp_pool(CpPoolBuildOpts {
        token_hooks: false,
        storage_swap_fee: false,
        swap_with_recipient: false,
    })
}

/// Token-hook pool (v2): one-time [`SELECTOR_SET_TOKENS`], then optional `CALL` payouts. CREATE2: [`NATIVE_CP_POOL_CREATE2_SALT_V2`].
#[must_use]
pub fn constant_product_pool_bytecode_v2() -> Vec<u8> {
    build_cp_pool(CpPoolBuildOpts {
        token_hooks: true,
        storage_swap_fee: false,
        swap_with_recipient: false,
    })
}

/// Ledger-only pool (v3): on-chain swap **fee bps** in [`swap_fee_bps_key`]; optional [`encode_set_swap_fee_bps_calldata`] before first `add_liquidity`. CREATE2: [`NATIVE_CP_POOL_CREATE2_SALT_V3`].
#[must_use]
pub fn constant_product_pool_bytecode_v3() -> Vec<u8> {
    build_cp_pool(CpPoolBuildOpts {
        token_hooks: false,
        storage_swap_fee: true,
        swap_with_recipient: false,
    })
}

/// Token hooks + configurable fee (v4). CREATE2: [`NATIVE_CP_POOL_CREATE2_SALT_V4`].
#[must_use]
pub fn constant_product_pool_bytecode_v4() -> Vec<u8> {
    build_cp_pool(CpPoolBuildOpts {
        token_hooks: true,
        storage_swap_fee: true,
        swap_with_recipient: false,
    })
}

/// Token hooks + configurable fee + **`swap_to`** (explicit swap output recipient). CREATE2: [`NATIVE_CP_POOL_CREATE2_SALT_V5`].
#[must_use]
pub fn constant_product_pool_bytecode_v5() -> Vec<u8> {
    build_cp_pool(CpPoolBuildOpts {
        token_hooks: true,
        storage_swap_fee: true,
        swap_with_recipient: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use boing_primitives::{Account, AccountId};
    use boing_state::StateStore;

    use crate::interpreter::Interpreter;

    #[test]
    fn encode_swap_matches_doc_example_structure() {
        let v = encode_swap_calldata(0, 1_000_000, 900_000);
        assert_eq!(v.len(), 128);
        assert_eq!(v[31], SELECTOR_SWAP);
    }

    #[test]
    fn encode_swap_to_is_160_bytes_with_selector_0x15() {
        let recip = AccountId([0x42u8; 32]);
        let v = encode_swap_to_calldata(0, 1, 1, &recip);
        assert_eq!(v.len(), 160);
        assert_eq!(v[31], SELECTOR_SWAP_TO);
        assert_eq!(&v[128..160], &recip.0);
    }

    #[test]
    fn constant_product_math_matches_swap_bytecode_a_to_b() {
        let ra = 1000u64;
        let rb = 2000u64;
        let dx = 100u64;
        assert_eq!(constant_product_amount_out(ra, rb, dx), 181);
        let dy = constant_product_amount_out_after_fee(ra, rb, dx);
        assert_eq!(dy, 180);

        let sender = AccountId([0xabu8; 32]);
        let contract = AccountId([0xcd; 32]);
        let mut state = StateStore::new();
        state.insert(Account {
            id: contract,
            state: Default::default(),
        });
        state.merge_contract_storage(contract, reserve_a_key(), amount_word(u128::from(ra)));
        state.merge_contract_storage(contract, reserve_b_key(), amount_word(u128::from(rb)));

        let code = constant_product_pool_bytecode();
        let calldata = encode_swap_calldata(0, u128::from(dx), u128::from(dy)); // min_out = exact

        let mut it = Interpreter::new(code, 5_000_000);
        it.run(sender, contract, &calldata, &mut state).unwrap();

        assert_eq!(it.logs.len(), 1);
        assert_eq!(it.logs[0].topics.len(), 2);
        assert_eq!(it.logs[0].topics[0], NATIVE_AMM_TOPIC_SWAP);
        assert_eq!(it.logs[0].topics[1], sender.0);
        assert_eq!(it.logs[0].data.len(), 96);
        assert_eq!(&it.logs[0].data[16..32], &amount_word(0)[16..32]);
        assert_eq!(&it.logs[0].data[48..64], &amount_word(u128::from(dx))[16..32]);
        assert_eq!(&it.logs[0].data[80..96], &amount_word(u128::from(dy))[16..32]);

        let ra2 = u128::from_be_bytes(state.get_contract_storage(&contract, &reserve_a_key())[16..32].try_into().unwrap());
        let rb2 = u128::from_be_bytes(state.get_contract_storage(&contract, &reserve_b_key())[16..32].try_into().unwrap());
        assert_eq!(ra2, u128::from(ra + dx));
        assert_eq!(rb2, u128::from(rb - dy));
    }

    #[test]
    fn swap_b_to_a_symmetric() {
        let ra = 500u64;
        let rb = 800u64;
        let dx = 50u64;
        let dy = constant_product_amount_out_after_fee(rb, ra, dx); // in=B, out=A
        let sender = AccountId([1u8; 32]);
        let contract = AccountId([2u8; 32]);
        let mut state = StateStore::new();
        state.insert(Account {
            id: contract,
            state: Default::default(),
        });
        state.merge_contract_storage(contract, reserve_a_key(), amount_word(u128::from(ra)));
        state.merge_contract_storage(contract, reserve_b_key(), amount_word(u128::from(rb)));

        let code = constant_product_pool_bytecode();
        let calldata = encode_swap_calldata(1, u128::from(dx), u128::from(dy));

        let mut it = Interpreter::new(code, 5_000_000);
        it.run(sender, contract, &calldata, &mut state).unwrap();

        let ra2 = u128::from_be_bytes(state.get_contract_storage(&contract, &reserve_a_key())[16..32].try_into().unwrap());
        let rb2 = u128::from_be_bytes(state.get_contract_storage(&contract, &reserve_b_key())[16..32].try_into().unwrap());
        assert_eq!(rb2, u128::from(rb + dx));
        assert_eq!(ra2, u128::from(ra - dy));
    }

    #[test]
    fn add_liquidity_increases_reserves() {
        let sender = AccountId([3u8; 32]);
        let contract = AccountId([4u8; 32]);
        let mut state = StateStore::new();
        state.insert(Account {
            id: contract,
            state: Default::default(),
        });
        state.merge_contract_storage(contract, reserve_a_key(), amount_word(100));
        state.merge_contract_storage(contract, reserve_b_key(), amount_word(200));

        let code = constant_product_pool_bytecode();
        let calldata = encode_add_liquidity_calldata(10, 20, 0);
        let mut it = Interpreter::new(code, 5_000_000);
        it.run(sender, contract, &calldata, &mut state).unwrap();

        assert_eq!(it.logs.len(), 1);
        assert_eq!(it.logs[0].topics[0], NATIVE_AMM_TOPIC_ADD_LIQUIDITY);
        assert_eq!(it.logs[0].topics[1], sender.0);
        assert_eq!(it.logs[0].data.len(), 96);

        let ra2 = u128::from_be_bytes(state.get_contract_storage(&contract, &reserve_a_key())[16..32].try_into().unwrap());
        let rb2 = u128::from_be_bytes(state.get_contract_storage(&contract, &reserve_b_key())[16..32].try_into().unwrap());
        assert_eq!(ra2, 110);
        assert_eq!(rb2, 220);
        let lp_key = lp_balance_storage_key(&sender.0);
        let t = u128::from_be_bytes(state.get_contract_storage(&contract, &total_lp_supply_key())[16..32].try_into().unwrap());
        let ulp = u128::from_be_bytes(state.get_contract_storage(&contract, &lp_key)[16..32].try_into().unwrap());
        assert_eq!(t, 10);
        assert_eq!(ulp, 10);
        let ret = it.return_data.as_deref().expect("add_liquidity return data");
        assert_eq!(decode_add_liquidity_return_lp_minted(ret), Some(10));
    }

    #[test]
    fn decode_add_liquidity_return_lp_minted_accepts_only_canonical_words() {
        assert_eq!(decode_add_liquidity_return_lp_minted(&[]), None);
        assert_eq!(decode_add_liquidity_return_lp_minted(&[0u8; 31]), None);
        let mut w = amount_word(42);
        assert_eq!(decode_add_liquidity_return_lp_minted(&w), Some(42));
        w[0] = 1;
        assert_eq!(decode_add_liquidity_return_lp_minted(&w), None);
    }

    #[test]
    fn remove_liquidity_burns_pro_rata() {
        let sender = AccountId([5u8; 32]);
        let contract = AccountId([6u8; 32]);
        let mut state = StateStore::new();
        state.insert(Account {
            id: contract,
            state: Default::default(),
        });

        let code = constant_product_pool_bytecode();
        let mut it = Interpreter::new(code.clone(), 5_000_000);
        it.run(sender, contract, &encode_add_liquidity_calldata(1_000, 2_000, 0), &mut state)
            .unwrap();

        assert_eq!(it.logs.len(), 1);
        assert_eq!(it.logs[0].topics[0], NATIVE_AMM_TOPIC_ADD_LIQUIDITY);

        let t0 = u128::from_be_bytes(state.get_contract_storage(&contract, &total_lp_supply_key())[16..32].try_into().unwrap());
        assert_eq!(t0, 1_000);

        let mut it2 = Interpreter::new(code, 5_000_000);
        it2
            .run(sender, contract, &encode_remove_liquidity_calldata(250, 0, 0), &mut state)
            .unwrap();

        assert_eq!(it2.logs.len(), 1);
        assert_eq!(it2.logs[0].topics[0], NATIVE_AMM_TOPIC_REMOVE_LIQUIDITY);

        let ra = u128::from_be_bytes(state.get_contract_storage(&contract, &reserve_a_key())[16..32].try_into().unwrap());
        let rb = u128::from_be_bytes(state.get_contract_storage(&contract, &reserve_b_key())[16..32].try_into().unwrap());
        assert_eq!(ra, 750);
        assert_eq!(rb, 1_500);
        let t1 = u128::from_be_bytes(state.get_contract_storage(&contract, &total_lp_supply_key())[16..32].try_into().unwrap());
        assert_eq!(t1, 750);
        let lp_key = lp_balance_storage_key(&sender.0);
        let ulp = u128::from_be_bytes(state.get_contract_storage(&contract, &lp_key)[16..32].try_into().unwrap());
        assert_eq!(ulp, 750);
    }

    /// Deploy-shaped bootstrap: empty pool → add liquidity → swap A→B; assert reserves.
    #[test]
    fn add_liquidity_then_swap_integration() {
        let sender = AccountId([0x11u8; 32]);
        let contract = AccountId([0x22u8; 32]);
        let mut state = StateStore::new();
        state.insert(Account {
            id: contract,
            state: Default::default(),
        });

        let code = constant_product_pool_bytecode();

        let add_calldata = encode_add_liquidity_calldata(1_000, 2_000, 0);
        let mut it = Interpreter::new(code.clone(), 5_000_000);
        it.run(sender, contract, &add_calldata, &mut state).unwrap();
        assert_eq!(it.logs.len(), 1);
        assert_eq!(it.logs[0].topics[0], NATIVE_AMM_TOPIC_ADD_LIQUIDITY);

        let ra = u128::from_be_bytes(state.get_contract_storage(&contract, &reserve_a_key())[16..32].try_into().unwrap());
        let rb = u128::from_be_bytes(state.get_contract_storage(&contract, &reserve_b_key())[16..32].try_into().unwrap());
        assert_eq!(ra, 1_000);
        assert_eq!(rb, 2_000);

        let dx = 100u128;
        let dy = constant_product_amount_out_after_fee(1_000u64, 2_000u64, dx as u64);
        let swap_calldata = encode_swap_calldata(0, dx, u128::from(dy));
        let mut it2 = Interpreter::new(code, 5_000_000);
        it2.run(sender, contract, &swap_calldata, &mut state).unwrap();
        assert_eq!(it2.logs.len(), 1);
        assert_eq!(it2.logs[0].topics[0], NATIVE_AMM_TOPIC_SWAP);

        let ra2 = u128::from_be_bytes(state.get_contract_storage(&contract, &reserve_a_key())[16..32].try_into().unwrap());
        let rb2 = u128::from_be_bytes(state.get_contract_storage(&contract, &reserve_b_key())[16..32].try_into().unwrap());
        assert_eq!(ra2, 1_000 + dx);
        assert_eq!(rb2, 2_000 - u128::from(dy));
    }

    #[test]
    fn swap_slippage_revert_emits_no_log() {
        let ra = 1000u64;
        let rb = 2000u64;
        let dx = 100u64;
        let dy = constant_product_amount_out_after_fee(ra, rb, dx);
        let sender = AccountId([0xeeu8; 32]);
        let contract = AccountId([0xffu8; 32]);
        let mut state = StateStore::new();
        state.insert(Account {
            id: contract,
            state: Default::default(),
        });
        state.merge_contract_storage(contract, reserve_a_key(), amount_word(u128::from(ra)));
        state.merge_contract_storage(contract, reserve_b_key(), amount_word(u128::from(rb)));

        let code = constant_product_pool_bytecode();
        let calldata = encode_swap_calldata(0, u128::from(dx), u128::from(dy + 1));
        let mut it = Interpreter::new(code, 5_000_000);
        it.run(sender, contract, &calldata, &mut state).unwrap();
        assert!(it.logs.is_empty());
    }

    /// `boing_qa` / `boing_qaCheck` must not reject canonical pool bytecode (checklist **A1.4**).
    #[test]
    fn constant_product_pool_bytecode_passes_protocol_qa() {
        use boing_qa::{check_contract_deploy_full, QaResult, RuleRegistry};

        let code = constant_product_pool_bytecode();
        let registry = RuleRegistry::new();
        let r = check_contract_deploy_full(&code, Some("dapp"), None, &registry);
        assert!(
            matches!(r, QaResult::Allow | QaResult::Unsure),
            "expected Allow or Unsure for native CP pool bytecode, got {r:?}"
        );
    }

    #[test]
    fn constant_product_pool_bytecode_v2_passes_protocol_qa() {
        use boing_qa::{check_contract_deploy_full, QaResult, RuleRegistry};

        let code = constant_product_pool_bytecode_v2();
        let registry = RuleRegistry::new();
        let r = check_contract_deploy_full(&code, Some("dapp"), None, &registry);
        assert!(
            matches!(r, QaResult::Allow | QaResult::Unsure),
            "expected Allow or Unsure for native CP pool v2 bytecode, got {r:?}"
        );
    }

    #[test]
    fn v2_swap_calls_output_token_reference_contract() {
        use crate::reference_token::smoke_contract_bytecode;

        let sender = AccountId([0xabu8; 32]);
        let pool = AccountId([0xcd; 32]);
        let token_b = AccountId([0x77; 32]);
        let token_a_zero = AccountId([0u8; 32]);

        let mut state = StateStore::new();
        for id in [sender, pool, token_b] {
            state.insert(Account {
                id,
                state: boing_primitives::AccountState::default(),
            });
        }
        state.set_contract_code(token_b, smoke_contract_bytecode());
        state.set_contract_code(pool, constant_product_pool_bytecode_v2());
        state.merge_contract_storage(pool, reserve_a_key(), amount_word(1_000));
        state.merge_contract_storage(pool, reserve_b_key(), amount_word(2_000));

        let pool_code = state.get_contract_code(&pool).unwrap().clone();
        let set_cd = encode_set_tokens_calldata(&token_a_zero, &token_b);
        let mut it = Interpreter::new(pool_code.clone(), 10_000_000);
        it.run(sender, pool, &set_cd, &mut state).unwrap();

        let dx = 100u64;
        let dy = constant_product_amount_out_after_fee(1_000, 2_000, dx);
        let swap_cd = encode_swap_calldata(0, u128::from(dx), u128::from(dy));
        let mut it2 = Interpreter::new(pool_code, 10_000_000);
        it2.run(sender, pool, &swap_cd, &mut state).unwrap();

        assert_eq!(it2.logs.len(), 2, "pool Log2 + token Log0");
        let smoke_key = [1u8; 32];
        assert_eq!(state.get_contract_storage(&token_b, &smoke_key), pool.0);
    }

    #[test]
    fn constant_product_pool_bytecode_v3_passes_protocol_qa() {
        use boing_qa::{check_contract_deploy_full, QaResult, RuleRegistry};

        let code = constant_product_pool_bytecode_v3();
        let registry = RuleRegistry::new();
        let r = check_contract_deploy_full(&code, Some("dapp"), None, &registry);
        assert!(
            matches!(r, QaResult::Allow | QaResult::Unsure),
            "expected Allow or Unsure for native CP pool v3 bytecode, got {r:?}"
        );
    }

    #[test]
    fn constant_product_pool_bytecode_v4_passes_protocol_qa() {
        use boing_qa::{check_contract_deploy_full, QaResult, RuleRegistry};

        let code = constant_product_pool_bytecode_v4();
        let registry = RuleRegistry::new();
        let r = check_contract_deploy_full(&code, Some("dapp"), None, &registry);
        assert!(
            matches!(r, QaResult::Allow | QaResult::Unsure),
            "expected Allow or Unsure for native CP pool v4 bytecode, got {r:?}"
        );
    }

    #[test]
    fn constant_product_pool_bytecode_v5_passes_protocol_qa() {
        use boing_qa::{check_contract_deploy_full, QaResult, RuleRegistry};

        let code = constant_product_pool_bytecode_v5();
        let registry = RuleRegistry::new();
        let r = check_contract_deploy_full(&code, Some("dapp"), None, &registry);
        assert!(
            matches!(r, QaResult::Allow | QaResult::Unsure),
            "expected Allow or Unsure for native CP pool v5 bytecode, got {r:?}"
        );
    }

    /// **v5:** `swap_to` pays reference output to **recipient** word, not the immediate caller (router pattern).
    #[test]
    fn v5_swap_to_routes_token_payout_to_recipient() {
        use crate::reference_token::{
            encode_mint_first_calldata, reference_fungible_template_bytecode, REF_FUNGIBLE_BALANCE_XOR,
        };

        fn ref_balance_key(holder: &AccountId) -> [u8; 32] {
            let mut k = holder.0;
            for i in 0..32 {
                k[i] ^= REF_FUNGIBLE_BALANCE_XOR[i];
            }
            k
        }
        fn word128_tail(w: &[u8; 32]) -> u128 {
            u128::from_be_bytes(w[16..32].try_into().unwrap())
        }

        let user = AccountId([0xabu8; 32]);
        let router = AccountId([0xeeu8; 32]);
        let pool = AccountId([0xcd; 32]);
        let token_b = AccountId([0x77; 32]);
        let token_a_zero = AccountId([0u8; 32]);

        let mut state = StateStore::new();
        for id in [user, router, pool, token_b] {
            state.insert(Account {
                id,
                state: boing_primitives::AccountState::default(),
            });
        }
        let tok_code = reference_fungible_template_bytecode();
        state.set_contract_code(token_b, tok_code.clone());
        state.set_contract_code(pool, constant_product_pool_bytecode_v5());
        state.merge_contract_storage(pool, reserve_a_key(), amount_word(1_000));
        state.merge_contract_storage(pool, reserve_b_key(), amount_word(2_000));

        let mut it_mint = Interpreter::new(tok_code.clone(), 10_000_000);
        it_mint
            .run(
                user,
                token_b,
                &encode_mint_first_calldata(&pool, 1_000_000),
                &mut state,
            )
            .unwrap();

        let pool_code = state.get_contract_code(&pool).unwrap().clone();
        let set_cd = encode_set_tokens_calldata(&token_a_zero, &token_b);
        let mut it = Interpreter::new(pool_code.clone(), 10_000_000);
        it.run(user, pool, &set_cd, &mut state).unwrap();

        let dx = 100u64;
        let dy = constant_product_amount_out_after_fee(1_000, 2_000, dx);
        assert_eq!(word128_tail(&state.get_contract_storage(&token_b, &ref_balance_key(&user))), 0);

        let swap_cd = encode_swap_to_calldata(0, u128::from(dx), u128::from(dy), &user);
        let mut it2 = Interpreter::new(pool_code, 10_000_000);
        it2.run(router, pool, &swap_cd, &mut state).unwrap();

        assert_eq!(
            word128_tail(&state.get_contract_storage(&token_b, &ref_balance_key(&user))),
            u128::from(dy),
            "recipient must receive on-chain token B, not router"
        );
        assert_eq!(
            word128_tail(&state.get_contract_storage(&token_b, &ref_balance_key(&router))),
            0
        );
    }

    /// **v5:** `remove_liquidity_to` sends reference token **B** to **recipient_b**, not the caller.
    #[test]
    fn v5_remove_liquidity_to_routes_token_b_payout_to_recipient() {
        use crate::reference_token::{
            encode_mint_first_calldata, reference_fungible_template_bytecode, REF_FUNGIBLE_BALANCE_XOR,
        };

        fn ref_balance_key(holder: &AccountId) -> [u8; 32] {
            let mut k = holder.0;
            for i in 0..32 {
                k[i] ^= REF_FUNGIBLE_BALANCE_XOR[i];
            }
            k
        }
        fn word128_tail(w: &[u8; 32]) -> u128 {
            u128::from_be_bytes(w[16..32].try_into().unwrap())
        }

        let user = AccountId([0xabu8; 32]);
        let beneficiary = AccountId([0x22u8; 32]);
        let pool = AccountId([0xcd; 32]);
        let token_b = AccountId([0x77; 32]);
        let token_a_zero = AccountId([0u8; 32]);

        let mut state = StateStore::new();
        for id in [user, beneficiary, pool, token_b] {
            state.insert(Account {
                id,
                state: boing_primitives::AccountState::default(),
            });
        }
        let tok_code = reference_fungible_template_bytecode();
        state.set_contract_code(token_b, tok_code.clone());
        state.set_contract_code(pool, constant_product_pool_bytecode_v5());

        let mut it_mint = Interpreter::new(tok_code.clone(), 10_000_000);
        it_mint
            .run(
                user,
                token_b,
                &encode_mint_first_calldata(&pool, 10_000_000),
                &mut state,
            )
            .unwrap();

        let pool_code = state.get_contract_code(&pool).unwrap().clone();
        let set_cd = encode_set_tokens_calldata(&token_a_zero, &token_b);
        let mut it = Interpreter::new(pool_code.clone(), 10_000_000);
        it.run(user, pool, &set_cd, &mut state).unwrap();

        let mut it_add = Interpreter::new(pool_code.clone(), 10_000_000);
        it_add
            .run(
                user,
                pool,
                &encode_add_liquidity_calldata(1_000, 2_000, 0),
                &mut state,
            )
            .unwrap();

        let lp_total =
            u128::from_be_bytes(state.get_contract_storage(&pool, &total_lp_supply_key())[16..32].try_into().unwrap());
        assert_eq!(lp_total, 1_000);

        let burn = 500u128;
        let min_a = 0u128;
        let min_b = 0u128;
        let rm_cd = encode_remove_liquidity_to_calldata(burn, min_a, min_b, &user, &beneficiary);
        let mut it_rm = Interpreter::new(pool_code, 10_000_000);
        it_rm.run(user, pool, &rm_cd, &mut state).unwrap();

        let exp_b_out = 1_000u128;
        assert_eq!(
            word128_tail(&state.get_contract_storage(&token_b, &ref_balance_key(&beneficiary))),
            exp_b_out,
            "recipient_b must receive on-chain token B"
        );
        assert_eq!(word128_tail(&state.get_contract_storage(&token_b, &ref_balance_key(&user))), 0);
    }

    /// **v3:** Storage [`swap_fee_bps_key`] + seeded reserves — swap uses on-chain **fee_bps** (here **100** = 1 %).
    #[test]
    fn v3_storage_swap_fee_bps_matches_wide_quote() {
        let ra = 1000u64;
        let rb = 2000u64;
        let dx = 100u64;
        let fee_bps: u16 = 100;
        let dy = constant_product_amount_out_after_fee_with_bps(ra, rb, dx, fee_bps);

        let sender = AccountId([0x22u8; 32]);
        let pool = AccountId([0x33u8; 32]);
        let mut state = StateStore::new();
        state.insert(Account {
            id: pool,
            state: Default::default(),
        });
        state.set_contract_code(pool, constant_product_pool_bytecode_v3());
        state.merge_contract_storage(pool, reserve_a_key(), amount_word(u128::from(ra)));
        state.merge_contract_storage(pool, reserve_b_key(), amount_word(u128::from(rb)));
        state.merge_contract_storage(pool, swap_fee_bps_key(), amount_word(u128::from(fee_bps)));

        let swap_cd = encode_swap_calldata(0, u128::from(dx), u128::from(dy));
        let mut it = Interpreter::new(constant_product_pool_bytecode_v3(), 5_000_000);
        it.run(sender, pool, &swap_cd, &mut state).unwrap();

        assert_eq!(it.logs.len(), 1);
        assert_eq!(&it.logs[0].data[80..96], &amount_word(u128::from(dy))[16..32]);

        let ra2 = u128::from_be_bytes(state.get_contract_storage(&pool, &reserve_a_key())[16..32].try_into().unwrap());
        let rb2 = u128::from_be_bytes(state.get_contract_storage(&pool, &reserve_b_key())[16..32].try_into().unwrap());
        assert_eq!(ra2, u128::from(ra + dx));
        assert_eq!(rb2, u128::from(rb - dy));
    }

    /// **`set_swap_fee_bps` calldata** persists fee before first `add_liquidity` (total LP **0**).
    #[test]
    fn v3_set_swap_fee_calldata_writes_storage() {
        let fee_bps: u16 = 50;
        let sender = AccountId([0x44u8; 32]);
        let pool = AccountId([0x55u8; 32]);
        let mut state = StateStore::new();
        state.insert(Account {
            id: pool,
            state: Default::default(),
        });
        state.set_contract_code(pool, constant_product_pool_bytecode_v3());

        let cd = encode_set_swap_fee_bps_calldata(u128::from(fee_bps));
        let mut it = Interpreter::new(constant_product_pool_bytecode_v3(), 5_000_000);
        it.run(sender, pool, &cd, &mut state).unwrap();

        let got = state.get_contract_storage(&pool, &swap_fee_bps_key());
        assert_eq!(u128::from_be_bytes(got[16..32].try_into().unwrap()), u128::from(fee_bps));
    }

    /// **v3:** `set_swap_fee_bps` → `add_liquidity` → `swap` end-to-end.
    #[test]
    fn v3_set_fee_add_liquidity_swap_round_trip() {
        let ra = 1000u64;
        let rb = 2000u64;
        let dx = 100u64;
        let fee_bps: u16 = 100;
        let dy = constant_product_amount_out_after_fee_with_bps(ra, rb, dx, fee_bps);

        let sender = AccountId([0x66u8; 32]);
        let pool = AccountId([0x77u8; 32]);
        let mut state = StateStore::new();
        state.insert(Account {
            id: pool,
            state: Default::default(),
        });
        let pool_code = constant_product_pool_bytecode_v3();
        state.set_contract_code(pool, pool_code.clone());

        let mut it0 = Interpreter::new(pool_code.clone(), 10_000_000);
        it0
            .run(sender, pool, &encode_set_swap_fee_bps_calldata(u128::from(fee_bps)), &mut state)
            .unwrap();

        let mut it1 = Interpreter::new(pool_code.clone(), 10_000_000);
        it1
            .run(sender, pool, &encode_add_liquidity_calldata(u128::from(ra), u128::from(rb), 0), &mut state)
            .unwrap();

        let mut it2 = Interpreter::new(pool_code, 10_000_000);
        it2
            .run(sender, pool, &encode_swap_calldata(0, u128::from(dx), u128::from(dy)), &mut state)
            .unwrap();

        assert_eq!(it2.logs.len(), 1);
        assert_eq!(&it2.logs[0].data[80..96], &amount_word(u128::from(dy))[16..32]);
    }
}
