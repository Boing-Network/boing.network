# Native AMM calldata (v1–v4 encoders + reference pool bytecode)

**Status:** **Implemented** in `crates/boing-execution/src/native_amm.rs` and `boing-sdk/src/nativeAmm.ts`. **v1** — `constant_product_pool_bytecode` — ledger-only reserves. **v2** — `constant_product_pool_bytecode_v2` — one-time **`set_tokens`** plus nested **`Call` (`0xf1`)** to reference-token contracts on **swap output** and **`remove_liquidity`** payouts ([TECHNICAL-SPECIFICATION.md](TECHNICAL-SPECIFICATION.md) §7.2, [BOING-REFERENCE-TOKEN.md](BOING-REFERENCE-TOKEN.md)). **v3** — `constant_product_pool_bytecode_v3` — same as v1 plus **on-chain swap fee bps** in storage (`swap_fee_bps_key`) and selector **`set_swap_fee_bps` (`0x14`)** when **total LP == 0**. **v4** — `constant_product_pool_bytecode_v4` — v2 token hooks **plus** the same configurable fee as v3. **v5** — `constant_product_pool_bytecode_v5` — v4 **plus** **`swap_to` (`0x15`)**: **160-byte** swap calldata with an explicit **output recipient** `AccountId` so reference-token **`transfer`** can target the end user when the pool is called from a **router contract** (Boing VM `Caller` is the router; recipient word is the beneficiary). CREATE2 salt: **`NATIVE_CP_POOL_CREATE2_SALT_V5`**. Reserves and trade sizes stay in **u64** on the pool ledger (encoded as u128 words); the VM **`Mul`** opcode uses full **256×256 → 256** multiplication so on-chain fee math matches wide quotes.

**Convention:** Extends the **96-byte reference call** style from [BOING-REFERENCE-TOKEN.md](BOING-REFERENCE-TOKEN.md): word0 = selector in the **last byte** (offset 31); additional arguments follow in 32-byte words. Calls longer than 96 bytes use **contiguous 32-byte words** after the first 96 bytes.

---

## Frozen MVP scope (checklist Phase 0)

| Item | Decision |
|------|----------|
| **Surface** | Single **constant-product pool** with **two in-storage reserves**. **v1:** ledger-only. **v2:** optional on-chain payout via reference-token **`transfer(Caller, amount)`** when token ids are set (pool is **`Caller`** on the token). |
| **Calldata** | Documented below (`swap` / `add_liquidity` / `remove_liquidity` selectors and 128-byte layouts). |
| **Logs / events** | **`Log2`** on successful **`swap`**, **`add_liquidity`**, **`remove_liquidity`** — fixed **topic0** strings (32 bytes, ASCII + zero pad) + **topic1 = caller**; **data** = three amount words (§ Logs). |
| **QA `purpose_category`** | Pool bytecode deploys (when not bare) should use categories accepted by `boing_qa` (e.g. **`dapp`** or **`tooling`**) per [QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md). |
| **Factory** | **Not in MVP** — one **configured pool `AccountId`** per environment (`nativeConstantProductPool` / env override on boing.finance). |

---

## Minimal access list by tx type (Phase 2)

| Calldata / action | Accounts on **read** and **write** (MVP) | If pool adds token `CALL` |
|-------------------|------------------------------------------|---------------------------|
| `swap` (`0x10`) | Signer + pool | **v2 with tokens:** include **output-side** token contract for that swap direction |
| `add_liquidity` (`0x11`) | Signer + pool | **v2:** still ledger-only; no token `CALL` in this revision |
| `remove_liquidity` (`0x12`) | Signer + pool | **v2 with tokens:** include **both** token contracts if either slot is non-zero |
| `set_tokens` (`0x13`) | Signer + pool | **v2:** include both token ids you pass (read/write) |
| `set_swap_fee_bps` (`0x14`) | Signer + pool | **v3/v4 only**; only before first `add_liquidity` (**total LP == 0**) |
| `swap_to` (`0x15`) | Signer + pool (+ output token if v5) | **v5 only**; same as `swap` plus **recipient** word; declare **recipient** in access list if it is a **new** account (rare) |
| `remove_liquidity_to` (`0x16`) | Signer + pool | **v5 only**; **v2 with tokens:** include both token contracts; declare **`recipient_a` / `recipient_b`** if they are **new** accounts (rare) |

**SDK (today):** `buildNativeConstantProductPoolAccessList` / `buildNativeConstantProductContractCallTx` / `mergeNativePoolAccessListWithSimulation` accept optional **`NativePoolAccessListOptions.additionalAccountsHex32`** so dApps can declare token contract ids before bytecode gains `CALL`; simulation merge still widens the list when needed.

