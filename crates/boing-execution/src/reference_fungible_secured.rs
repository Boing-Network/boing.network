//! **Secured** reference fungible: `0xFD` **init** + runtime enforcing deny-list, pause, caps,
//! anti-bot window, per-sender cooldown, and optional transfer-unlock height.
//!
//! Calldata matches [`reference_token`](crate::reference_token) for **`transfer`** / **`mint_first`** (96 bytes).
//! Admin selectors (same layout): **`0x03`** set deny, **`0x04`** set pause, **`0x05`** renounce admin,
//! **`0x07`** set transfer-unlock height (u64 in amount word low 8 bytes).
//!
//! See `docs/BOING-REFERENCE-TOKEN.md`.

use boing_primitives::CONTRACT_DEPLOY_INIT_CODE_MARKER;

use crate::bytecode::Opcode;
use crate::reference_token::{amount_word, selector_word, SELECTOR_MINT_FIRST, SELECTOR_TRANSFER};

/// Admin: deny / freeze `to` (amount word low byte zero = clear, non-zero = set).
pub const SELECTOR_SET_DENY: u8 = 0x03;
/// Admin: pause (amount all-zero = unpause, else pause).
pub const SELECTOR_SET_PAUSE: u8 = 0x04;
/// Admin: clear admin (irreversible).
pub const SELECTOR_RENOUNCE_ADMIN: u8 = 0x05;
/// Admin: minimum block height before transfers (u64 BE in amount low 8 bytes). Requires [`FLAG_TRANSFER_UNLOCK`].
pub const SELECTOR_SET_TRANSFER_UNLOCK: u8 = 0x07;

pub const REF_SECURED_BALANCE_XOR: [u8; 32] =
    *b"BOING_REFSECURED_BAL___\x00\x00\x00\x00\x00\x00\x00\x00\x00";
pub const REF_SECURED_DENY_XOR: [u8; 32] =
    *b"BOING_REFSECURED_DENY__\x00\x00\x00\x00\x00\x00\x00\x00\x00";
pub const REF_SECURED_COOLDOWN_XOR: [u8; 32] =
    *b"BOING_REFSECURED_COOL__\x00\x00\x00\x00\x00\x00\x00\x00\x00";

#[must_use]
pub fn ref_sec_admin_key() -> [u8; 32] {
    let mut k = [0u8; 32];
    k[31] = 0xc1;
    k
}
#[must_use]
pub fn ref_sec_mint_once_key() -> [u8; 32] {
    let mut k = [0u8; 32];
    k[31] = 0xc2;
    k
}
#[must_use]
pub fn ref_sec_flags_key() -> [u8; 32] {
    let mut k = [0u8; 32];
    k[31] = 0xc3;
    k
}
#[must_use]
pub fn ref_sec_max_tx_key() -> [u8; 32] {
    let mut k = [0u8; 32];
    k[31] = 0xc4;
    k
}
#[must_use]
pub fn ref_sec_max_wallet_key() -> [u8; 32] {
    let mut k = [0u8; 32];
    k[31] = 0xc5;
    k
}
#[must_use]
pub fn ref_sec_paused_key() -> [u8; 32] {
    let mut k = [0u8; 32];
    k[31] = 0xc6;
    k
}
#[must_use]
pub fn ref_sec_anti_bot_end_key() -> [u8; 32] {
    let mut k = [0u8; 32];
    k[31] = 0xc7;
    k
}
#[must_use]
pub fn ref_sec_anti_bot_max_key() -> [u8; 32] {
    let mut k = [0u8; 32];
    k[31] = 0xc8;
    k
}
#[must_use]
pub fn ref_sec_cooldown_secs_key() -> [u8; 32] {
    let mut k = [0u8; 32];
    k[31] = 0xc9;
    k
}
#[must_use]
pub fn ref_sec_xfer_unlock_key() -> [u8; 32] {
    let mut k = [0u8; 32];
    k[31] = 0xca;
    k
}

pub const FLAG_DENYLIST: u32 = 0x01;
pub const FLAG_MAX_TX: u32 = 0x02;
pub const FLAG_MAX_WALLET: u32 = 0x04;
pub const FLAG_ANTI_BOT: u32 = 0x08;
pub const FLAG_COOLDOWN: u32 = 0x10;
pub const FLAG_NO_MINT: u32 = 0x20;
pub const FLAG_TRANSFER_UNLOCK: u32 = 0x40;

/// Baked into init bytecode; see flag constants.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceFungibleSecuredConfig {
    pub flags: u32,
    pub max_tx: u128,
    pub max_wallet: u128,
    pub anti_bot_extra_blocks: u64,
    pub anti_bot_max_amount: u128,
    pub cooldown_secs: u64,
    pub transfer_unlock_height: u64,
    pub initial_paused: bool,
}

