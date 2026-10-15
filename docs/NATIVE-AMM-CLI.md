# Native AMM from the CLI and SDK (`boing pool`)

> **Developers:** keep this aligned with `crates/boing-cli/src/pool.rs` and `boing-sdk/src/boingPool.ts`.

`boing pool` and the matching `boing-sdk` helpers drive the native constant-product pool templates (`constant_product_pool_bytecode{,_v2..v8}`, specified in [NATIVE-AMM-CALLDATA.md](NATIVE-AMM-CALLDATA.md)). **v8 (the default) is the first template with real custody:** it pulls deposits and swap input from the caller with reference-token **v2** `transfer_from`. Everything is Boing-native: Boing VM bytecode, bincode `SignedTransaction` signed with Ed25519, `boing_submitTransaction`, `boing_getContractStorage`. There is no EVM or Solana runtime. v1–v7 and the v1 token bytecode are unchanged; v8 and token v2 are new, versioned templates.

## Commands

All write commands take `--secret-hex` (or env `BOING_SECRET_HEX`), `--nonce`, `--dry-run`, plus the global `--rpc-url`. Use `--dry-run --nonce N` to sign fully offline (no RPC). Swap and remove then need explicit `--min-out` / `--min-a --min-b`.

| Command | What it does |
|---|---|
| `boing pool create [--version v8] [--token-a T --token-b T] [--fee-bps N] [--fee-admin A]` | Nonce-derived deploy (purpose `dapp`), then optional `set_tokens` → `set_fee_admin` → `set_swap_fee_bps` at nonce+1… (all must happen before the first deposit). Prints the pool id. v8 **requires** `--token-a/--token-b` (reference fungible **v2** tokens). |
| `boing pool add-liquidity --pool P --amount-a A --amount-b B [--min-liquidity L] [--deadline-height H] [--no-approve]` | `add_liquidity`. Quotes the LP to be minted first and refuses to submit if it is 0 or below `--min-liquidity`. v8 also enforces `min_liquidity` and the deadline on-chain, and first sends exact-amount `approve(pool, A)` on token A (nonce n) and `approve(pool, B)` on token B (n+1), then the add (n+2). `--no-approve` skips the approvals (approve yourself with `boing token approve`). v1–v7 ignore `min_liquidity` on-chain. |
| `boing pool remove-liquidity --pool P --liquidity L [--min-a X --min-b Y] [--slippage-bps 50]` | `remove_liquidity`. Without explicit mins it checks your LP balance (no overdraft) and derives mins from the pro-rata quote. |
| `boing pool swap --pool P --direction a-to-b\|b-to-a --amount-in X [--min-out M] [--slippage-bps 50] [--to R] [--deadline-height H] [--no-approve]` | `swap` (or `swap_to` with `--to`, v5..v8). v8: approves exactly `amount_in` of the input token at nonce n, swaps at n+1. Without `--min-out` it quotes live reserves and fee and sets `min_out = quote × (1 − slippage)`. The pool enforces `min_out` on-chain. |
| `boing pool quote --pool P --direction … --amount-in X` | Read-only JSON quote (`amount_out`, `fee_bps`, `min_out`). |
| `boing pool reserves --pool P [--holder H]` | Read-only JSON: reserves, total LP, effective fee, token ids, optional holder LP. |
| `boing pool bytecode [--version vN]` | Template bytecode hex. |

**`--version` (default `v8`):** it selects the quote formula and features: v1/v2 have a fixed 30 bps output-side fee, v3–v6 a stored output-side fee, v7/v8 a stored fee-on-input; v8 adds token pulls and the trailing deadline word. Pass the version the pool was deployed with. The canonical public testnet pool is **v1**. For token-hook versions (v2, v4–v8), swap and remove automatically add the configured token contracts to the access list. `--access-account` adds more.

### Example (local `boing dev`)

