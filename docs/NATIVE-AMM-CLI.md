# Native AMM from the CLI and SDK (`boing pool`)

> **Developers:** keep this aligned with `crates/boing-cli/src/pool.rs` and `boing-sdk/src/boingPool.ts`.

`boing pool` and the matching `boing-sdk` helpers drive the **existing** native constant-product pool templates (`constant_product_pool_bytecode{,_v2..v7}`, specified in [NATIVE-AMM-CALLDATA.md](NATIVE-AMM-CALLDATA.md)). Everything is Boing-native: Boing VM bytecode, bincode `SignedTransaction` signed with Ed25519, `boing_submitTransaction`, `boing_getContractStorage`. There is no EVM or Solana runtime. These helpers don't change any template bytecode.

## Commands

All write commands take `--secret-hex` (or env `BOING_SECRET_HEX`), `--nonce`, `--dry-run`, plus the global `--rpc-url`. Use `--dry-run --nonce N` to sign fully offline (no RPC). Swap and remove then need explicit `--min-out` / `--min-a --min-b`.

| Command | What it does |
|---|---|
| `boing pool create [--version v7] [--token-a T --token-b T] [--fee-bps N] [--fee-admin A]` | Nonce-derived deploy (purpose `dapp`), then optional `set_tokens` → `set_fee_admin` → `set_swap_fee_bps` at nonce+1… (all must happen before the first deposit). Prints the pool id. |
| `boing pool add-liquidity --pool P --amount-a A --amount-b B [--min-liquidity L]` | `add_liquidity`. Quotes the LP to be minted first and refuses to submit if it is 0 or below `--min-liquidity` (the templates ignore the `min_liquidity` word on-chain). |
| `boing pool remove-liquidity --pool P --liquidity L [--min-a X --min-b Y] [--slippage-bps 50]` | `remove_liquidity`. Without explicit mins it checks your LP balance (no overdraft) and derives mins from the pro-rata quote. |
| `boing pool swap --pool P --direction a-to-b\|b-to-a --amount-in X [--min-out M] [--slippage-bps 50] [--to R]` | `swap` (or `swap_to` with `--to`, v5..v7). Without `--min-out` it quotes live reserves and fee and sets `min_out = quote × (1 − slippage)`. The pool enforces `min_out` on-chain. |
| `boing pool quote --pool P --direction … --amount-in X` | Read-only JSON quote (`amount_out`, `fee_bps`, `min_out`). |
| `boing pool reserves --pool P [--holder H]` | Read-only JSON: reserves, total LP, effective fee, token ids, optional holder LP. |
| `boing pool bytecode [--version vN]` | Template bytecode hex. |

**`--version` (default `v7`):** it selects the quote formula and features: v1/v2 have a fixed 30 bps output-side fee, v3–v6 a stored output-side fee, v7 a stored fee-on-input. Pass the version the pool was deployed with. The canonical public testnet pool is **v1**. For token-hook versions (v2, v4–v7), swap and remove automatically add the configured token contracts to the access list. `--access-account` adds more.

### Example (local `boing dev`)

```bash
export BOING_SECRET_HEX=0x...   # never commit or paste this
boing pool create --fee-bps 30                     # prints pool 0x...
boing pool add-liquidity --pool 0xPOOL --amount-a 10000 --amount-b 40000
boing pool quote --pool 0xPOOL --direction a-to-b --amount-in 1000
boing pool swap  --pool 0xPOOL --direction a-to-b --amount-in 1000 --slippage-bps 50
boing pool reserves --pool 0xPOOL --holder 0xME
boing pool remove-liquidity --pool 0xPOOL --liquidity 2500
```

## SDK twin (`boing-sdk`)