impl Default for ReferenceFungibleSecuredConfig {
    fn default() -> Self {
        Self {
            flags: 0,
            max_tx: 0,
            max_wallet: 0,
            anti_bot_extra_blocks: 0,
            anti_bot_max_amount: 0,
            cooldown_secs: 0,
            transfer_unlock_height: 0,
            initial_paused: false,
        }
    }
}

impl ReferenceFungibleSecuredConfig {
    /// Pinned SDK / docs default: enforcement flags off (admin set in init; same rough UX as minimal fungible).
    #[must_use]
    pub fn pinned_public_default() -> Self {
        Self::default()
    }
}

const MEM_SCRATCH_SEL: u64 = 384;
const MEM_SCRATCH_TO: u64 = 352;
const MEM_SCRATCH_AMT: u64 = 416;
const MEM_OLD_BAL: u64 = 448;
const MEM_BAL_TO: u64 = 480;
const MEM_NEW_TO: u64 = 512;
const MEM_NEW_C: u64 = 544;
const MEM_KEY_CALLER: u64 = 576;
const MEM_KEY_TO: u64 = 608;
const MEM_TMP_A: u64 = 640;
const MEM_TMP_B: u64 = 672;

fn push32(code: &mut Vec<u8>, w: &[u8; 32]) {
    code.push(Opcode::Push32 as u8);
    code.extend_from_slice(w);
}

fn word_u64(n: u64) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[24..32].copy_from_slice(&n.to_be_bytes());
    w
}

fn word_u32(n: u32) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[28..32].copy_from_slice(&n.to_be_bytes());
    w
}

fn patch_push32_dest(code: &mut [u8], push32_opcode_at: usize, dest: usize) {
    code[push32_opcode_at + 1..push32_opcode_at + 33].copy_from_slice(&word_u64(dest as u64));
}

fn mask_low_byte() -> [u8; 32] {
    let mut m = [0u8; 32];
    m[31] = 0xff;
    m
}

fn word_one() -> [u8; 32] {
    let mut w = [0u8; 32];
    w[31] = 1;
    w
}

fn word_zero() -> [u8; 32] {
    [0u8; 32]
}

/// `JumpI` to patched dest if `(flags & mask) == 0` (skip guarded block when flag off).
fn emit_skip_unless_flag(code: &mut Vec<u8>, mask: u32) -> usize {
    push32(code, &ref_sec_flags_key());
    code.push(Opcode::SLoad as u8);
    push32(code, &word_u32(mask));
    code.push(Opcode::And as u8);
    code.push(Opcode::IsZero as u8);
    let at = code.len();
    push32(code, &[0u8; 32]);
    code.push(Opcode::JumpI as u8);
    at
}

