# Native AMM calldata (v1 — encoders + reference pool bytecode)

**Status:** **Implemented** in `crates/boing-execution/src/native_amm.rs` (`constant_product_pool_bytecode`, calldata encoders) and `boing-sdk/src/nativeAmm.ts`. The on-chain program is a **ledger-only** MVP (no `CALL` to reference tokens); reserves and trade sizes must stay in **u64 range** for VM `Mul` parity (see module rustdoc).

**Convention:** Extends the **96-byte reference call** style from [BOING-REFERENCE-TOKEN.md](BOING-REFERENCE-TOKEN.md): word0 = selector in the **last byte** (offset 31); additional arguments follow in 32-byte words. Calls longer than 96 bytes use **contiguous 32-byte words** after the first 96 bytes.

---

## Word layout

| Word index | Byte offset | Content |
|------------|-------------|---------|
| 0 | 0–31 | Selector word: zeros + **selector** in byte 31 |
| 1 | 32–63 | Argument A (meaning per method) |
| 2 | 64–95 | Argument B |
| 3+ | 96+ | Optional further args (big-endian `u128` in **low 16 bytes** unless noted) |

---

## Selectors (pool contract, draft)

| Byte 31 (hex) | Name | Args (word1, word2, word3, …) |
|---------------|------|--------------------------------|
| `0x10` | `swap` | `direction` — `u128` 0 = A→B, 1 = B→A (high bytes zero); `amount_in` — `u128`; `min_out` — `u128` |
| `0x11` | `add_liquidity` | `amount_a` — `u128`; `amount_b` — `u128`; `min_liquidity` — `u128` (128-bit total; 96 bytes if `min_liquidity` omitted in MVP—then use **two-word** 64-byte layout only if contract supports it) |
| `0x12` | `remove_liquidity` | `liquidity_burn` — `u128`; `min_a` — `u128`; `min_b` — `u128` |

**Note:** `add_liquidity` as specified needs **128 bytes** (4 words). Pool bytecode must define whether word3 is required or optional with a default.

---

## Example hex (illustrative only)

`swap` direction A→B, `amount_in = 1_000_000`, `min_out = 900_000`:

- Word0: `0x` + 62 zeros + `10`
- Word1: zeros + big-endian u128 `0` (direction A→B)
- Word2: zeros + big-endian u128 `1000000`
- Word3: zeros + big-endian u128 `900000`

Concatenate 4 × 32 bytes → `0x` + 256 hex chars for `calldata` in `contract_call`.

---

## Factory (optional)

If a **factory** contract deploys pools, define a separate selector table here (e.g. `0x20` = `create_pool`). Until then, **omit factory** from calldata spec and use config-file **pool addresses**.

---

## Bytecode hex (local)

```bash
cargo run -p boing-execution --example dump_native_amm_pool
```

Pipe into your deploy / `boing_qaCheck` flow; then set `VITE_BOING_NATIVE_AMM_POOL` on **boing.finance** to the deployed pool `AccountId` (see checklist Phase 5).

---

## Access list (reminder)

Each `swap` must include **read/write** entries for: **signer**, **pool contract**, and **both token contracts** (if reference-token transfers are invoked internally). Validate with simulation ([NATIVE-AMM-INTEGRATION-CHECKLIST.md](NATIVE-AMM-INTEGRATION-CHECKLIST.md) Phase 2).

---

## SDK / Rust

- [x] **`boing_execution`:** `encode_swap_calldata`, `encode_add_liquidity_calldata`, `encode_remove_liquidity_calldata`, `constant_product_pool_bytecode`, `constant_product_amount_out`, `reserve_a_key` / `reserve_b_key`.
- [x] **`boing-sdk`:** `encodeNativeAmmSwapCalldata`, `encodeNativeAmmAddLiquidityCalldata`, `encodeNativeAmmRemoveLiquidityCalldata`, `constantProductAmountOut`, hex helper for swap.
