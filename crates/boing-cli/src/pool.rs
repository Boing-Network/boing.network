//! `boing pool` -- Boing-native constant-product AMM pools (create / liquidity / swap / quote / reserves).
//!
//! Thin, dependency-free wrapper over the existing **native CP pool** templates in
//! `boing_execution::native_amm` (`constant_product_pool_bytecode{,_v2..v7}`, `docs/NATIVE-AMM-CALLDATA.md`).
//! Nothing here changes pool bytecode: transactions are Boing VM calls (bincode `SignedTransaction`,
//! Ed25519) submitted via `boing_submitTransaction`; state is read with `boing_getContractStorage`.
//!
//! **Custody caveat (all v1..v7 templates):** `add_liquidity` and the swap input are *ledger-only* --
//! the pool credits reserves from calldata and never pulls tokens. Pools with token hooks (v2, v4..v7)
//! pay swap output / LP removals out of the pool's own reference-token balance. Treat these pools as
//! trusted/demo liquidity; see `docs/NATIVE-AMM-CLI.md` (custody gap + proposed v8 design).

use crate::token::{
    account_of, build_approve_tx, hex_account, nonce_for, parse_account, resolve_signing_key, rpc,
    submit,
};
use boing_execution::{
    constant_product_amount_out_after_fee_with_bps,
    constant_product_amount_out_fee_on_input_with_bps, constant_product_pool_bytecode,
    constant_product_pool_bytecode_v2, constant_product_pool_bytecode_v3,
    constant_product_pool_bytecode_v4, constant_product_pool_bytecode_v5,
    constant_product_pool_bytecode_v6, constant_product_pool_bytecode_v7,
    constant_product_pool_bytecode_v8, constant_product_pool_bytecode_v9, encode_add_liquidity_calldata,
    encode_remove_liquidity_calldata, encode_set_fee_admin_calldata,
    encode_set_swap_fee_bps_calldata, encode_set_tokens_calldata, encode_swap_calldata,
    encode_swap_to_calldata, encode_v8_add_liquidity_calldata, encode_v8_remove_liquidity_calldata,
    encode_v8_swap_calldata, encode_v8_swap_to_calldata, lp_balance_storage_key, reserve_a_key,
    reserve_b_key, swap_fee_bps_key, token_a_key, token_b_key, total_lp_supply_key,
    NATIVE_CP_SWAP_FEE_BPS,
};
use boing_primitives::{
    nonce_derived_contract_address, AccessList, AccountId, SignedTransaction, Transaction,
    TransactionPayload,
};
use ed25519_dalek::SigningKey;
use serde_json::{json, Value};

/// QA purpose category for pool deploys (same default as `deploy-native-amm-pool.mjs`).
pub const POOL_PURPOSE: &str = "dapp";

/// Pool template version (`v1`..`v8`, see `NATIVE-AMM-CALLDATA.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoolVersion(pub u8);

impl std::str::FromStr for PoolVersion {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        let n: u8 = s
            .trim()
            .trim_start_matches(['v', 'V'])
            .parse()
            .map_err(|_| format!("invalid pool version `{s}` (expected v1..v10)"))?;
        if (1..=10).contains(&n) {
            Ok(PoolVersion(n))
        } else {
            Err(format!("invalid pool version `{s}` (expected v1..v10)"))
        }
    }
}

impl PoolVersion {
    pub fn bytecode(self) -> Vec<u8> {
        match self.0 {
            1 => constant_product_pool_bytecode(),
            2 => constant_product_pool_bytecode_v2(),
            3 => constant_product_pool_bytecode_v3(),
            4 => constant_product_pool_bytecode_v4(),
            5 => constant_product_pool_bytecode_v5(),
            6 => constant_product_pool_bytecode_v6(),
            7 => constant_product_pool_bytecode_v7(),
            8 => constant_product_pool_bytecode_v8(),
            9 => constant_product_pool_bytecode_v9(),
            _ => boing_execution::constant_product_pool_bytecode_v10(),
        }
    }
    /// v2, v4..v7 pay out through reference-token contracts once `set_tokens` ran.
    pub fn has_token_hooks(self) -> bool {
        matches!(self.0, 2 | 4..=10)
    }
    /// v3..v7 keep the fee in storage (`set_swap_fee_bps`).
    pub fn has_storage_fee(self) -> bool {
        self.0 >= 3
    }
    /// v6/v7: `set_fee_admin`.
    pub fn has_fee_admin(self) -> bool {
        self.0 >= 6
    }
    /// v5..v7: `swap_to` / `remove_liquidity_to`.
    pub fn has_recipient_selectors(self) -> bool {
        self.0 >= 5
    }
    /// v7/v8 apply the fee to the input (Uniswap style); v1..v6 to the output.
    pub fn fee_on_input(self) -> bool {
        self.0 >= 7
    }
    /// v8 pulls deposits / swap input with reference-token v2 `transfer_from` (needs `approve`).
    /// v9 = v8 whose swap returns `amount_out` (needed for custody-router intermediate hops).
    pub fn pulls_tokens(self) -> bool {
        self.0 >= 8
    }
    /// v10 = v9 with u128 amounts / reserves (18-decimal tokens such as wBOING).
    pub fn u128_amounts(self) -> bool {
        self.0 >= 10
    }
}

/// Access list for a pool call: `[sender, pool, ...sorted unique extras]` on both read and write
/// (same as `boing-sdk` `buildNativeConstantProductPoolAccessList`).
pub fn pool_access_list(sender: AccountId, pool: AccountId, extras: &[AccountId]) -> AccessList {
    let mut ex: Vec<AccountId> = extras
        .iter()
        .copied()
        .filter(|a| *a != sender && *a != pool && a.0 != [0u8; 32])
        .collect();
    ex.sort_by_key(|a| a.0);
    ex.dedup();
    let mut v = vec![sender, pool];
    v.extend(ex);
    AccessList {
        read: v.clone(),
        write: v,
    }
}

/// Signed pool `ContractCall` with the pool access list.
pub fn build_pool_call_tx(
    key: &SigningKey,
    nonce: u64,
    pool: AccountId,
    calldata: Vec<u8>,
    extras: &[AccountId],
) -> SignedTransaction {
    let sender = account_of(key);
    let tx = Transaction {
        nonce,
        sender,
        payload: TransactionPayload::ContractCall {
            contract: pool,
            calldata,
        },
        access_list: pool_access_list(sender, pool, extras),
    };
    SignedTransaction::new(tx, key)
}

/// Signed nonce-derived `ContractDeployWithPurpose` (purpose `dapp`) for the chosen template.
pub fn build_pool_deploy_tx(
    key: &SigningKey,
    nonce: u64,
    version: PoolVersion,
) -> SignedTransaction {
    let mut tx = Transaction {
        nonce,
        sender: account_of(key),
        payload: TransactionPayload::ContractDeployWithPurpose {
            bytecode: version.bytecode(),
            purpose_category: POOL_PURPOSE.to_string(),
            description_hash: None,
            create2_salt: None,
        },
        access_list: AccessList::default(),
    };
    tx.access_list = tx.suggested_parallel_access_list();
    SignedTransaction::new(tx, key)
}

