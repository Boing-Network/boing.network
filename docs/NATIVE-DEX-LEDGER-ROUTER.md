# Native DEX ledger router (Boing VM)

Bytecode:

- **v1:** `native_dex_ledger_router_bytecode()` — `crates/boing-execution/src/native_dex_ledger_router.rs`
- **v2:** `native_dex_ledger_router_bytecode_v2()` — forwards **160-byte** inner calldata (v5 **`swap_to`**)
- **v3:** `native_dex_ledger_router_bytecode_v3()` — forwards **192-byte** inner calldata (v5 **`remove_liquidity_to`**)

## Purpose

Single **entry contract** that **`Call`s** a native constant-product **pool** with forwarded pool calldata.

## Calldata

### v1 (**192** bytes)

| Word | Offset | Content |
|------|--------|---------|
| 0 | 0 | Selector **`0xE0`** (low byte) |
| 1 | 32 | Target **pool** `AccountId` |
| 2–4 | 64–191 | **Inner** pool calldata (**128** bytes) |

### v2 (**224** bytes)

| Word | Offset | Content |
|------|--------|---------|
| 0 | 0 | Selector **`0xE1`** (low byte) |
| 1 | 32 | Target **pool** `AccountId` |
| 2–6 | 64–223 | **Inner** pool calldata (**160** bytes; e.g. v5 **`swap_to`**) |

### v3 (**256** bytes)

| Word | Offset | Content |
|------|--------|---------|
| 0 | 0 | Selector **`0xE2`** (low byte) |
| 1 | 32 | Target **pool** `AccountId` |
| 2–7 | 64–255 | **Inner** pool calldata (**192** bytes; v5 **`remove_liquidity_to`**) |

## Caller semantics (critical)

Inside the pool, **`Caller`** is the **router contract**, not the transaction signer.

- **v1 router + ledger-only pools (native CP v1):** safe (no reference-token payout to `Caller`).
- **v1 router + token-hook pools (native CP v2 / v4):** **unsafe** for swaps/removes that pay **`Caller`** — payouts would hit the router.
- **v2 router + v5 pool + `swap_to`:** reference output token **`transfer`** uses the **recipient** word in inner calldata — router-safe for that path.
- **v3 router + v5 pool + `remove_liquidity_to`:** reference-token **`transfer`** uses **`recipient_a` / `recipient_b`** from inner calldata — router-safe for that path (LP must be held by the router contract if the router is the caller to the pool).

## Multi-hop (documentation)

There is no dedicated “multi-hop” router artifact in-repo yet. A common pattern: **sequence of `ContractCall`s** (or an app-level batch) where each step uses **`swap_to`** / **`remove_liquidity_to`** with explicit recipients so intermediate contracts do not custody user tokens. **v2** ledger router can forward **one** **`swap_to`** per hop; **v3** can forward **one** **`remove_liquidity_to`** per hop when each hop targets a **v5** pool.

## CREATE2

- v1 salt: `NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V1` (Rust / `boing-sdk` `create2.ts`)
- v2 salt: `NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V2`
- v3 salt: `NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V3`

Dump bytecode:

```bash
cargo run -p boing-execution --example dump_native_dex_ledger_router
cargo run -p boing-execution --example dump_native_dex_ledger_router_v2
cargo run -p boing-execution --example dump_native_dex_ledger_router_v3
```

## SDK

`encodeNativeDexLedgerRouterForwardCalldata*`, `buildNativeDexLedgerRouterAccessList`, `buildNativeDexLedgerRouterContractCallTx` / `buildNativeDexLedgerRouterV2ContractCallTx` / `buildNativeDexLedgerRouterV3ContractCallTx` in `boing-sdk` (`nativeDexLedgerRouter.ts`).