```bash
export BOING_SECRET_HEX=0x...   # never commit or paste this
boing token deploy --symbol TKA --initial-supply 1000000   # token v2 (default) -> 0xTA
boing token deploy --symbol TKB --initial-supply 1000000   # -> 0xTB
boing pool create --token-a 0xTA --token-b 0xTB --fee-bps 30   # v8 (default), prints pool 0x...
boing pool add-liquidity --pool 0xPOOL --amount-a 10000 --amount-b 40000   # approve A, approve B, add
boing token allowance --token 0xTA --owner 0xME --spender 0xPOOL           # 0 after the pull
boing pool quote --pool 0xPOOL --direction a-to-b --amount-in 1000
boing pool swap  --pool 0xPOOL --direction a-to-b --amount-in 1000 --slippage-bps 50
boing pool reserves --pool 0xPOOL --holder 0xME
boing pool remove-liquidity --pool 0xPOOL --liquidity 2500
```

## SDK twin (`boing-sdk`)

`createBoingPool`, `addBoingPoolLiquidity` (v8: returns the approve txs in `approvals`), `removeBoingPoolLiquidity`, `swapBoingPool`, `quoteBoingPoolSwap`, `getBoingPoolState`, `getBoingPoolLpBalance`, the unsigned builders `buildBoingPoolDeployTransaction` / `buildBoingPoolCallTransaction`, pure math (`quoteBoingPoolAmountOut`, `boingPoolMinOutWithSlippage`, `quoteBoingPoolLpMinted`, `quoteBoingPoolRemove`, `boingPoolEffectiveFeeBps`) and `boingPoolBytecodeHex` / `NATIVE_CP_POOL_BYTECODE_HEX` (v1..v8). v8 options: `tokenAHex`/`tokenBHex` (read from the pool when omitted; required offline), `noApprove`, `deadlineHeight`; encoders `encodeBoingPoolV8{Swap,SwapTo,AddLiquidity,RemoveLiquidity}Calldata`. Default version is 8, like the CLI.

**Byte-identical guarantee:** the CLI's Rust test `sdk_pool_vectors_and_bytecode_are_in_sync` generates `boing-sdk/tests/fixtures/pool-cli-vectors.json` (public test seed `0x07…07`; includes a `v8` block with token-v2 deploy, approvals, add/swap/swap_to/remove with deadlines and the allowance key), `boing-sdk/src/defaultNativeCpPoolBytecodeHex.ts` and `boing-sdk/src/defaultReferenceFungibleTemplateV2BytecodeHex.ts`, and fails if either is stale. `boing-sdk/tests/boingPool.test.ts` checks the SDK reproduces every signed transaction exactly. To regenerate: `BOING_WRITE_POOL_VECTORS=1 cargo test -p boing-cli --bin boing`.

## Tests on the Boing VM

`crates/boing-cli/src/pool.rs` runs the real templates in `boing_execution::Interpreter`:

- **v7 lifecycle:** fee set before liquidity; first-mint LP = `min(a, b)`; proportional second LP share (matches `quote_lp_minted`); swap output equals the off-chain quote in both directions; `reserve_a × reserve_b` never decreases.
- **Slippage:** a swap with `min_out = quote + 1` is rejected with no state change and no log. So is a removal with `min_a` above pro-rata.
- **No overdraft:** burning more LP than you hold, or any LP when you hold none, is rejected.
- **Token hooks:** a v7 pool with reference tokens pays swap output from the pool's token balance to the trader.
- **v8 + token v2 via the CLI builders:** add without approval is rejected with no state change; after the CLI approve flow the add pulls both tokens; a trader's swap without approval is rejected, with approval it pulls the input; pool token balances equal reserves afterwards and `k` does not decrease.