Always validate with **`boing_simulateTransaction`**: when **`access_list_covers_suggestion`** is `false`, merge **`suggested_access_list`** into the declared list (see `boing-sdk` / boing.finance `mergeAccessListWithSimulation`).

---

## Slippage, deadline, upgrade policy

- **Slippage:** On-chain enforcement uses **`min_out`** on `swap` (and `min_liquidity` / `min_a` / `min_b` on liquidity methods when active). The UI computes `min_out` from an off-chain quote and user slippage bps — **rounding** can still cause reverts if reserves move; there is **no block deadline** field in current calldata (client-only urgency if desired).
- **Upgrades:** MVP pool bytecode is **immutable** once deployed; there is **no admin pause** in this revision — communicate that in product copy.

### Swap fee (default bps + v3/v4 on-chain override)

The pool applies a **swap fee on the output** of the no-fee constant-product step (u64-safe; avoids `r_in * 10^4` overflow from fee-on-input formulas at full reserve range):

- **Default fee:** **`NATIVE_CP_SWAP_FEE_BPS = 30`** (0.30%). Same constant in `boing_execution::native_amm`, `boing-sdk` `nativeAmm.ts`, and **v1/v2** bytecode (fixed fee).
- **v3/v4:** Fee is read from **`swap_fee_bps_key`** (`k[31] = 0x07`). Storage **`0`** means **unset**: on **first** `add_liquidity`, the pool writes **`NATIVE_CP_SWAP_FEE_BPS`**; on **swap**, **`SLoad` 0** is treated as **`30`** in scratch. **`set_swap_fee_bps`** (calldata § Selectors) may set **`1..=10_000`** only while **total LP supply == 0**; it **cannot** set fee to **`0`** via that selector.
- **On-chain bounds:** If stored fee **`> 10_000`**, swap **aborts**.
- **Formula:** Let `dy_raw = ⌊ r_out · Δ_in / (r_in + Δ_in) ⌋`. Then **`dy = ⌊ dy_raw · (10_000 - fee_bps) / 10_000 ⌋`**. If `dy == 0` after the fee step, the swap **aborts** (no state change).
- **Quotes:** **v1/v2:** **`constant_product_amount_out_after_fee`** (Rust) or **`constantProductAmountOut`** (TS). **v3/v4:** read storage (or assume default before first mint); if raw word is **`0`**, quote with **`NATIVE_CP_SWAP_FEE_BPS`**; else use **`constant_product_amount_out_after_fee_with_bps`** / **`constantProductAmountOutWithFeeBps`**. Raw CP step only: **`constant_product_amount_out`** / **`constantProductAmountOutNoFee`**.

The fee accrues to LPs implicitly (traders receive less `out` token per swap).

---

## Contract storage (pool ledger)

Values are 32-byte words; u128 amounts use **big-endian in the low 16 bytes** (same as reference-token amount words).

| Key | Rust helper | Notes |
|-----|-------------|--------|
| Reserve A | `reserve_a_key()` — `k[31] = 0x01` | Token A side |
| Reserve B | `reserve_b_key()` — `k[31] = 0x02` | Token B side |
| Total LP supply | `total_lp_supply_key()` — `k[31] = 0x03` | Increases on `add_liquidity`, decreases on `remove_liquidity` |
| Signer LP balance | `lp_balance_storage_key(&caller.0)` | `caller_id ^ LP_BALANCE_STORAGE_XOR` (32-byte XOR constant in `native_amm.rs` / `nativeAmmLpBalanceStorageKeyHex` in TS) |
| **v2** Token A id | `token_a_key()` — `k[31] = 0x04` | Reference-token contract for side A (`0` = no on-chain payout for A on remove / not used as swap-out for B→A when zero) |
| **v2** Token B id | `token_b_key()` — `k[31] = 0x05` | Same for side B |
| **v2** Configured | `tokens_configured_key()` — `k[31] = 0x06` | Non-zero after successful **`set_tokens`**; further **`set_tokens`** calls **no-op** (stop) |
| **v3/v4** Swap fee bps | `swap_fee_bps_key()` — `k[31] = 0x07` | Output-side fee in basis points; **`0`** = unset (see § Swap fee); **`set_swap_fee_bps`** writes **`1..=10_000`** |

---

## Logs (`Log2` — NAMM-3)

On **successful** completion (before `STOP`), the pool appends one execution log:

