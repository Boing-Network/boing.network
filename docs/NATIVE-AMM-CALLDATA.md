# Native AMM calldata (v1 — encoders + reference pool bytecode)

**Status:** **Implemented** in `crates/boing-execution/src/native_amm.rs` (`constant_product_pool_bytecode`, calldata encoders) and `boing-sdk/src/nativeAmm.ts`. The on-chain program is a **ledger-only** MVP (no `CALL` to reference tokens); reserves and trade sizes must stay in **u64 range** for VM `Mul` parity (see module rustdoc).

**Convention:** Extends the **96-byte reference call** style from [BOING-REFERENCE-TOKEN.md](BOING-REFERENCE-TOKEN.md): word0 = selector in the **last byte** (offset 31); additional arguments follow in 32-byte words. Calls longer than 96 bytes use **contiguous 32-byte words** after the first 96 bytes.

---

## Frozen MVP scope (checklist Phase 0)

| Item | Decision |
|------|----------|
| **Surface** | Single **constant-product pool** contract with **two in-storage reserves** (ledger units). No reference-token `CALL` in this bytecode revision. |
| **Calldata** | Documented below (`swap` / `add_liquidity` / `remove_liquidity` selectors and 128-byte layouts). |
| **Logs / events** | **None** in current pool bytecode — indexers cannot rely on `LOG` topics yet. When logs are added, define **topic0** + indexed fields here and in [BOING-PATTERN-AMM-LIQUIDITY.md](BOING-PATTERN-AMM-LIQUIDITY.md). |
| **QA `purpose_category`** | Pool bytecode deploys (when not bare) should use categories accepted by `boing_qa` (e.g. **`dapp`** or **`tooling`**) per [QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md). |
| **Factory** | **Not in MVP** — one **configured pool `AccountId`** per environment (`nativeConstantProductPool` / env override on boing.finance). |

---

## Minimal access list by tx type (Phase 2)

| Calldata / action | Accounts on **read** and **write** (MVP) | If pool adds token `CALL` |
|-------------------|------------------------------------------|---------------------------|
| `swap` (`0x10`) | Signer + pool | Add each token contract account touched |
| `add_liquidity` (`0x11`) | Signer + pool | Add token contracts as above |
| `remove_liquidity` (`0x12`) | Signer + pool (no-op today) | Same rule when implemented |

Always validate with **`boing_simulateTransaction`**: when **`access_list_covers_suggestion`** is `false`, merge **`suggested_access_list`** into the declared list (see `boing-sdk` / boing.finance `mergeAccessListWithSimulation`).

---

## Slippage, deadline, upgrade policy

- **Slippage:** On-chain enforcement uses **`min_out`** on `swap` (and `min_liquidity` / `min_a` / `min_b` on liquidity methods when active). The UI computes `min_out` from an off-chain quote and user slippage bps — **rounding** can still cause reverts if reserves move; there is **no block deadline** field in current calldata (client-only urgency if desired).
- **Upgrades:** MVP pool bytecode is **immutable** once deployed; there is **no admin pause** in this revision — communicate that in product copy.

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

Pipe into your deploy / **`boing_qaCheck`** flow (e.g. `purpose_category`: `dapp`). CI asserts bytecode passes **`boing_qa`** rules via `constant_product_pool_bytecode_passes_protocol_qa` in `boing-execution`. Then set **`REACT_APP_BOING_NATIVE_AMM_POOL`** (boing.finance) to the deployed pool `AccountId` (see checklist Phase 5).

---

## Access list (reminder)

**Pool-only MVP (current bytecode):** **read/write** = **signer** + **pool** (matches `suggested_parallel_access_list` and `boing-sdk` `buildNativeConstantProductPoolAccessList`). If the pool later **`CALL`s reference-token contracts**, widen the list to those accounts and merge with simulation (`mergeNativePoolAccessListWithSimulation`).

---

## Pool metadata without a separate HTTP API (checklist A3.3)

MVP pools have **no logs** and **no factory registry** on-chain. Partners can treat **`pool_account_id` + JSON-RPC** as the lightweight “metadata API”:

1. **Reserves** — `boing_getContractStorage(pool, reserve_a_key)` and `boing_getContractStorage(pool, reserve_b_key)` (keys: 32-byte hex from this doc / `boing_execution`; values: u128 in low 16 bytes of the word).
2. **Optional batch** — HTTP clients may POST a JSON-RPC **batch** array with two `boing_getContractStorage` requests to reduce round-trips.
3. **Future** — When the pool emits **`LOG`** topics or a factory exists, prefer **`boing_getLogs`** or an indexer for discovery; see [NATIVE-AMM-INTEGRATION-CHECKLIST.md](NATIVE-AMM-INTEGRATION-CHECKLIST.md).

---

## SDK / Rust

- [x] **`boing_execution`:** `encode_swap_calldata`, `encode_add_liquidity_calldata`, `encode_remove_liquidity_calldata`, `constant_product_pool_bytecode`, `constant_product_amount_out`, `reserve_a_key` / `reserve_b_key`.
- [x] **`boing-sdk`:** `encodeNativeAmmSwapCalldata`, `encodeNativeAmmAddLiquidityCalldata`, `encodeNativeAmmRemoveLiquidityCalldata`, `constantProductAmountOut`, hex helper for swap.
- [x] **`boing-sdk` `nativeAmmPool`:** `buildNativeConstantProductPoolAccessList`, `buildNativeConstantProductContractCallTx`, `mergeNativePoolAccessListWithSimulation`.