pub fn parse_direction(s: &str) -> anyhow::Result<u128> {
    match s.trim().to_ascii_lowercase().as_str() {
        "a-to-b" | "atob" | "a2b" | "0" => Ok(0),
        "b-to-a" | "btoa" | "b2a" | "1" => Ok(1),
        _ => anyhow::bail!("direction must be a-to-b or b-to-a"),
    }
}

fn to_u64(v: u128, what: &str) -> anyhow::Result<u64> {
    u64::try_from(v).map_err(|_| anyhow::anyhow!("{what} exceeds u64 (pool ledger limit)"))
}

/// Effective swap fee bps: v1/v2 fixed 30; v3..v7 stored word (`0` = unset -> 30).
pub fn effective_fee_bps(version: PoolVersion, stored: u128) -> u128 {
    if !version.has_storage_fee() || stored == 0 {
        u128::from(NATIVE_CP_SWAP_FEE_BPS)
    } else {
        stored
    }
}

/// Off-chain quote matching the pool bytecode for `version`.
pub fn quote_amount_out(
    version: PoolVersion,
    reserve_in: u128,
    reserve_out: u128,
    amount_in: u128,
    fee_bps: u128,
) -> anyhow::Result<u128> {
    if version.u128_amounts() {
        // v10: fee on input, 256-bit products (same formula as the pool bytecode).
        if fee_bps > 10_000 {
            anyhow::bail!("fee_bps > 10000");
        }
        use num_bigint::BigUint;
        let after_fee = BigUint::from(amount_in) * BigUint::from(10_000 - fee_bps) / BigUint::from(10_000u32);
        let den = BigUint::from(reserve_in) + &after_fee;
        if after_fee == BigUint::from(0u32) || den == BigUint::from(0u32) {
            return Ok(0);
        }
        let out = BigUint::from(reserve_out) * after_fee / den;
        return u128::try_from(out).map_err(|_| anyhow::anyhow!("quote exceeds u128"));
    }
    let (ri, ro, ai) = (
        to_u64(reserve_in, "reserve")?,
        to_u64(reserve_out, "reserve")?,
        to_u64(amount_in, "amount_in")?,
    );
    if fee_bps > 10_000 {
        anyhow::bail!("fee_bps > 10000");
    }
    let f = fee_bps as u16;
    let out = if version.fee_on_input() {
        constant_product_amount_out_fee_on_input_with_bps(ri, ro, ai, f)
    } else {
        constant_product_amount_out_after_fee_with_bps(ri, ro, ai, f)
    };
    Ok(u128::from(out))
}

/// `min_out = floor(quote * (10_000 - slippage_bps) / 10_000)`, at least 1.
pub fn min_out_with_slippage(quote: u128, slippage_bps: u128) -> anyhow::Result<u128> {
    if slippage_bps > 10_000 {
        anyhow::bail!("slippage bps must be <= 10000");
    }
    Ok((quote * (10_000 - slippage_bps) / 10_000).max(1))
}

/// LP minted by `add_liquidity` (mirrors bytecode: first mint = min(a, b); then
/// min(a * T / ra, b * T / rb)).
pub fn quote_lp_minted(ra: u128, rb: u128, total: u128, a: u128, b: u128) -> u128 {
    if a == 0 || b == 0 {
        return 0;
    }
    if total == 0 {
        return a.min(b);
    }
    if ra == 0 || rb == 0 {
        return 0;
    }
    (a * total / ra).min(b * total / rb)
}

/// Amounts paid by `remove_liquidity(burn)`: (burn * ra / T, burn * rb / T).
pub fn quote_remove(ra: u128, rb: u128, total: u128, burn: u128) -> (u128, u128) {
    if total == 0 {
        return (0, 0);
    }
    (burn * ra / total, burn * rb / total)
}

// --- RPC reads ---

pub fn decode_word_u128(hex_word: &str) -> anyhow::Result<u128> {
    crate::token::decode_balance_word(hex_word)
}

async fn read_word(rpc_url: &str, pool: &AccountId, key: [u8; 32]) -> anyhow::Result<[u8; 32]> {
    let r = rpc(
        rpc_url,
        "boing_getContractStorage",
        json!([hex_account(pool), format!("0x{}", hex::encode(key))]),
    )
    .await?;
    let s = r
        .get("value")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("unexpected boing_getContractStorage result: {r}"))?;
    let b = hex::decode(s.trim_start_matches("0x"))?;
    b.try_into()
        .map_err(|_| anyhow::anyhow!("storage word must be 32 bytes"))
}

async fn read_u128(rpc_url: &str, pool: &AccountId, key: [u8; 32]) -> anyhow::Result<u128> {
    decode_word_u128(&hex::encode(read_word(rpc_url, pool, key).await?))
}

pub struct PoolState {
    pub reserve_a: u128,
    pub reserve_b: u128,
    pub total_lp: u128,
    pub stored_fee_bps: u128,
    pub token_a: AccountId,
    pub token_b: AccountId,
}

pub(crate) async fn read_state(rpc_url: &str, pool: &AccountId) -> anyhow::Result<PoolState> {
    Ok(PoolState {
        reserve_a: read_u128(rpc_url, pool, reserve_a_key()).await?,
        reserve_b: read_u128(rpc_url, pool, reserve_b_key()).await?,
        total_lp: read_u128(rpc_url, pool, total_lp_supply_key()).await?,
        stored_fee_bps: read_u128(rpc_url, pool, swap_fee_bps_key()).await?,
        token_a: AccountId(read_word(rpc_url, pool, token_a_key()).await?),
        token_b: AccountId(read_word(rpc_url, pool, token_b_key()).await?),
    })
}

pub(crate) fn parse_extras(v: &[String]) -> anyhow::Result<Vec<AccountId>> {
    v.iter().map(|s| parse_account(s)).collect()
}

/// Extras + configured token contracts (for token-hook pools) so nested `CALL`s are declared.
async fn extras_with_tokens(
    rpc_url: &str,
    pool: &AccountId,
    version: PoolVersion,
    extra: &[String],
    offline: bool,
) -> anyhow::Result<Vec<AccountId>> {
    let mut v = parse_extras(extra)?;
    if version.has_token_hooks() && !offline {
        v.push(AccountId(read_word(rpc_url, pool, token_a_key()).await?));
        v.push(AccountId(read_word(rpc_url, pool, token_b_key()).await?));
    }
    Ok(v)
}

/// v8-only options shared by add / remove / swap.
#[derive(Default)]
pub struct V8Opts {
    /// Token ids (needed offline; otherwise read from the pool).
    pub token_a: Option<String>,
    pub token_b: Option<String>,
    /// Skip the automatic exact-amount `approve` transaction(s).
    pub no_approve: bool,
    /// Deadline block height (`0` = none).
    pub deadline: u64,
}