| Field | Content |
|-------|---------|
| **topics[0]** | Fixed 32-byte **topic0** (UTF-8 label, zero-padded). Rust: `NATIVE_AMM_TOPIC_SWAP`, `NATIVE_AMM_TOPIC_ADD_LIQUIDITY`, `NATIVE_AMM_TOPIC_REMOVE_LIQUIDITY`. TS: `NATIVE_AMM_TOPIC_*_HEX` in `nativeAmm.ts`. |
| **topics[1]** | **`caller`** account id (32 bytes, same encoding as other log topics). |
| **data** | **96 bytes** = three consecutive **32-byte words** (u128 each in the **low 16 bytes**, high 16 zero), big-endian. |

**Data layout by method**

| Method | Word 0 (bytes 0–31) | Word 1 (32–63) | Word 2 (64–95) |
|--------|---------------------|----------------|----------------|
| `swap` (`0x10`) | `direction` (same as calldata word1) | `amount_in` | `amount_out` (after swap fee) |
| `add_liquidity` (`0x11`) | `amount_a` | `amount_b` | `lp_minted` |
| `remove_liquidity` (`0x12`) | `liquidity_burn` | `amount_a_out` | `amount_b_out` |

**Indexing:** Filter receipts with **`topics[0]`** equals the pool’s event id hex; **`topics[1]`** filters by trader. **Failed** calls (slippage, zero burn, etc.) emit **no** log.

**SDK:** Parse **`data`** with **`decodeNativeAmmLogDataU128Triple`** (`boing-sdk` `nativeAmmPool.ts`) → `[word0, word1, word2]` as `bigint`. For full **`Log2`** rows (two topics + data), use **`tryParseNativeAmmLog2`** / **`tryParseNativeAmmRpcLogEntry`** (`nativeAmmLogs.ts`).

---

## Word layout

| Word index | Byte offset | Content |
|------------|-------------|---------|
| 0 | 0–31 | Selector word: zeros + **selector** in byte 31 |
| 1 | 32–63 | Argument A (meaning per method) |
| 2 | 64–95 | Argument B |
| 3+ | 96+ | Optional further args (big-endian `u128` in **low 16 bytes** unless noted) |

---

## Selectors (pool contract)

| Byte 31 (hex) | Name | Args (word1, word2, word3, …) |
|---------------|------|--------------------------------|
| `0x10` | `swap` | `direction` — `u128` 0 = A→B, 1 = B→A (high bytes zero); `amount_in` — `u128`; `min_out` — `u128` |
| `0x11` | `add_liquidity` | `amount_a` — `u128`; `amount_b` — `u128`; `min_liquidity` — `u128` (128-bit total; 96 bytes if `min_liquidity` omitted in MVP—then use **two-word** 64-byte layout only if contract supports it) |
| `0x12` | `remove_liquidity` | `liquidity_burn` — `u128`; `min_a` — `u128`; `min_b` — `u128` |
| `0x13` | **`set_tokens` (v2 only)** | **96-byte** calldata: word1 = **token A** `AccountId` (32 bytes); word2 = **token B** `AccountId` (32 bytes). All-zero id = “no token contract” for that side. **Once** per pool after deploy; writes `tokens_configured_key`. |
| `0x14` | **`set_swap_fee_bps` (v3/v4 only)** | **64-byte** calldata: word1 = **`fee_bps`** (`u128` in low 16 bytes). **`1 ≤ fee_bps ≤ 10_000`**. Only when **`total_lp_supply == 0`**; otherwise the call **no-ops** (stop). |
| `0x15` | **`swap_to` (v5 only)** | **160-byte** calldata: word1 = `direction`; word2 = `amount_in`; word3 = `min_out`; word4 = **`recipient`** — 32-byte **`AccountId`** (raw id, not a u128 amount). On-chain token **`transfer`** for swap output uses **`recipient`** instead of the VM **`Caller`** (so routers can deliver to the signer). Legacy **`swap` (`0x10`)** on v5 still pays output to **`Caller`**. |
| `0x16` | **`remove_liquidity_to` (v5 only)** | **192-byte** calldata: words 1–3 as **`remove_liquidity`**; word4 = **`recipient_a`**; word5 = **`recipient_b`** — each a 32-byte **`AccountId`**. Reference-token **`transfer`** for each non-zero token side uses the corresponding recipient instead of **`Caller`**. Legacy **`remove_liquidity` (`0x12`)** on v5 still pays on-chain tokens to **`Caller`**. |