`createBoingPool`, `addBoingPoolLiquidity`, `removeBoingPoolLiquidity`, `swapBoingPool`, `quoteBoingPoolSwap`, `getBoingPoolState`, `getBoingPoolLpBalance`, the unsigned builders `buildBoingPoolDeployTransaction` / `buildBoingPoolCallTransaction`, pure math (`quoteBoingPoolAmountOut`, `boingPoolMinOutWithSlippage`, `quoteBoingPoolLpMinted`, `quoteBoingPoolRemove`, `boingPoolEffectiveFeeBps`) and `boingPoolBytecodeHex` / `NATIVE_CP_POOL_BYTECODE_HEX`.

**Byte-identical guarantee:** the CLI's Rust test `sdk_pool_vectors_and_bytecode_are_in_sync` generates `boing-sdk/tests/fixtures/pool-cli-vectors.json` (public test seed `0x07…07`) and `boing-sdk/src/defaultNativeCpPoolBytecodeHex.ts`, and fails if either is stale. `boing-sdk/tests/boingPool.test.ts` checks the SDK reproduces every signed transaction exactly. To regenerate: `BOING_WRITE_POOL_VECTORS=1 cargo test -p boing-cli --bin boing`.

## Tests on the Boing VM

`crates/boing-cli/src/pool.rs` runs the real templates in `boing_execution::Interpreter`:

- **v7 lifecycle:** fee set before liquidity; first-mint LP = `min(a, b)`; proportional second LP share (matches `quote_lp_minted`); swap output equals the off-chain quote in both directions; `reserve_a × reserve_b` never decreases.
- **Slippage:** a swap with `min_out = quote + 1` is rejected with no state change and no log. So is a removal with `min_a` above pro-rata.
- **No overdraft:** burning more LP than you hold, or any LP when you hold none, is rejected.
- **Token hooks:** a v7 pool with reference tokens pays swap output from the pool's token balance to the trader.

These sit alongside the existing template tests in `crates/boing-execution/src/native_amm.rs` and `tests/proptest_native_amm.rs`.

## Custody gap (important) and proposed v8 template

Every shipped template (v1–v7) is **ledger-only on the way in**: `add_liquidity` and the swap `amount_in` credit reserves from calldata and never pull tokens from the caller. The reference fungible token has only `transfer` (no `approve`/`transfer_from`), so a pool cannot pull funds. Consequences:

- **v1 / v3 (no token hooks):** reserves are pure bookkeeping. This is fine for demos and testnet UX but represents no real assets.
- **Token-hook pools (v2, v4–v7):** swap output and LP removal **pay out real reference tokens** from the pool's balance, but inputs are never collected. Anyone can "deposit" or "swap in" without paying, then withdraw real tokens. **Never fund a token-hook pool with valuable tokens.** The CLI prints this caveat on `create`.

This needs a new, versioned design. **Existing bytecode stays untouched and nothing below is implemented yet:**

1. **Reference fungible v2 template** (new bytecode, new purpose version): add `approve(spender, amount)` (`0x08`) and `transfer_from(from, to, amount)` (`0x09`) with an allowance slot `keccak/xor(owner, spender)`. It must refuse overdraft and allowance underflow.
2. **Pool v8** = v7 math plus, on `add_liquidity`, `CALL token_a.transfer_from(Caller, pool, amount_a)` (and the same for B), and on `swap`, `CALL token_in.transfer_from(Caller, pool, amount_in)`. Each must succeed (return word `1`) or the pool aborts before any reserve write. It should also enforce `min_liquidity` on-chain and accept an optional `deadline_height` word.
3. **Invariant tests to require:** no reserve change without a successful pull; reserves ≤ pool token balances after every op; CP invariant; slippage; LP share; no overdraft; reentrancy (token `CALL` back into the pool must not double-credit).
4. Add a new CREATE2 salt `NATIVE_CP_POOL_CREATE2_SALT_V8`, plus factory and router support and a boing.finance switch. Shipping it needs operator deployment and is out of scope for this pass.

Until v8 exists, `boing pool` is for ledger-only (v1/v3/v7-without-tokens) pools on dev or test networks.