/// Runtime only (stored after init `RETURN`).
#[must_use]
pub fn reference_fungible_secured_runtime_bytecode() -> Vec<u8> {
    let mut c = Vec::new();

    push32(&mut c, &word_u64(0));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &mask_low_byte());
    c.push(Opcode::And as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_SEL));
    c.push(Opcode::MStore as u8);

    // ----- mint_first -----
    push32(&mut c, &word_u64(MEM_SCRATCH_SEL));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &selector_word(SELECTOR_MINT_FIRST));
    c.push(Opcode::Eq as u8);
    c.push(Opcode::IsZero as u8);
    let fix_skip_mint = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    let p_nm = emit_skip_unless_flag(&mut c, FLAG_NO_MINT);
    c.push(Opcode::Stop as u8);
    let after_nm = c.len();
    patch_push32_dest(&mut c, p_nm, after_nm);

    push32(&mut c, &ref_sec_mint_once_key());
    c.push(Opcode::SLoad as u8);
    c.push(Opcode::IsZero as u8);
    c.push(Opcode::IsZero as u8);
    let fix_mint_used = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    c.push(Opcode::Caller as u8);
    push32(&mut c, &ref_sec_admin_key());
    c.push(Opcode::SLoad as u8);
    c.push(Opcode::Eq as u8);
    c.push(Opcode::IsZero as u8);
    let fix_na = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(&mut c, &word_u64(32));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_TO));
    c.push(Opcode::MStore as u8);
    push32(&mut c, &word_u64(64));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_AMT));
    c.push(Opcode::MStore as u8);

    push32(&mut c, &word_u64(MEM_SCRATCH_AMT));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::IsZero as u8);
    let fix_mz = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(&mut c, &word_u64(MEM_SCRATCH_TO));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &REF_SECURED_BALANCE_XOR);
    c.push(Opcode::Xor as u8);
    c.push(Opcode::SLoad as u8);
    push32(&mut c, &word_u64(MEM_OLD_BAL));
    c.push(Opcode::MStore as u8);

    push32(&mut c, &word_u64(MEM_SCRATCH_AMT));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_OLD_BAL));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Add as u8);
    push32(&mut c, &word_u64(MEM_NEW_TO));
    c.push(Opcode::MStore as u8);

    push32(&mut c, &word_u64(MEM_NEW_TO));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_OLD_BAL));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Lt as u8);
    let fix_mov = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(&mut c, &word_u64(MEM_NEW_TO));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_TO));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &REF_SECURED_BALANCE_XOR);
    c.push(Opcode::Xor as u8);
    c.push(Opcode::SStore as u8);

    push32(&mut c, &word_one());
    push32(&mut c, &ref_sec_mint_once_key());
    c.push(Opcode::SStore as u8);
    c.push(Opcode::Stop as u8);

    let mint_abort = c.len();
    patch_push32_dest(&mut c, fix_mint_used, mint_abort);
    patch_push32_dest(&mut c, fix_na, mint_abort);
    patch_push32_dest(&mut c, fix_mz, mint_abort);
    patch_push32_dest(&mut c, fix_mov, mint_abort);
    c.push(Opcode::Stop as u8);

    let after_mint = c.len();
    patch_push32_dest(&mut c, fix_skip_mint, after_mint);

    // ----- transfer -----
    push32(&mut c, &word_u64(MEM_SCRATCH_SEL));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &selector_word(SELECTOR_TRANSFER));
    c.push(Opcode::Eq as u8);
    c.push(Opcode::IsZero as u8);
    let fix_skip_xfer = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(&mut c, &ref_sec_paused_key());
    c.push(Opcode::SLoad as u8);
    c.push(Opcode::IsZero as u8);
    let fj_pause = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    c.push(Opcode::Stop as u8);
    let xfer_go = c.len();
    patch_push32_dest(&mut c, fj_pause, xfer_go);

    let pd = emit_skip_unless_flag(&mut c, FLAG_DENYLIST);
    c.push(Opcode::Caller as u8);
    push32(&mut c, &REF_SECURED_DENY_XOR);
    c.push(Opcode::Xor as u8);
    c.push(Opcode::SLoad as u8);
    c.push(Opcode::IsZero as u8);
    let okc = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    c.push(Opcode::Stop as u8);
    let ad = c.len();
    patch_push32_dest(&mut c, okc, ad);
    patch_push32_dest(&mut c, pd, ad);

    push32(&mut c, &word_u64(64));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::IsZero as u8);
    let fxz = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(&mut c, &word_u64(32));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_TO));
    c.push(Opcode::MStore as u8);
    push32(&mut c, &word_u64(64));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_AMT));
    c.push(Opcode::MStore as u8);

    let pd2 = emit_skip_unless_flag(&mut c, FLAG_DENYLIST);
    push32(&mut c, &word_u64(MEM_SCRATCH_TO));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &REF_SECURED_DENY_XOR);
    c.push(Opcode::Xor as u8);
    c.push(Opcode::SLoad as u8);
    c.push(Opcode::IsZero as u8);
    let okt = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    c.push(Opcode::Stop as u8);
    let ad2 = c.len();
    patch_push32_dest(&mut c, okt, ad2);
    patch_push32_dest(&mut c, pd2, ad2);

    let pu = emit_skip_unless_flag(&mut c, FLAG_TRANSFER_UNLOCK);
    c.push(Opcode::BlockHeight as u8);
    push32(&mut c, &ref_sec_xfer_unlock_key());
    c.push(Opcode::SLoad as u8);
    c.push(Opcode::Lt as u8);
    let uok = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    c.push(Opcode::Stop as u8);
    let au = c.len();
    patch_push32_dest(&mut c, uok, au);
    patch_push32_dest(&mut c, pu, au);

    // anti-bot: if height < end_block then amount <= anti_bot_max
    let pab = emit_skip_unless_flag(&mut c, FLAG_ANTI_BOT);
    c.push(Opcode::BlockHeight as u8);
    push32(&mut c, &ref_sec_anti_bot_end_key());
    c.push(Opcode::SLoad as u8);
    c.push(Opcode::Lt as u8);
    let ab_need = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    let ab_skip = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::Jump as u8);
    let ab_cap = c.len();
    patch_push32_dest(&mut c, ab_need, ab_cap);
    push32(&mut c, &word_u64(MEM_SCRATCH_AMT));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &ref_sec_anti_bot_max_key());
    c.push(Opcode::SLoad as u8);
    c.push(Opcode::Gt as u8);
    c.push(Opcode::IsZero as u8);
    let ab_ok = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    c.push(Opcode::Stop as u8);
    let ab_after = c.len();
    patch_push32_dest(&mut c, ab_ok, ab_after);
    patch_push32_dest(&mut c, ab_skip, ab_after);
    patch_push32_dest(&mut c, pab, ab_after);

    let pmt = emit_skip_unless_flag(&mut c, FLAG_MAX_TX);
    push32(&mut c, &word_u64(MEM_SCRATCH_AMT));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &ref_sec_max_tx_key());
    c.push(Opcode::SLoad as u8);
    c.push(Opcode::Gt as u8);
    c.push(Opcode::IsZero as u8);
    let mt_ok = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    c.push(Opcode::Stop as u8);
    let amtx = c.len();
    patch_push32_dest(&mut c, mt_ok, amtx);
    patch_push32_dest(&mut c, pmt, amtx);

    let pcd = emit_skip_unless_flag(&mut c, FLAG_COOLDOWN);
    c.push(Opcode::Caller as u8);
    push32(&mut c, &REF_SECURED_COOLDOWN_XOR);
    c.push(Opcode::Xor as u8);
    c.push(Opcode::SLoad as u8);
    push32(&mut c, &word_u64(MEM_TMP_A));
    c.push(Opcode::MStore as u8);
    push32(&mut c, &word_u64(MEM_TMP_A));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::IsZero as u8);
    let cd_first = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    push32(&mut c, &word_u64(MEM_TMP_A));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &ref_sec_cooldown_secs_key());
    c.push(Opcode::SLoad as u8);
    c.push(Opcode::Add as u8);
    push32(&mut c, &word_u64(MEM_TMP_B));
    c.push(Opcode::MStore as u8);
    c.push(Opcode::Timestamp as u8);
    push32(&mut c, &word_u64(MEM_TMP_B));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Lt as u8);
    c.push(Opcode::IsZero as u8);
    let cd_ok = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    c.push(Opcode::Stop as u8);
    let acd = c.len();
    patch_push32_dest(&mut c, cd_ok, acd);
    let acd2 = c.len();
    patch_push32_dest(&mut c, cd_first, acd2);
    patch_push32_dest(&mut c, pcd, acd2);

    c.push(Opcode::Caller as u8);
    push32(&mut c, &REF_SECURED_BALANCE_XOR);
    c.push(Opcode::Xor as u8);
    push32(&mut c, &word_u64(MEM_KEY_CALLER));
    c.push(Opcode::MStore as u8);

    push32(&mut c, &word_u64(MEM_SCRATCH_TO));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &REF_SECURED_BALANCE_XOR);
    c.push(Opcode::Xor as u8);
    push32(&mut c, &word_u64(MEM_KEY_TO));
    c.push(Opcode::MStore as u8);

    push32(&mut c, &word_u64(MEM_KEY_CALLER));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::SLoad as u8);
    push32(&mut c, &word_u64(MEM_OLD_BAL));
    c.push(Opcode::MStore as u8);

    push32(&mut c, &word_u64(MEM_OLD_BAL));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_AMT));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Lt as u8);
    let fu = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(&mut c, &word_u64(MEM_KEY_TO));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::SLoad as u8);
    push32(&mut c, &word_u64(MEM_BAL_TO));
    c.push(Opcode::MStore as u8);

    push32(&mut c, &word_u64(MEM_BAL_TO));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_AMT));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Add as u8);
    push32(&mut c, &word_u64(MEM_NEW_TO));
    c.push(Opcode::MStore as u8);

    let pmw = emit_skip_unless_flag(&mut c, FLAG_MAX_WALLET);
    push32(&mut c, &word_u64(MEM_NEW_TO));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &ref_sec_max_wallet_key());
    c.push(Opcode::SLoad as u8);
    c.push(Opcode::Gt as u8);
    c.push(Opcode::IsZero as u8);
    let mw_ok = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    c.push(Opcode::Stop as u8);
    let amw = c.len();
    patch_push32_dest(&mut c, mw_ok, amw);
    patch_push32_dest(&mut c, pmw, amw);

    push32(&mut c, &word_u64(MEM_NEW_TO));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_BAL_TO));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Lt as u8);
    let fo = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);

    push32(&mut c, &word_u64(MEM_OLD_BAL));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_SCRATCH_AMT));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::Sub as u8);
    push32(&mut c, &word_u64(MEM_NEW_C));
    c.push(Opcode::MStore as u8);

    push32(&mut c, &word_u64(MEM_NEW_C));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_KEY_CALLER));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::SStore as u8);

    push32(&mut c, &word_u64(MEM_NEW_TO));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &word_u64(MEM_KEY_TO));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::SStore as u8);

    let p8 = emit_skip_unless_flag(&mut c, FLAG_COOLDOWN);
    c.push(Opcode::Timestamp as u8);
    c.push(Opcode::Caller as u8);
    push32(&mut c, &REF_SECURED_COOLDOWN_XOR);
    c.push(Opcode::Xor as u8);
    c.push(Opcode::SStore as u8);
    let a8 = c.len();
    patch_push32_dest(&mut c, p8, a8);

    c.push(Opcode::Stop as u8);

    let xab = c.len();
    patch_push32_dest(&mut c, fu, xab);
    patch_push32_dest(&mut c, fo, xab);
    c.push(Opcode::Stop as u8);

    let xzd = c.len();
    patch_push32_dest(&mut c, fxz, xzd);
    c.push(Opcode::Stop as u8);

    // Not `transfer`: fall through to admin handlers (do not STOP here — that would skip admin ops).
    let after_xfer_dispatch = c.len();
    patch_push32_dest(&mut c, fix_skip_xfer, after_xfer_dispatch);

    // ----- admin: set_deny -----
    push32(&mut c, &word_u64(MEM_SCRATCH_SEL));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &selector_word(SELECTOR_SET_DENY));
    c.push(Opcode::Eq as u8);
    c.push(Opcode::IsZero as u8);
    let sd0 = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    c.push(Opcode::Caller as u8);
    push32(&mut c, &ref_sec_admin_key());
    c.push(Opcode::SLoad as u8);
    c.push(Opcode::Eq as u8);
    c.push(Opcode::IsZero as u8);
    let sd1 = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    push32(&mut c, &word_u64(64));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::IsZero as u8);
    let sd_clr = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    push32(&mut c, &word_one());
    let sd_j = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::Jump as u8);
    let sd_clear_val = c.len();
    patch_push32_dest(&mut c, sd_clr, sd_clear_val);
    push32(&mut c, &word_zero());
    let sd_join = c.len();
    patch_push32_dest(&mut c, sd_j, sd_join);
    push32(&mut c, &word_u64(32));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &REF_SECURED_DENY_XOR);
    c.push(Opcode::Xor as u8);
    c.push(Opcode::SStore as u8);
    c.push(Opcode::Stop as u8);
    let sd_ab = c.len();
    patch_push32_dest(&mut c, sd1, sd_ab);
    c.push(Opcode::Stop as u8);
    let sd_a2 = c.len();
    patch_push32_dest(&mut c, sd0, sd_a2);

    // ----- set_pause -----
    push32(&mut c, &word_u64(MEM_SCRATCH_SEL));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &selector_word(SELECTOR_SET_PAUSE));
    c.push(Opcode::Eq as u8);
    c.push(Opcode::IsZero as u8);
    let sp0 = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    c.push(Opcode::Caller as u8);
    push32(&mut c, &ref_sec_admin_key());
    c.push(Opcode::SLoad as u8);
    c.push(Opcode::Eq as u8);
    c.push(Opcode::IsZero as u8);
    let sp1 = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    push32(&mut c, &word_u64(64));
    c.push(Opcode::MLoad as u8);
    c.push(Opcode::IsZero as u8);
    let sp_u = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    push32(&mut c, &word_one());
    let sp_j = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::Jump as u8);
    let sp_unpause_val = c.len();
    patch_push32_dest(&mut c, sp_u, sp_unpause_val);
    push32(&mut c, &word_zero());
    let sp_join = c.len();
    patch_push32_dest(&mut c, sp_j, sp_join);
    push32(&mut c, &ref_sec_paused_key());
    c.push(Opcode::SStore as u8);
    c.push(Opcode::Stop as u8);
    let sp_ab = c.len();
    patch_push32_dest(&mut c, sp1, sp_ab);
    c.push(Opcode::Stop as u8);
    let sp_a2 = c.len();
    patch_push32_dest(&mut c, sp0, sp_a2);

    // ----- renounce admin -----
    push32(&mut c, &word_u64(MEM_SCRATCH_SEL));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &selector_word(SELECTOR_RENOUNCE_ADMIN));
    c.push(Opcode::Eq as u8);
    c.push(Opcode::IsZero as u8);
    let r0 = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    c.push(Opcode::Caller as u8);
    push32(&mut c, &ref_sec_admin_key());
    c.push(Opcode::SLoad as u8);
    c.push(Opcode::Eq as u8);
    c.push(Opcode::IsZero as u8);
    let r1 = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    push32(&mut c, &word_zero());
    push32(&mut c, &ref_sec_admin_key());
    c.push(Opcode::SStore as u8);
    c.push(Opcode::Stop as u8);
    let r_ab = c.len();
    patch_push32_dest(&mut c, r1, r_ab);
    c.push(Opcode::Stop as u8);
    let r_a2 = c.len();
    patch_push32_dest(&mut c, r0, r_a2);

    // ----- set_transfer_unlock -----
    push32(&mut c, &word_u64(MEM_SCRATCH_SEL));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &selector_word(SELECTOR_SET_TRANSFER_UNLOCK));
    c.push(Opcode::Eq as u8);
    c.push(Opcode::IsZero as u8);
    let su0 = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    c.push(Opcode::Caller as u8);
    push32(&mut c, &ref_sec_admin_key());
    c.push(Opcode::SLoad as u8);
    c.push(Opcode::Eq as u8);
    c.push(Opcode::IsZero as u8);
    let su1 = c.len();
    push32(&mut c, &[0u8; 32]);
    c.push(Opcode::JumpI as u8);
    push32(&mut c, &word_u64(64));
    c.push(Opcode::MLoad as u8);
    push32(&mut c, &ref_sec_xfer_unlock_key());
    c.push(Opcode::SStore as u8);
    c.push(Opcode::Stop as u8);
    let su_ab = c.len();
    patch_push32_dest(&mut c, su1, su_ab);
    c.push(Opcode::Stop as u8);
    let su_a2 = c.len();
    patch_push32_dest(&mut c, su0, su_a2);

    c.push(Opcode::Stop as u8);
    c
}