**Note:** `add_liquidity` uses **128 bytes** (4 words). On **success**, the pool **`Return`s** **32** bytes: **LP minted in this call** as **u128** in the **low 16 bytes** of the word (same encoding as amount words). Other selectors do not use contract return data in the current bytecode. Rust: `decode_add_liquidity_return_lp_minted`; TypeScript: `decodeNativeAmmAddLiquidityReturnLpMinted` (`boing-sdk` `nativeAmmPool.ts`).

### v2 settlement semantics

- **`swap`:** After updating ledger reserves, if the **output** token id for the trade direction is non-zero, the pool executes **`CALL`** on that contract with **reference `transfer` calldata** ([BOING-REFERENCE-TOKEN.md](BOING-REFERENCE-TOKEN.md)): **`to` = transaction signer**, **`amount` = amount out** (after fee). The pool must hold sufficient balance on that token contract; **`add_liquidity` does not** pull ERC-style deposits in this revision.
- **`remove_liquidity`:** After burning LP and updating reserves, **non-zero** token A / B ids each trigger a **`transfer(Caller, amount_out)`** to the immediate caller for that side.
- **`remove_liquidity_to` (v5):** Same, but **`transfer(to, …)`** uses **`recipient_a` / `recipient_b`** from calldata for the respective side (router-safe when LP removal is invoked via a router contract).
- **Input asset** on swap is still **ledger-only** in v2 (no pull from user’s token balance inside the pool call). Product flows that need atomic “user pays token A” require a **future** extension (e.g. user `transfer` in a prior tx, or a richer token ABI).

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

Prints **four** lines: **v1**–**v4** bytecode hex (stderr comments label byte lengths).

**Pool `AccountId` (nonce deploy, no salt):** `cargo run -p boing-primitives --example nonce_derived_contract_address -- 0x<DEPLOYER_64_HEX> <nonce>` — see [OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md](OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md).

### CREATE2 (recommended for canonical public pool)

Use a fixed **32-byte salt** so the pool address depends only on **deployer + bytecode** (not on unrelated txs that bump nonce). Protocol: `create2_contract_address` in `boing_primitives` / [TECHNICAL-SPECIFICATION.md](TECHNICAL-SPECIFICATION.md) §4.4.

- **v1 salt (ledger-only pool):** `NATIVE_CP_POOL_CREATE2_SALT_V1` — label `BOING_NATIVECP_C2V1`.
- **v2 salt (token-hook pool):** `NATIVE_CP_POOL_CREATE2_SALT_V2` — label `BOING_NATIVECP_C2V2`. **Different bytecode** ⇒ **different address** than v1 for the same deployer.
- **v3 salt (ledger + configurable fee):** `NATIVE_CP_POOL_CREATE2_SALT_V3` — `BOING_NATIVECP_C2V3`.
- **v4 salt (v2 + configurable fee):** `NATIVE_CP_POOL_CREATE2_SALT_V4` — `BOING_NATIVECP_C2V4`.
- **Print salt hex:** `cargo run -p boing-execution --example print_native_cp_create2_salt` (prints `SALT_V1=` … `SALT_V4=`).
- **Predict pool id:** use the **matching** salt + bytecode (pick the correct line from `dump_native_amm_pool`), then  
  `cargo run -p boing-primitives --example create2_contract_address -- 0x<DEPLOYER> 0x<SALT> path/to/pool.hex`
- **SDK:** `predictNativeCpPoolCreate2Address` / **`predictNativeCpPoolV2Create2Address`** / **`predictNativeCpPoolV3Create2Address`** / **`predictNativeCpPoolV4Create2Address`** with the matching **`NATIVE_CP_POOL_CREATE2_SALT_V*`** (`boing-sdk` `create2.ts`).
- **Deploy:** `create2_salt: Some(NATIVE_CP_POOL_CREATE2_SALT_V1)` or **`…_V2`** as appropriate (TS: `buildDeployWithPurposeTransaction`). Regression: `native_amm_create2_deploy_add_swap_via_rpc` in `boing-node` tests (v1).

Pipe into your deploy / **`boing_qaCheck`** flow (e.g. `purpose_category`: `dapp`). CI asserts bytecode passes **`boing_qa`** (`constant_product_pool_bytecode_passes_protocol_qa` and v2 twin in `boing-execution`). Then set the pool `AccountId` in boing.finance: **`frontend/src/config/boingCanonicalTestnetPool.js`** and/or **`REACT_APP_BOING_NATIVE_AMM_POOL`**, and **`nativeConstantProductPool`** in `contracts.js` (see checklist Phase 5).

---

## Access list (reminder)

**v1:** **read/write** = **signer** + **pool**. **v2** `swap` / `remove_liquidity` with non-zero token slots: **read/write** must include **each token contract** the pool may **`CALL`** (use **`additionalAccountsHex32`** and **`mergeNativePoolAccessListWithSimulation`**).