async fn pool_tokens(
    rpc_url: &str,
    pool: &AccountId,
    o: &V8Opts,
    offline: bool,
) -> anyhow::Result<(AccountId, AccountId)> {
    match (&o.token_a, &o.token_b) {
        (Some(a), Some(b)) => Ok((parse_account(a)?, parse_account(b)?)),
        (None, None) if !offline => {
            let a = AccountId(read_word(rpc_url, pool, token_a_key()).await?);
            let b = AccountId(read_word(rpc_url, pool, token_b_key()).await?);
            if a.0 == [0u8; 32] || b.0 == [0u8; 32] {
                anyhow::bail!("pool has no tokens configured (is it really v8? pass --version)");
            }
            Ok((a, b))
        }
        (None, None) => anyhow::bail!("offline v8 dry-run needs --token-a and --token-b"),
        _ => anyhow::bail!("pass both --token-a and --token-b (or neither)"),
    }
}

// --- commands ---

pub struct Signer {
    pub secret: Option<String>,
    pub nonce: Option<u64>,
    pub dry_run: bool,
}

impl Signer {
    /// Offline = `--dry-run` with an explicit `--nonce`: no RPC at all.
    fn offline(&self) -> bool {
        self.dry_run && self.nonce.is_some()
    }
}

pub struct CreateArgs {
    pub version: PoolVersion,
    pub token_a: Option<String>,
    pub token_b: Option<String>,
    pub fee_bps: Option<u128>,
    pub fee_admin: Option<String>,
}