fn emit_runtime_to_memory(code: &mut Vec<u8>, runtime: &[u8]) {
    let mut rt = runtime.to_vec();
    while rt.len() % 32 != 0 {
        rt.push(0);
    }
    let mut off = 0usize;
    for ch in rt.chunks_exact(32) {
        let mut w = [0u8; 32];
        w.copy_from_slice(ch);
        push32(code, &w);
        push32(code, &word_u64(off as u64));
        code.push(Opcode::MStore as u8);
        off += 32;
    }
}

/// Init body only (**without** `0xFD` marker).
#[must_use]
pub fn reference_fungible_secured_init_bytecode(
    config: &ReferenceFungibleSecuredConfig,
    runtime: &[u8],
) -> Vec<u8> {
    let mut c = Vec::new();
    c.push(Opcode::Caller as u8);
    push32(&mut c, &ref_sec_admin_key());
    c.push(Opcode::SStore as u8);

    c.push(Opcode::BlockHeight as u8);
    push32(&mut c, &word_u64(config.anti_bot_extra_blocks));
    c.push(Opcode::Add as u8);
    push32(&mut c, &ref_sec_anti_bot_end_key());
    c.push(Opcode::SStore as u8);

    push32(&mut c, &word_u32(config.flags));
    push32(&mut c, &ref_sec_flags_key());
    c.push(Opcode::SStore as u8);

    push32(&mut c, &amount_word(config.max_tx));
    push32(&mut c, &ref_sec_max_tx_key());
    c.push(Opcode::SStore as u8);

    push32(&mut c, &amount_word(config.max_wallet));
    push32(&mut c, &ref_sec_max_wallet_key());
    c.push(Opcode::SStore as u8);

    push32(&mut c, &amount_word(config.anti_bot_max_amount));
    push32(&mut c, &ref_sec_anti_bot_max_key());
    c.push(Opcode::SStore as u8);

    push32(&mut c, &word_u64(config.cooldown_secs));
    push32(&mut c, &ref_sec_cooldown_secs_key());
    c.push(Opcode::SStore as u8);

    push32(&mut c, &word_u64(config.transfer_unlock_height));
    push32(&mut c, &ref_sec_xfer_unlock_key());
    c.push(Opcode::SStore as u8);

    if config.initial_paused {
        push32(&mut c, &word_one());
    } else {
        push32(&mut c, &word_zero());
    }
    push32(&mut c, &ref_sec_paused_key());
    c.push(Opcode::SStore as u8);

    emit_runtime_to_memory(&mut c, runtime);
    push32(&mut c, &word_u64(runtime.len() as u64));
    push32(&mut c, &word_u64(0));
    c.push(Opcode::Return as u8);
    c.push(Opcode::Stop as u8);
    c
}