---

## Pool metadata without a separate HTTP API (checklist A3.3)

MVP pools have **no factory registry** on-chain. Partners can treat **`pool_account_id` + JSON-RPC** as the lightweight “metadata API”:

1. **Reserves** — `boing_getContractStorage(pool, reserve_a_key)` and `boing_getContractStorage(pool, reserve_b_key)` (keys: 32-byte hex from this doc / `boing_execution`; values: u128 in low 16 bytes of the word). Optionally read **total LP**, a signer’s **LP balance**, and **v3/v4** **`swap_fee_bps_key`** (§ Contract storage / § Swap fee).
2. **Optional batch** — HTTP clients may POST a JSON-RPC **batch** array with two `boing_getContractStorage` requests to reduce round-trips.
3. **Logs** — Pool **`Log2`** events (§ Logs); use **`boing_getLogs`** with **`topics`** + contract address, or scan receipt **`logs`** from **`boing_getTransactionReceipt`**. **Future:** factory registry or subgraph for multi-pool discovery; see [NATIVE-AMM-INTEGRATION-CHECKLIST.md](NATIVE-AMM-INTEGRATION-CHECKLIST.md).

---

## SDK / Rust

- [x] **`boing_execution`:** `encode_swap_calldata`, **`encode_swap_to_calldata`**, `encode_add_liquidity_calldata`, `encode_remove_liquidity_calldata`, **`encode_remove_liquidity_to_calldata`**, **`encode_set_tokens_calldata`**, **`encode_set_swap_fee_bps_calldata`**, `constant_product_pool_bytecode`, **`constant_product_pool_bytecode_v2`**, **`constant_product_pool_bytecode_v3`**, **`constant_product_pool_bytecode_v4`**, **`constant_product_pool_bytecode_v5`**, `constant_product_amount_out`, **`constant_product_amount_out_after_fee`**, **`constant_product_amount_out_after_fee_with_bps`**, **`NATIVE_CP_SWAP_FEE_BPS`**, **`SELECTOR_SET_SWAP_FEE_BPS`**, **`NATIVE_AMM_TOPIC_*`**, **`NATIVE_CP_POOL_CREATE2_SALT_V1` … `V5`**, `reserve_*` / `token_*_key` / `tokens_configured_key` / **`swap_fee_bps_key`** / `total_lp_supply_key` / `lp_balance_storage_key` / `LP_BALANCE_STORAGE_XOR`.
- [x] **`boing-sdk`:** `encodeNativeAmmSwapCalldata`, **`encodeNativeAmmSwapToCalldata`**, `encodeNativeAmmAddLiquidityCalldata`, `encodeNativeAmmRemoveLiquidityCalldata`, **`encodeNativeAmmRemoveLiquidityToCalldata`**, **`encodeNativeAmmSetTokensCalldata`**, **`encodeNativeAmmSetSwapFeeBpsCalldata`**, **`constantProductAmountOut`** (default 30 bps), **`constantProductAmountOutWithFeeBps`**, **`constantProductAmountOutNoFee`**, **`NATIVE_CP_SWAP_FEE_BPS`**, **`SELECTOR_NATIVE_AMM_SET_SWAP_FEE_BPS`**, **`NATIVE_AMM_TOPIC_*_HEX`**, hex helpers.
- [x] **`boing-sdk` `nativeAmmPool`:** `buildNativeConstantProductPoolAccessList`, `buildNativeConstantProductContractCallTx`, `mergeNativePoolAccessListWithSimulation`; **`NATIVE_CONSTANT_PRODUCT_RESERVE_*_KEY_HEX`**, **`NATIVE_CONSTANT_PRODUCT_TOTAL_LP_KEY_HEX`**, **`NATIVE_CONSTANT_PRODUCT_TOKEN_*_KEY_HEX`**, **`NATIVE_CONSTANT_PRODUCT_TOKENS_CONFIGURED_KEY_HEX`**, **`NATIVE_CONSTANT_PRODUCT_SWAP_FEE_BPS_KEY_HEX`**, **`nativeAmmLpBalanceStorageKeyHex`**, **`decodeBoingStorageWordU128`**, **`decodeNativeAmmLogDataU128Triple`**, **`fetchNativeConstantProductReserves`**, **`fetchNativeConstantProductTotalLpSupply`**, **`fetchNativeConstantProductSwapFeeBps`**, **`fetchNativeAmmSignerLpBalance`**, **`fetchNativeConstantProductPoolSnapshot`** (§ Pool metadata + § Logs).