pub async fn create(rpc_url: &str, s: Signer, a: CreateArgs) -> anyhow::Result<()> {
    let key = resolve_signing_key(s.secret.clone())?;
    let me = account_of(&key);
    let v = a.version;
    let tokens = match (&a.token_a, &a.token_b) {
        (None, None) => None,
        (Some(x), Some(y)) => {
            if !v.has_token_hooks() {
                anyhow::bail!(
                    "pool v{} has no token hooks; use v2 or v4..v8 with --token-a/--token-b",
                    v.0
                );
            }
            Some((parse_account(x)?, parse_account(y)?))
        }
        _ => anyhow::bail!("pass both --token-a and --token-b (or neither)"),
    };
    if v.pulls_tokens() && tokens.is_none() {
        anyhow::bail!("pool v8 needs --token-a and --token-b (reference fungible v2 tokens)");
    }
    if let Some(f) = a.fee_bps {
        if !v.has_storage_fee() {
            anyhow::bail!(
                "pool v{} has a fixed 30 bps fee; --fee-bps needs v3..v8",
                v.0
            );
        }
        if !(1..=10_000).contains(&f) {
            anyhow::bail!("--fee-bps must be 1..=10000");
        }
    }
    let admin = match &a.fee_admin {
        Some(x) if !v.has_fee_admin() => {
            anyhow::bail!("--fee-admin needs v6..v8 (got v{}; {x})", v.0)
        }
        Some(x) => Some(parse_account(x)?),
        None => None,
    };
    let mut n = nonce_for(rpc_url, &key, s.nonce).await?;
    let pool = nonce_derived_contract_address(&me, n);
    println!("Deploying Boing native constant-product pool v{}", v.0);
    println!("  deployer {}", hex_account(&me));
    println!("  pool     {}", hex_account(&pool));
    submit(rpc_url, &build_pool_deploy_tx(&key, n, v), s.dry_run).await?;
    if let Some((ta, tb)) = tokens {
        n += 1;
        println!("set_tokens A={} B={}", hex_account(&ta), hex_account(&tb));
        let tx = build_pool_call_tx(
            &key,
            n,
            pool,
            encode_set_tokens_calldata(&ta, &tb),
            &[ta, tb],
        );
        submit(rpc_url, &tx, s.dry_run).await?;
    }
    if let Some(ad) = admin {
        n += 1;
        println!("set_fee_admin {}", hex_account(&ad));
        let tx = build_pool_call_tx(&key, n, pool, encode_set_fee_admin_calldata(&ad), &[]);
        submit(rpc_url, &tx, s.dry_run).await?;
    }
    if let Some(f) = a.fee_bps {
        n += 1;
        println!("set_swap_fee_bps {f}");
        let tx = build_pool_call_tx(&key, n, pool, encode_set_swap_fee_bps_calldata(f), &[]);
        submit(rpc_url, &tx, s.dry_run).await?;
    }
    if v.pulls_tokens() {
        println!("Note: v8 pulls deposits/swap input via transfer_from; tokens must be reference fungible v2.");
    } else {
        println!("Note: v1..v7 pools are ledger-only on deposits (no token pull). See docs/NATIVE-AMM-CLI.md.");
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn add_liquidity(
    rpc_url: &str,
    s: Signer,
    pool: &str,
    amount_a: u128,
    amount_b: u128,
    min_liquidity: Option<u128>,
    version: PoolVersion,
    v8: &V8Opts,
) -> anyhow::Result<()> {
    if amount_a == 0 || amount_b == 0 {
        anyhow::bail!("amounts must be > 0");
    }
    let key = resolve_signing_key(s.secret.clone())?;
    let pool = parse_account(pool)?;
    let min_lp = min_liquidity.unwrap_or(0);
    if !s.offline() {
        // v1..v7 ignore min_liquidity on-chain (v8 enforces it): preview it client-side either way.
        let st = read_state(rpc_url, &pool).await?;
        let lp = quote_lp_minted(st.reserve_a, st.reserve_b, st.total_lp, amount_a, amount_b);
        println!("  expected LP minted {lp}");
        if lp == 0 {
            anyhow::bail!("this deposit would mint 0 LP (amounts too small for the current ratio)");
        }
        if lp < min_lp {
            anyhow::bail!("expected LP {lp} < --min-liquidity {min_lp}; not submitting");
        }
    }
    let mut n = nonce_for(rpc_url, &key, s.nonce).await?;
    if version.pulls_tokens() {
        let (ta, tb) = pool_tokens(rpc_url, &pool, v8, s.offline()).await?;
        if !v8.no_approve {
            println!("approve A {amount_a} and B {amount_b} to the pool");
            submit(
                rpc_url,
                &build_approve_tx(&key, n, ta, pool, amount_a),
                s.dry_run,
            )
            .await?;
            submit(
                rpc_url,
                &build_approve_tx(&key, n + 1, tb, pool, amount_b),
                s.dry_run,
            )
            .await?;
            n += 2;
        }
        let cd = encode_v8_add_liquidity_calldata(amount_a, amount_b, min_lp, v8.deadline);
        return submit(
            rpc_url,
            &build_pool_call_tx(&key, n, pool, cd, &[ta, tb]),
            s.dry_run,
        )
        .await;
    }
    let tx = build_pool_call_tx(
        &key,
        n,
        pool,
        encode_add_liquidity_calldata(amount_a, amount_b, min_lp),
        &[],
    );
    submit(rpc_url, &tx, s.dry_run).await
}

#[allow(clippy::too_many_arguments)]
pub async fn remove_liquidity(
    rpc_url: &str,
    s: Signer,
    pool: &str,
    version: PoolVersion,
    liquidity: u128,
    min_a: Option<u128>,
    min_b: Option<u128>,
    slippage_bps: u128,
    extra: &[String],
    v8: &V8Opts,
) -> anyhow::Result<()> {
    if liquidity == 0 {
        anyhow::bail!("--liquidity must be > 0");
    }
    let key = resolve_signing_key(s.secret.clone())?;
    let me = account_of(&key);
    let pool_id = parse_account(pool)?;
    let (min_a, min_b) = match (min_a, min_b) {
        (Some(a), Some(b)) => (a, b),
        _ => {
            if s.offline() {
                anyhow::bail!("offline dry-run needs explicit --min-a and --min-b");
            }
            let st = read_state(rpc_url, &pool_id).await?;
            let mine = read_u128(rpc_url, &pool_id, lp_balance_storage_key(&me.0)).await?;
            if liquidity > mine {
                anyhow::bail!("--liquidity {liquidity} exceeds your LP balance {mine}");
            }
            let (oa, ob) = quote_remove(st.reserve_a, st.reserve_b, st.total_lp, liquidity);
            println!("  expected out A={oa} B={ob}");
            (
                min_a.unwrap_or(min_out_with_slippage(oa, slippage_bps)?.min(oa)),
                min_b.unwrap_or(min_out_with_slippage(ob, slippage_bps)?.min(ob)),
            )
        }
    };
    println!("  min_a {min_a} min_b {min_b}");
    let (extras, calldata) = if version.pulls_tokens() {
        let (ta, tb) = pool_tokens(rpc_url, &pool_id, v8, s.offline()).await?;
        let mut ex = parse_extras(extra)?;
        ex.extend([ta, tb]);
        let cd = encode_v8_remove_liquidity_calldata(liquidity, min_a, min_b, v8.deadline);
        (ex, cd)
    } else {
        (
            extras_with_tokens(rpc_url, &pool_id, version, extra, s.offline()).await?,
            encode_remove_liquidity_calldata(liquidity, min_a, min_b),
        )
    };
    let n = nonce_for(rpc_url, &key, s.nonce).await?;
    let tx = build_pool_call_tx(&key, n, pool_id, calldata, &extras);
    submit(rpc_url, &tx, s.dry_run).await
}

#[allow(clippy::too_many_arguments)]
pub async fn swap(
    rpc_url: &str,
    s: Signer,
    pool: &str,
    version: PoolVersion,
    direction: &str,
    amount_in: u128,
    min_out: Option<u128>,
    slippage_bps: u128,
    to: Option<String>,
    extra: &[String],
    v8: &V8Opts,
) -> anyhow::Result<()> {
    if amount_in == 0 {
        anyhow::bail!("--amount-in must be > 0");
    }
    let dir = parse_direction(direction)?;
    let key = resolve_signing_key(s.secret.clone())?;
    let pool_id = parse_account(pool)?;
    let min_out = match min_out {
        Some(m) => m,
        None => {
            if s.offline() {
                anyhow::bail!("offline dry-run needs an explicit --min-out");
            }
            let st = read_state(rpc_url, &pool_id).await?;
            let (ri, ro) = if dir == 0 {
                (st.reserve_a, st.reserve_b)
            } else {
                (st.reserve_b, st.reserve_a)
            };
            let fee = effective_fee_bps(version, st.stored_fee_bps);
            let q = quote_amount_out(version, ri, ro, amount_in, fee)?;
            if q == 0 {
                anyhow::bail!("quote is 0 (empty pool or amount too small)");
            }
            let m = min_out_with_slippage(q, slippage_bps)?;
            println!("  quote {q} (fee {fee} bps), min_out {m} ({slippage_bps} bps slippage)");
            m
        }
    };
    if min_out == 0 {
        anyhow::bail!("--min-out must be > 0");
    }
    let calldata = match to {
        Some(t) => {
            if !version.has_recipient_selectors() {
                anyhow::bail!("--to needs pool v5..v8 (swap_to)");
            }
            let r = parse_account(&t)?;
            if version.pulls_tokens() {
                encode_v8_swap_to_calldata(dir, amount_in, min_out, &r, v8.deadline)
            } else {
                encode_swap_to_calldata(dir, amount_in, min_out, &r)
            }
        }
        None if version.pulls_tokens() => {
            encode_v8_swap_calldata(dir, amount_in, min_out, v8.deadline)
        }
        None => encode_swap_calldata(dir, amount_in, min_out),
    };
    let mut n = nonce_for(rpc_url, &key, s.nonce).await?;
    let extras = if version.pulls_tokens() {
        let (ta, tb) = pool_tokens(rpc_url, &pool_id, v8, s.offline()).await?;
        if !v8.no_approve {
            let tin = if dir == 0 { ta } else { tb };
            println!("approve {amount_in} of {} to the pool", hex_account(&tin));
            submit(
                rpc_url,
                &build_approve_tx(&key, n, tin, pool_id, amount_in),
                s.dry_run,
            )
            .await?;
            n += 1;
        }
        let mut ex = parse_extras(extra)?;
        ex.extend([ta, tb]);
        ex
    } else {
        extras_with_tokens(rpc_url, &pool_id, version, extra, s.offline()).await?
    };
    submit(
        rpc_url,
        &build_pool_call_tx(&key, n, pool_id, calldata, &extras),
        s.dry_run,
    )
    .await
}

pub async fn quote(
    rpc_url: &str,
    pool: &str,
    version: PoolVersion,
    direction: &str,
    amount_in: u128,
    slippage_bps: u128,
) -> anyhow::Result<()> {
    let dir = parse_direction(direction)?;
    let pool = parse_account(pool)?;
    let st = read_state(rpc_url, &pool).await?;
    let (ri, ro) = if dir == 0 {
        (st.reserve_a, st.reserve_b)
    } else {
        (st.reserve_b, st.reserve_a)
    };
    let fee = effective_fee_bps(version, st.stored_fee_bps);
    let q = quote_amount_out(version, ri, ro, amount_in, fee)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "direction": if dir == 0 { "a-to-b" } else { "b-to-a" },
            "amount_in": amount_in.to_string(),
            "amount_out": q.to_string(),
            "fee_bps": fee.to_string(),
            "min_out": min_out_with_slippage(q, slippage_bps)?.min(q).to_string(),
            "slippage_bps": slippage_bps.to_string(),
        }))?
    );
    Ok(())
}