/// Full deploy payload: **`0xFD` || init** (init `RETURN`s runtime).
#[must_use]
pub fn reference_fungible_secured_deploy_bytecode(config: &ReferenceFungibleSecuredConfig) -> Vec<u8> {
    let rt = reference_fungible_secured_runtime_bytecode();
    let init = reference_fungible_secured_init_bytecode(config, &rt);
    let mut out = vec![CONTRACT_DEPLOY_INIT_CODE_MARKER];
    out.extend(init);
    out
}

/// [`reference_fungible_secured_deploy_bytecode`] with [`ReferenceFungibleSecuredConfig::pinned_public_default`].
#[must_use]
pub fn reference_fungible_secured_pinned_default_deploy_bytecode() -> Vec<u8> {
    reference_fungible_secured_deploy_bytecode(&ReferenceFungibleSecuredConfig::pinned_public_default())
}

/// Encode admin **set_deny** calldata (`to`, `set_nonzero`).
#[must_use]
pub fn encode_secured_set_deny_calldata(to: &boing_primitives::AccountId, set: bool) -> Vec<u8> {
    let mut v = selector_word(SELECTOR_SET_DENY).to_vec();
    v.extend_from_slice(&to.0);
    v.extend_from_slice(&amount_word(if set { 1 } else { 0 }));
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use boing_primitives::Account;
    use boing_state::StateStore;

    use crate::interpreter::VmExecutionContext;
    use crate::reference_token::encode_mint_first_calldata;
    use crate::reference_token::encode_transfer_calldata;
    use crate::vm::Vm;

    fn xor_bal_key(holder: &boing_primitives::AccountId) -> [u8; 32] {
        let mut k = [0u8; 32];
        for i in 0..32 {
            k[i] = holder.0[i] ^ REF_SECURED_BALANCE_XOR[i];
        }
        k
    }

    #[test]
    fn secured_deploy_bytecode_passes_protocol_qa() {
        let code = reference_fungible_secured_pinned_default_deploy_bytecode();
        use boing_qa::{check_contract_deploy_full, QaResult, RuleRegistry};
        let registry = RuleRegistry::new();
        let r = check_contract_deploy_full(&code, Some("token"), None, &registry);
        assert!(matches!(r, QaResult::Allow), "got {:?}", r);
    }

    #[test]
    fn secured_default_mint_and_transfer_under_vm() {
        use ed25519_dalek::SigningKey;
        use rand::rngs::OsRng;

        let key = SigningKey::generate(&mut OsRng);
        let deployer = boing_primitives::AccountId(key.verifying_key().to_bytes());
        let bob = boing_primitives::AccountId([0xbbu8; 32]);

        let mut state = StateStore::new();
        state.insert(Account {
            id: deployer,
            state: boing_primitives::AccountState {
                balance: 10_000_000,
                nonce: 0,
                stake: 0, ..Default::default() },
        });

        let deploy_bc = reference_fungible_secured_pinned_default_deploy_bytecode();
        let vm = Vm::new();
        let deploy_tx = boing_primitives::Transaction {
            nonce: 0,
            sender: deployer,
            payload: boing_primitives::TransactionPayload::ContractDeployWithPurposeAndMetadata {
                bytecode: deploy_bc,
                purpose_category: "token".to_string(),
                description_hash: None,
                asset_name: None,
                asset_symbol: None,
                create2_salt: None,
            },
            access_list: boing_primitives::AccessList::default(),
        };
        let ctx = VmExecutionContext {
            block_height: 10,
            block_timestamp: 1_700_000_000,
        };
        vm.execute_with_context(&deploy_tx, &mut state, ctx).unwrap();
        let token = boing_primitives::nonce_derived_contract_address(&deployer, 0);

        let call_mint = boing_primitives::Transaction {
            nonce: 1,
            sender: deployer,
            payload: boing_primitives::TransactionPayload::ContractCall {
                contract: token,
                calldata: encode_mint_first_calldata(&deployer, 1_000),
            },
            access_list: boing_primitives::AccessList::default(),
        };
        vm.execute_with_context(&call_mint, &mut state, ctx).unwrap();

        let call_xfer = boing_primitives::Transaction {
            nonce: 2,
            sender: deployer,
            payload: boing_primitives::TransactionPayload::ContractCall {
                contract: token,
                calldata: encode_transfer_calldata(&bob, 100),
            },
            access_list: boing_primitives::AccessList::default(),
        };
        vm.execute_with_context(&call_xfer, &mut state, ctx).unwrap();

        let b_bob = state.get_contract_storage(&token, &xor_bal_key(&bob));
        assert_eq!(&b_bob[16..32], &100u128.to_be_bytes());
    }

    #[test]
    fn secured_deny_blocks_inbound_transfer() {
        use ed25519_dalek::SigningKey;
        use rand::rngs::OsRng;

        let key = SigningKey::generate(&mut OsRng);
        let deployer = boing_primitives::AccountId(key.verifying_key().to_bytes());
        let bob = boing_primitives::AccountId([0xbbu8; 32]);

        let cfg = ReferenceFungibleSecuredConfig {
            flags: FLAG_DENYLIST,
            ..Default::default()
        };
        let deploy_bc = reference_fungible_secured_deploy_bytecode(&cfg);

        let mut state = StateStore::new();
        state.insert(Account {
            id: deployer,
            state: boing_primitives::AccountState {
                balance: 10_000_000,
                nonce: 0,
                stake: 0, ..Default::default() },
        });

        let vm = Vm::new();
        let ctx = VmExecutionContext {
            block_height: 1,
            block_timestamp: 1_000,
        };
        let deploy_tx = boing_primitives::Transaction {
            nonce: 0,
            sender: deployer,
            payload: boing_primitives::TransactionPayload::ContractDeployWithPurposeAndMetadata {
                bytecode: deploy_bc,
                purpose_category: "token".to_string(),
                description_hash: None,
                asset_name: None,
                asset_symbol: None,
                create2_salt: None,
            },
            access_list: boing_primitives::AccessList::default(),
        };
        vm.execute_with_context(&deploy_tx, &mut state, ctx).unwrap();
        let token = boing_primitives::nonce_derived_contract_address(&deployer, 0);

        vm.execute_with_context(
            &boing_primitives::Transaction {
                nonce: 1,
                sender: deployer,
                payload: boing_primitives::TransactionPayload::ContractCall {
                    contract: token,
                    calldata: encode_mint_first_calldata(&deployer, 1_000),
                },
                access_list: boing_primitives::AccessList::default(),
            },
            &mut state,
            ctx,
        )
        .unwrap();

        vm.execute_with_context(
            &boing_primitives::Transaction {
                nonce: 2,
                sender: deployer,
                payload: boing_primitives::TransactionPayload::ContractCall {
                    contract: token,
                    calldata: encode_secured_set_deny_calldata(&bob, true),
                },
                access_list: boing_primitives::AccessList::default(),
            },
            &mut state,
            ctx,
        )
        .unwrap();

        vm.execute_with_context(
            &boing_primitives::Transaction {
                nonce: 3,
                sender: deployer,
                payload: boing_primitives::TransactionPayload::ContractCall {
                    contract: token,
                    calldata: encode_transfer_calldata(&bob, 10),
                },
                access_list: boing_primitives::AccessList::default(),
            },
            &mut state,
            ctx,
        )
        .unwrap();
        assert_eq!(
            state.get_contract_storage(&token, &xor_bal_key(&bob)),
            word_zero(),
            "denied inbound transfer must not credit recipient"
        );
    }

    #[test]
    fn secured_anti_bot_caps_amount_in_window() {
        use ed25519_dalek::SigningKey;
        use rand::rngs::OsRng;

        let key = SigningKey::generate(&mut OsRng);
        let deployer = boing_primitives::AccountId(key.verifying_key().to_bytes());
        let bob = boing_primitives::AccountId([0xccu8; 32]);

        let cfg = ReferenceFungibleSecuredConfig {
            flags: FLAG_ANTI_BOT,
            anti_bot_extra_blocks: 100,
            anti_bot_max_amount: 50,
            ..Default::default()
        };
        let deploy_bc = reference_fungible_secured_deploy_bytecode(&cfg);

        let mut state = StateStore::new();
        state.insert(Account {
            id: deployer,
            state: boing_primitives::AccountState {
                balance: 10_000_000,
                nonce: 0,
                stake: 0, ..Default::default() },
        });

        let vm = Vm::new();
        let deploy_h = 1_000u64;
        let ctx = VmExecutionContext {
            block_height: deploy_h,
            block_timestamp: 2_000,
        };
        let deploy_tx = boing_primitives::Transaction {
            nonce: 0,
            sender: deployer,
            payload: boing_primitives::TransactionPayload::ContractDeployWithPurposeAndMetadata {
                bytecode: deploy_bc,
                purpose_category: "token".to_string(),
                description_hash: None,
                asset_name: None,
                asset_symbol: None,
                create2_salt: None,
            },
            access_list: boing_primitives::AccessList::default(),
        };
        vm.execute_with_context(&deploy_tx, &mut state, ctx).unwrap();
        let token = boing_primitives::nonce_derived_contract_address(&deployer, 0);

        vm.execute_with_context(
            &boing_primitives::Transaction {
                nonce: 1,
                sender: deployer,
                payload: boing_primitives::TransactionPayload::ContractCall {
                    contract: token,
                    calldata: encode_mint_first_calldata(&deployer, 1_000),
                },
                access_list: boing_primitives::AccessList::default(),
            },
            &mut state,
            ctx,
        )
        .unwrap();

        let mid = VmExecutionContext {
            block_height: deploy_h + 50,
            block_timestamp: 3_000,
        };
        vm.execute_with_context(
            &boing_primitives::Transaction {
                nonce: 2,
                sender: deployer,
                payload: boing_primitives::TransactionPayload::ContractCall {
                    contract: token,
                    calldata: encode_transfer_calldata(&bob, 99),
                },
                access_list: boing_primitives::AccessList::default(),
            },
            &mut state,
            mid,
        )
        .unwrap();
        assert_eq!(
            state.get_contract_storage(&token, &xor_bal_key(&bob)),
            word_zero(),
            "anti-bot cap must block oversized transfer (contract STOP is still Ok)"
        );

        vm.execute_with_context(
            &boing_primitives::Transaction {
                nonce: 3,
                sender: deployer,
                payload: boing_primitives::TransactionPayload::ContractCall {
                    contract: token,
                    calldata: encode_transfer_calldata(&bob, 40),
                },
                access_list: boing_primitives::AccessList::default(),
            },
            &mut state,
            mid,
        )
        .unwrap();
        let b_bob = state.get_contract_storage(&token, &xor_bal_key(&bob));
        assert_eq!(&b_bob[16..32], &40u128.to_be_bytes());
    }
}