The v8 template itself is tested in `crates/boing-execution/src/native_amm_v8.rs` (custody, failed pull leaves state unchanged including pull B failing after pull A, CP invariant, slippage, LP math, on-chain `min_liquidity`, deadline, no overdraft, `remove_liquidity_to`, the fake-deposit drain that works on v7 fails on v8, a malicious token re-entering the pool cannot double-credit, fee/admin rules, protocol QA as `dapp`). Token v2 is tested in `crates/boing-execution/src/reference_token_v2.rs` (approve/overwrite, `transfer_from` decrements, insufficient allowance/balance rejected, self-transfer is a no-op, QA as `token`).

These sit alongside the existing template tests in `crates/boing-execution/src/native_amm.rs` and `tests/proptest_native_amm.rs`.

## Custody: v1–v7 are ledger-only, v8 pulls tokens

Every older template (v1–v7) is **ledger-only on the way in**: `add_liquidity` and the swap `amount_in` credit reserves from calldata and never pull tokens.

- **v1 / v3 (no token hooks):** reserves are pure bookkeeping. Fine for demos and testnet UX; represents no real assets.
- **Token-hook pools (v2, v4–v7):** swap output and LP removal **pay out real reference tokens**, but inputs are never collected, so anyone can "deposit" or "swap in" without paying and withdraw real tokens. **Never fund a v2/v4–v7 pool with valuable tokens.**

### v8 (`constant_product_pool_bytecode_v8`, CREATE2 salt `NATIVE_CP_POOL_CREATE2_SALT_V8` = `BOING_NATIVECP_C2V8`)

- v7 math (fee on input, stored fee, fee admin, `swap_to`, `remove_liquidity_to`), same storage keys and log topics.
- `add_liquidity` pulls A then B with `token.transfer_from(Caller, pool, amount)` and `swap` pulls `amount_in` of the input token **before any reserve write**. Each call must return word `1`, otherwise the pool faults and the whole transaction reverts (no partial state).
- Every rejection (slippage, deadline, overdraft, missing tokens, unknown selector) is a VM fault, i.e. a failed receipt with state reverted, not a silent `STOP`.
- On-chain `min_liquidity`; optional trailing `deadline_height` word on add/remove/swap (`0` = none; rejected when `block_height > deadline`).
- Reentrancy lock (slot `0x…09`): a token that calls back into the pool during a pull or payout faults the transaction.
- `set_tokens` is required before the first deposit. As in v2–v7 anyone may call it once before liquidity exists; the CLI sends it in the transaction right after deploy, but a front-runner could set other tokens first. Check `boing pool reserves` (token ids) before adding liquidity.
- Calldata: the v1–v7 layouts with one extra 32-byte deadline word at the end (`swap` 160 bytes, `swap_to` 192, `add_liquidity` 160, `remove_liquidity` 160, `remove_liquidity_to` 224). See [NATIVE-AMM-CALLDATA.md](NATIVE-AMM-CALLDATA.md).
- Tokens must be **reference fungible v2** ([BOING-REFERENCE-TOKEN.md](BOING-REFERENCE-TOKEN.md)); v1 tokens have no `transfer_from`, so every pull fails.

### Not done yet

- **Factory / routers:** the DEX factory registers pairs and the ledger/multihop routers forward calls with `Caller = router`. On v8 the pull would hit the router's balance and allowance, so routers need a new version that pulls from the user first (or user→router approvals). Not implemented.
- **No testnet deployment** of v8 or token v2 yet. The canonical testnet pool is still v1.
- **boing.finance** still uses the v1–v7 templates. Switching needs: deploy token v2 pairs and v8 pools (operator keys), publish canonical ids/salts in `boing-sdk` and the boing.finance config (`frontend/src/config/boingCanonicalTestnetPool.js`, `contracts.js`), add an approve step (or an `approvals` batch from `addBoingPoolLiquidity`/`swapBoingPool`) to the swap and liquidity panels, append the deadline word, use v8 in `boingNativeCreatePool.js`, ship v8-aware routers before re-enabling routed swaps, and keep the EVM/Solana paths as they are.