pub async fn reserves(
    rpc_url: &str,
    pool: &str,
    version: PoolVersion,
    holder: Option<String>,
) -> anyhow::Result<()> {
    let pool = parse_account(pool)?;
    let st = read_state(rpc_url, &pool).await?;
    let mut out = json!({
        "pool": hex_account(&pool),
        "reserve_a": st.reserve_a.to_string(),
        "reserve_b": st.reserve_b.to_string(),
        "total_lp": st.total_lp.to_string(),
        "fee_bps": effective_fee_bps(version, st.stored_fee_bps).to_string(),
        "token_a": hex_account(&st.token_a),
        "token_b": hex_account(&st.token_b),
    });
    if let Some(h) = holder {
        let h = parse_account(&h)?;
        let lp = read_u128(rpc_url, &pool, lp_balance_storage_key(&h.0)).await?;
        out["holder"] = json!(hex_account(&h));
        out["holder_lp"] = json!(lp.to_string());
    }
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

pub fn print_bytecode(version: PoolVersion) {
    println!("0x{}", hex::encode(version.bytecode()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::{balance_storage_key, build_deploy_tx, build_mint_tx, build_transfer_tx};
    use boing_execution::{decode_add_liquidity_return_lp_minted, Interpreter};
    use boing_primitives::Account;
    use boing_state::StateStore;

    fn key(b: u8) -> SigningKey {
        SigningKey::from_bytes(&[b; 32])
    }

    struct Run {
        logs: usize,
        ret: Option<Vec<u8>>,
    }

    fn try_run(state: &mut StateStore, stx: &SignedTransaction) -> Result<Run, String> {
        let TransactionPayload::ContractCall { contract, calldata } = &stx.tx.payload else {
            panic!("expected call");
        };
        let code = state.get_contract_code(contract).expect("code").clone();
        let cp = state.checkpoint();
        let mut it = Interpreter::new(code, 10_000_000);
        if let Err(e) = it.run(stx.tx.sender, *contract, calldata, state) {
            // Like the block executor: a VM fault reverts the whole transaction.
            state.revert(cp);
            return Err(format!("{e:?}"));
        }
        Ok(Run {
            logs: it.logs.len(),
            ret: it.return_data.clone(),
        })
    }

    fn run(state: &mut StateStore, stx: &SignedTransaction) -> Run {
        try_run(state, stx).unwrap()
    }

    fn word(state: &StateStore, c: &AccountId, k: [u8; 32]) -> u128 {
        decode_word_u128(&hex::encode(state.get_contract_storage(c, &k))).unwrap()
    }

    fn snap(state: &StateStore, pool: &AccountId) -> (u128, u128, u128) {
        (
            word(state, pool, reserve_a_key()),
            word(state, pool, reserve_b_key()),
            word(state, pool, total_lp_supply_key()),
        )
    }

    fn deploy(state: &mut StateStore, stx: &SignedTransaction) -> AccountId {
        let id = nonce_derived_contract_address(&stx.tx.sender, stx.tx.nonce);
        state.insert(Account {
            id,
            state: Default::default(),
        });
        let code = match &stx.tx.payload {
            TransactionPayload::ContractDeployWithPurpose { bytecode, .. }
            | TransactionPayload::ContractDeployWithPurposeAndMetadata { bytecode, .. } => {
                bytecode.clone()
            }
            p => panic!("unexpected {p:?}"),
        };
        state.set_contract_code(id, code);
        id
    }

    #[test]
    fn version_parsing_and_features() {
        assert_eq!("v7".parse::<PoolVersion>().unwrap(), PoolVersion(7));
        assert_eq!("3".parse::<PoolVersion>().unwrap(), PoolVersion(3));
        assert_eq!("v8".parse::<PoolVersion>().unwrap(), PoolVersion(8));
        assert_eq!("v9".parse::<PoolVersion>().unwrap(), PoolVersion(9));
        assert_eq!("v10".parse::<PoolVersion>().unwrap(), PoolVersion(10));
        assert!("v11".parse::<PoolVersion>().is_err());
        assert_eq!(PoolVersion(10).bytecode(), boing_execution::constant_product_pool_bytecode_v10());
        assert!(PoolVersion(10).pulls_tokens() && PoolVersion(10).u128_amounts() && !PoolVersion(9).u128_amounts());
        // v10 quotes 18-decimal amounts above 2^64 (v9 rejects them).
        let e18 = 10u128.pow(18);
        let q = quote_amount_out(PoolVersion(10), 1_000 * e18, 100_000 * e18, e18, 30).unwrap();
        assert_eq!(q, 99_600_698_103_990_321_649); // fee taken from the input first, then x*y=k (floors)
        assert!(quote_amount_out(PoolVersion(9), 1_000 * e18, 100_000 * e18, e18, 30).is_err());
        assert!(PoolVersion(8).pulls_tokens() && !PoolVersion(7).pulls_tokens());
        assert!(PoolVersion(8).fee_on_input() && PoolVersion(8).has_token_hooks());
        assert_eq!(
            PoolVersion(8).bytecode(),
            constant_product_pool_bytecode_v8()
        );
        assert!("v0".parse::<PoolVersion>().is_err());
        assert!(PoolVersion(7).fee_on_input() && !PoolVersion(6).fee_on_input());
        assert!(!PoolVersion(3).has_token_hooks() && PoolVersion(4).has_token_hooks());
        assert_eq!(PoolVersion(1).bytecode(), constant_product_pool_bytecode());
        assert_eq!(
            PoolVersion(7).bytecode(),
            constant_product_pool_bytecode_v7()
        );
        assert_eq!(parse_direction("b-to-a").unwrap(), 1);
        assert!(parse_direction("sideways").is_err());
    }

    #[test]
    fn deploy_and_call_txs_are_signed_and_well_formed() {
        let k = key(7);
        let me = account_of(&k);
        let d = build_pool_deploy_tx(&k, 4, PoolVersion(7));
        d.verify().unwrap();
        assert!(d.tx.access_list_covers_parallel_suggestion());
        match &d.tx.payload {
            TransactionPayload::ContractDeployWithPurpose {
                purpose_category,
                create2_salt,
                bytecode,
                ..
            } => {
                assert_eq!(purpose_category, "dapp");
                assert!(create2_salt.is_none());
                assert_eq!(bytecode, &constant_product_pool_bytecode_v7());
            }
            p => panic!("{p:?}"),
        }
        let pool = AccountId([0x55; 32]);
        let t1 = AccountId([0x99; 32]);
        let t0 = AccountId([0x11; 32]);
        let c = build_pool_call_tx(
            &k,
            5,
            pool,
            encode_swap_calldata(0, 10, 9),
            &[t1, t0, t1, pool],
        );
        c.verify().unwrap();
        assert_eq!(c.tx.access_list.read, vec![me, pool, t0, t1]);
        assert_eq!(c.tx.access_list.read, c.tx.access_list.write);
        assert!(c.tx.access_list_covers_parallel_suggestion());
        let bytes = bincode::serialize(&c).unwrap();
        let back = SignedTransaction::from_bincode(&bytes).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn quote_helpers() {
        // Doc example: 1000/2000 reserves, 100 in, 30 bps -> 180 (output-side and input-side).
        assert_eq!(
            quote_amount_out(PoolVersion(1), 1000, 2000, 100, 30).unwrap(),
            180
        );
        assert_eq!(
            quote_amount_out(PoolVersion(7), 1000, 2000, 100, 30).unwrap(),
            180
        );
        assert_ne!(
            quote_amount_out(PoolVersion(6), 1000, 2000, 100, 2000).unwrap(),
            quote_amount_out(PoolVersion(7), 1000, 2000, 100, 2000).unwrap()
        );
        assert!(quote_amount_out(PoolVersion(1), u128::from(u64::MAX) + 1, 1, 1, 30).is_err());
        assert_eq!(effective_fee_bps(PoolVersion(7), 0), 30);
        assert_eq!(effective_fee_bps(PoolVersion(7), 100), 100);
        assert_eq!(effective_fee_bps(PoolVersion(2), 100), 30);
        assert_eq!(min_out_with_slippage(1000, 50).unwrap(), 995);
        assert_eq!(min_out_with_slippage(1, 10_000).unwrap(), 1);
        assert!(min_out_with_slippage(1, 10_001).is_err());
        assert_eq!(quote_lp_minted(0, 0, 0, 1000, 2000), 1000);
        assert_eq!(quote_lp_minted(1000, 2000, 1000, 100, 500), 100);
        assert_eq!(quote_remove(1000, 2000, 1000, 250), (250, 500));
    }

    /// End-to-end on the Boing VM with the v7 template: create -> fee -> add -> swap -> remove,
    /// checking LP share math, the constant-product invariant, slippage rejection and no overdraft.
    #[test]
    fn v7_pool_lifecycle_on_boing_vm() {
        let lp1 = key(1);
        let lp2 = key(2);
        let trader = key(3);
        let (a1, a2) = (account_of(&lp1), account_of(&lp2));
        let mut st = StateStore::new();
        let pool = deploy(&mut st, &build_pool_deploy_tx(&lp1, 0, PoolVersion(7)));

        // Fee before liquidity.
        run(
            &mut st,
            &build_pool_call_tx(&lp1, 1, pool, encode_set_swap_fee_bps_calldata(50), &[]),
        );
        assert_eq!(word(&st, &pool, swap_fee_bps_key()), 50);

        // First mint: LP = min(a, b).
        let r = run(
            &mut st,
            &build_pool_call_tx(
                &lp1,
                2,
                pool,
                encode_add_liquidity_calldata(10_000, 40_000, 0),
                &[],
            ),
        );
        assert_eq!(r.logs, 1);
        assert_eq!(
            decode_add_liquidity_return_lp_minted(r.ret.as_deref().unwrap()),
            Some(10_000)
        );
        assert_eq!(snap(&st, &pool), (10_000, 40_000, 10_000));

        // Second LP at the same ratio gets a proportional share (quote helper matches the VM).
        let want = quote_lp_minted(10_000, 40_000, 10_000, 2_500, 10_000);
        let r = run(
            &mut st,
            &build_pool_call_tx(
                &lp2,
                0,
                pool,
                encode_add_liquidity_calldata(2_500, 10_000, 0),
                &[],
            ),
        );
        assert_eq!(
            decode_add_liquidity_return_lp_minted(r.ret.as_deref().unwrap()),
            Some(want)
        );
        assert_eq!(want, 2_500);
        assert_eq!(word(&st, &pool, lp_balance_storage_key(&a2.0)), 2_500);

        // Swap A->B: VM output equals off-chain quote; k never decreases.
        let (ra, rb, _) = snap(&st, &pool);
        let k0 = ra * rb;
        let q = quote_amount_out(PoolVersion(7), ra, rb, 1_000, 50).unwrap();
        // Slippage: min_out = q + 1 must be rejected with no state change and no log.
        let r = run(
            &mut st,
            &build_pool_call_tx(&trader, 0, pool, encode_swap_calldata(0, 1_000, q + 1), &[]),
        );
        assert_eq!(r.logs, 0);
        assert_eq!(snap(&st, &pool).0, ra);
        let r = run(
            &mut st,
            &build_pool_call_tx(
                &trader,
                1,
                pool,
                encode_swap_calldata(0, 1_000, min_out_with_slippage(q, 100).unwrap()),
                &[],
            ),
        );
        assert_eq!(r.logs, 1);
        let (ra2, rb2, t2) = snap(&st, &pool);
        assert_eq!((ra2, rb2), (ra + 1_000, rb - q));
        assert!(ra2 * rb2 >= k0, "constant product decreased");

        // Swap B->A also preserves k.
        let q2 = quote_amount_out(PoolVersion(7), rb2, ra2, 3_000, 50).unwrap();
        run(
            &mut st,
            &build_pool_call_tx(&trader, 2, pool, encode_swap_calldata(1, 3_000, q2), &[]),
        );
        let (ra3, rb3, _) = snap(&st, &pool);
        assert_eq!((ra3, rb3), (ra2 - q2, rb2 + 3_000));
        assert!(ra3 * rb3 >= ra2 * rb2);

        // No overdraft: lp2 cannot burn more than its 2_500 LP.
        let r = run(
            &mut st,
            &build_pool_call_tx(
                &lp2,
                1,
                pool,
                encode_remove_liquidity_calldata(2_501, 0, 0),
                &[],
            ),
        );
        assert_eq!(r.logs, 0);
        assert_eq!(snap(&st, &pool), (ra3, rb3, t2));
        // A trader with no LP cannot remove anything.
        let r = run(
            &mut st,
            &build_pool_call_tx(
                &trader,
                3,
                pool,
                encode_remove_liquidity_calldata(1, 0, 0),
                &[],
            ),
        );
        assert_eq!(r.logs, 0);

        // Remove-liquidity slippage guard (min_a above pro-rata) is rejected.
        let (oa, ob) = quote_remove(ra3, rb3, t2, 2_500);
        let r = run(
            &mut st,
            &build_pool_call_tx(
                &lp2,
                2,
                pool,
                encode_remove_liquidity_calldata(2_500, oa + 1, 0),
                &[],
            ),
        );
        assert_eq!(r.logs, 0);
        // Exact pro-rata removal succeeds and matches the quote.
        let r = run(
            &mut st,
            &build_pool_call_tx(
                &lp2,
                3,
                pool,
                encode_remove_liquidity_calldata(2_500, oa, ob),
                &[],
            ),
        );
        assert_eq!(r.logs, 1);
        assert_eq!(snap(&st, &pool), (ra3 - oa, rb3 - ob, t2 - 2_500));
        assert_eq!(word(&st, &pool, lp_balance_storage_key(&a2.0)), 0);
        assert_eq!(word(&st, &pool, lp_balance_storage_key(&a1.0)), 10_000);
    }

    /// v7 with reference-token hooks: swap output and LP removal are paid from the pool's token balance.
    #[test]
    fn v7_token_hooks_pay_out_reference_tokens() {
        let admin = key(1);
        let trader = key(3);
        let (me, tr) = (account_of(&admin), account_of(&trader));
        let mut st = StateStore::new();
        let ta = deploy(
            &mut st,
            &build_deploy_tx(&admin, 0, None, Some("TA".into())),
        );
        let tb = deploy(
            &mut st,
            &build_deploy_tx(&admin, 1, None, Some("TB".into())),
        );
        run(&mut st, &build_mint_tx(&admin, 2, ta, me, 1_000_000));
        run(&mut st, &build_mint_tx(&admin, 3, tb, me, 1_000_000));
        let pool = deploy(&mut st, &build_pool_deploy_tx(&admin, 4, PoolVersion(7)));
        run(
            &mut st,
            &build_pool_call_tx(
                &admin,
                5,
                pool,
                encode_set_tokens_calldata(&ta, &tb),
                &[ta, tb],
            ),
        );
        assert_eq!(st.get_contract_storage(&pool, &token_a_key()), ta.0);
        // Fund the pool's token balances to back the ledger deposit (ledger-only custody).
        run(&mut st, &build_transfer_tx(&admin, 6, ta, pool, 100_000));
        run(&mut st, &build_transfer_tx(&admin, 7, tb, pool, 200_000));
        run(
            &mut st,
            &build_pool_call_tx(
                &admin,
                8,
                pool,
                encode_add_liquidity_calldata(100_000, 200_000, 0),
                &[],
            ),
        );

        let bal = |s: &StateStore, t: &AccountId, w: &AccountId| word(s, t, balance_storage_key(w));
        let q = quote_amount_out(PoolVersion(7), 100_000, 200_000, 5_000, 30).unwrap();
        let r = run(
            &mut st,
            &build_pool_call_tx(
                &trader,
                0,
                pool,
                encode_swap_calldata(0, 5_000, q),
                &[ta, tb],
            ),
        );
        assert!(r.logs >= 1);
        assert_eq!(bal(&st, &tb, &tr), q);
        assert_eq!(bal(&st, &tb, &pool), 200_000 - q);
    }

    /// v8 + reference token v2 through the CLI tx builders: approve -> add pulls, approve -> swap
    /// pulls, no approve -> rejected, custody: pool token balances == reserves.
    #[test]
    fn v8_pool_pulls_tokens_with_v2_approve() {
        use crate::token::{build_deploy_tx_versioned, TokenVersion};
        let admin = key(1);
        let trader = key(3);
        let (me, tr) = (account_of(&admin), account_of(&trader));
        let mut st = StateStore::new();
        let ta = deploy(
            &mut st,
            &build_deploy_tx_versioned(&admin, 0, None, None, TokenVersion(2)),
        );
        let tb = deploy(
            &mut st,
            &build_deploy_tx_versioned(&admin, 1, None, None, TokenVersion(2)),
        );
        run(&mut st, &build_mint_tx(&admin, 2, ta, me, 1_000_000));
        run(&mut st, &build_mint_tx(&admin, 3, tb, me, 1_000_000));
        let pool = deploy(&mut st, &build_pool_deploy_tx(&admin, 4, PoolVersion(8)));
        let tok = [ta, tb];
        run(
            &mut st,
            &build_pool_call_tx(&admin, 5, pool, encode_set_tokens_calldata(&ta, &tb), &tok),
        );
        let bal = |s: &StateStore, t: &AccountId, w: &AccountId| word(s, t, balance_storage_key(w));
        // No approval: add is rejected and nothing is credited.
        let add = encode_v8_add_liquidity_calldata(100_000, 200_000, 1, 0);
        assert!(try_run(
            &mut st,
            &build_pool_call_tx(&admin, 6, pool, add.clone(), &tok)
        )
        .is_err());
        // Approve flow (what `boing pool add-liquidity` submits).
        run(&mut st, &build_approve_tx(&admin, 6, ta, pool, 100_000));
        run(&mut st, &build_approve_tx(&admin, 7, tb, pool, 200_000));
        let r = run(&mut st, &build_pool_call_tx(&admin, 8, pool, add, &tok));
        assert_eq!(
            decode_add_liquidity_return_lp_minted(r.ret.as_deref().unwrap()),
            Some(100_000)
        );
        assert_eq!(snap(&st, &pool), (100_000, 200_000, 100_000));
        assert_eq!(
            (bal(&st, &ta, &pool), bal(&st, &tb, &pool)),
            (100_000, 200_000)
        );
        assert_eq!(bal(&st, &ta, &me), 900_000);
        // Trader: fund, approve, swap A->B.
        run(&mut st, &build_transfer_tx(&admin, 9, ta, tr, 10_000));
        let q = quote_amount_out(PoolVersion(8), 100_000, 200_000, 5_000, 30).unwrap();
        let sw = encode_v8_swap_calldata(0, 5_000, q, 0);
        assert!(try_run(
            &mut st,
            &build_pool_call_tx(&trader, 0, pool, sw.clone(), &tok)
        )
        .is_err());
        run(&mut st, &build_approve_tx(&trader, 0, ta, pool, 5_000));
        run(&mut st, &build_pool_call_tx(&trader, 1, pool, sw, &tok));
        assert_eq!(bal(&st, &ta, &tr), 5_000);
        assert_eq!(bal(&st, &tb, &tr), q);
        let (ra, rb, _) = snap(&st, &pool);
        assert_eq!((ra, rb), (105_000, 200_000 - q));
        assert_eq!((bal(&st, &ta, &pool), bal(&st, &tb, &pool)), (ra, rb));
        assert!(ra * rb >= 100_000 * 200_000);
    }

    /// Cross-check vectors for `boing-sdk` (`tests/boingPool.test.ts`) and the SDK's copy of the
    /// pool template bytecode. Regenerate with `BOING_WRITE_POOL_VECTORS=1 cargo test -p boing-cli`.
    #[test]
    fn sdk_pool_vectors_and_bytecode_are_in_sync() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let k = key(7);
        let me = account_of(&k);
        let pool = nonce_derived_contract_address(&me, 0);
        let ta = AccountId([0x0a; 32]);
        let tb = AccountId([0x0b; 32]);
        let bob = AccountId([0x0c; 32]);
        let sig = |s: &SignedTransaction| json!({ "txId": format!("0x{}", hex::encode(s.tx.id().0)), "signedHex": crate::token::encode_signed(s).unwrap() });
        let v = json!({
            "secretHex": format!("0x{}", "07".repeat(32)),
            "senderHex": hex_account(&me),
            "poolHex": hex_account(&pool),
            "tokenAHex": hex_account(&ta),
            "tokenBHex": hex_account(&tb),
            "recipientHex": hex_account(&bob),
            "deployV7": sig(&build_pool_deploy_tx(&k, 0, PoolVersion(7))),
            "deployV1": sig(&build_pool_deploy_tx(&k, 0, PoolVersion(1))),
            "setTokens": sig(&build_pool_call_tx(&k, 1, pool, encode_set_tokens_calldata(&ta, &tb), &[ta, tb])),
            "setFeeAdmin": sig(&build_pool_call_tx(&k, 2, pool, encode_set_fee_admin_calldata(&bob), &[])),
            "setFee": sig(&build_pool_call_tx(&k, 3, pool, encode_set_swap_fee_bps_calldata(50), &[])),
            "addLiquidity": sig(&build_pool_call_tx(&k, 4, pool, encode_add_liquidity_calldata(10_000, 40_000, 9_000), &[])),
            "swap": sig(&build_pool_call_tx(&k, 5, pool, encode_swap_calldata(0, 1_000, 3_600), &[tb, ta])),
            "swapTo": sig(&build_pool_call_tx(&k, 6, pool, encode_swap_to_calldata(1, 2_000, 400, &bob), &[ta, tb])),
            "removeLiquidity": sig(&build_pool_call_tx(&k, 7, pool, encode_remove_liquidity_calldata(2_500, 2_000, 9_000), &[ta, tb])),
            "v8": {
                "deploy": sig(&build_pool_deploy_tx(&k, 0, PoolVersion(8))),
                "tokenV2Deploy": sig(&crate::token::build_deploy_tx_versioned(&k, 0, Some("Boing V2".into()), Some("BV2".into()), crate::token::TokenVersion(2))),
                "approveA": sig(&build_approve_tx(&k, 4, ta, pool, 10_000)),
                "approveB": sig(&build_approve_tx(&k, 5, tb, pool, 40_000)),
                "addLiquidity": sig(&build_pool_call_tx(&k, 6, pool, encode_v8_add_liquidity_calldata(10_000, 40_000, 9_000, 0), &[ta, tb])),
                "addLiquidityDeadline": sig(&build_pool_call_tx(&k, 6, pool, encode_v8_add_liquidity_calldata(10_000, 40_000, 9_000, 123_456), &[ta, tb])),
                "swap": sig(&build_pool_call_tx(&k, 8, pool, encode_v8_swap_calldata(0, 1_000, 3_600, 777), &[ta, tb])),
                "swapTo": sig(&build_pool_call_tx(&k, 9, pool, encode_v8_swap_to_calldata(1, 2_000, 400, &bob, 0), &[ta, tb])),
                "removeLiquidity": sig(&build_pool_call_tx(&k, 10, pool, encode_v8_remove_liquidity_calldata(2_500, 2_000, 9_000, 99), &[ta, tb])),
                "allowanceKeyHex": format!("0x{}", hex::encode(boing_execution::ref_fungible_allowance_key(&me, &pool))),
            },
            "lpVaultV2": {
                "configure": format!("0x{}", hex::encode(boing_execution::encode_lp_vault_v2_configure_calldata(&pool, &ta, &tb))),
                "deposit": format!("0x{}", hex::encode(boing_execution::encode_lp_vault_v2_deposit_calldata(10_000, 20_000, 9_000, 77))),
                "withdraw": format!("0x{}", hex::encode(boing_execution::encode_lp_vault_v2_withdraw_calldata(4_000, 3_000, 6_000, 0))),
                "transferShares": format!("0x{}", hex::encode(boing_execution::encode_lp_vault_v2_transfer_shares_calldata(&bob, 1_234))),
                "shareKeyHex": format!("0x{}", hex::encode(boing_execution::lp_vault_v2_share_key(&me))),
            },            "router": {
                "deployV9Pool": sig(&build_pool_deploy_tx(&k, 0, PoolVersion(9))),
                "deploy": sig(&crate::router::build_router_deploy_tx(&k, 1)),
                "approveRouter": sig(&build_approve_tx(&k, 2, ta, pool, 5_000)),
                "swap2": sig(&crate::router::build_router_swap_tx(&k, 3, pool, &[
                    crate::router::RouteHop { pool: ta, direction: 0, token_in: tb, token_out: me },
                    crate::router::RouteHop { pool: tb, direction: 1, token_in: me, token_out: bob },
                ], 5_000, 4_000, &bob, 321, &[])),
            },
        });
        let norm_file = |p: &std::path::Path| {
            std::fs::read_to_string(p)
                .unwrap_or_default()
                .replace("\r\n", "\n")
        };
        let vec_path = root.join("boing-sdk/tests/fixtures/pool-cli-vectors.json");
        let vec_text = format!("{}\n", serde_json::to_string_pretty(&v).unwrap());
        let mut ts = String::from(
            "// GENERATED by `BOING_WRITE_POOL_VECTORS=1 cargo test -p boing-cli` from\n// boing_execution::constant_product_pool_bytecode{,_v2..v10} + native_dex_custody_router_bytecode + native_lp_vault_v2_bytecode. Do not edit.\n\n/** Native constant-product pool template bytecode hex by version (v1..v10; v10 = u128 amounts for 18-decimal tokens). */\nexport const NATIVE_CP_POOL_BYTECODE_HEX: Readonly<Record<1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10, `0x${string}`>> = {\n",
        );
        for n in 1..=10u8 {
            ts.push_str(&format!(
                "  {n}: '0x{}',\n",
                hex::encode(PoolVersion(n).bytecode())
            ));
        }
        ts.push_str("};\n");
        ts.push_str(&format!(
            "\n/** Custody router v1 (`boing pool router-deploy`), pulls via transfer_from, 1..=4 hops. */\nexport const NATIVE_DEX_CUSTODY_ROUTER_BYTECODE_HEX: `0x${{string}}` =\n  '0x{}';\n",
            hex::encode(boing_execution::native_dex_custody_router_bytecode())
        ));
        ts.push_str(&format!(
            "\n/** LP vault v2 for custody (v8/v9) pools: deposit -> add_liquidity -> shares; withdraw -> burn -> remove_liquidity_to. */\nexport const NATIVE_LP_VAULT_V2_BYTECODE_HEX: `0x${{string}}` =\n  '0x{}';\n",
            hex::encode(boing_execution::native_lp_vault_v2_bytecode())
        ));        let ts_path = root.join("boing-sdk/src/defaultNativeCpPoolBytecodeHex.ts");
        let tok_ts = format!(
            "// GENERATED by `BOING_WRITE_POOL_VECTORS=1 cargo test -p boing-cli` from\n// boing_execution::reference_fungible_template_v2_bytecode. Do not edit.\n\n/** Reference fungible template v2 (approve / transfer_from / allowance). */\nexport const DEFAULT_REFERENCE_FUNGIBLE_TEMPLATE_V2_BYTECODE_HEX: `0x${{string}}` =\n  '0x{}';\n",
            hex::encode(boing_execution::reference_fungible_template_v2_bytecode())
        );
        let tok_path = root.join("boing-sdk/src/defaultReferenceFungibleTemplateV2BytecodeHex.ts");
        let wb_ts = format!(
            "// GENERATED by `BOING_WRITE_POOL_VECTORS=1 cargo test -p boing-cli` from\n// boing_execution::wrapped_boing_bytecode. Do not edit.\n\n/** Wrapped BOING (wBOING): payable deposit mints 1:1, withdraw burns and pays native BOING; token v2 compatible, 18 decimals. Needs the native-value rules (testnet height 1150). */\nexport const WRAPPED_BOING_BYTECODE_HEX: `0x${{string}}` =\n  '0x{}';\n",
            hex::encode(boing_execution::wrapped_boing_bytecode())
        );
        let wb_path = root.join("boing-sdk/src/defaultWrappedBoingBytecodeHex.ts");
        if std::env::var("BOING_WRITE_POOL_VECTORS").is_ok() {
            // Only rewrite files whose content changed (line endings ignored).
            for (p, text) in [(&vec_path, &vec_text), (&ts_path, &ts), (&tok_path, &tok_ts), (&wb_path, &wb_ts)] {
                if norm_file(p) != *text {
                    std::fs::write(p, text).unwrap();
                }
            }
        }
        assert_eq!(norm_file(&wb_path), wb_ts, "defaultWrappedBoingBytecodeHex.ts out of date");
        assert_eq!(
            norm_file(&tok_path),
            tok_ts,
            "defaultReferenceFungibleTemplateV2BytecodeHex.ts out of date"
        );
        let norm = |s: String| s.replace("\r\n", "\n");
        assert_eq!(
            norm(std::fs::read_to_string(&vec_path).expect("vectors")),
            vec_text,
            "pool-cli-vectors.json out of date"
        );
        assert_eq!(
            norm(std::fs::read_to_string(&ts_path).expect("bytecode ts")),
            ts,
            "defaultNativeCpPoolBytecodeHex.ts out of date"
        );
    }
}
