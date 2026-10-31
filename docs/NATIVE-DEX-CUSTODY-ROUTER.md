# Custody router v1 + pool v9

**Status:** templates, CLI, SDK and VM tests only. Nothing is deployed to testnet.

## Why
Pool v8 pulls swap input from `CALLER` through `transfer_from`. The older routers
(`native_dex_ledger_router`, `native_dex_multihop_swap_router`) forward calls with `Caller = router`,
so on v8 the pool would pull from the router's own balance. Also, v8 `swap` returns nothing, so a
router can't learn the intermediate amounts it needs for multihop.

## Pool v9
v9 is the v8 generator plus one change: `swap` / `swap_to` **return `amount_out`** as one 32-byte word.
v8 bytecode is byte-identical to before; the SDK hex table pins this. The CREATE2 salt is
`BOING_NATIVECP_C2V9`. The CLI and SDK accept `--version v9` / `version: 9`; the default stays `v8`.

## Router (`native_dex_custody_router`, selector `0xF1`, salt `BOING_NATIVEDEX_CROUTER_V1`)
Calldata: `sel | hops | amount_in | min_out | recipient | deadline | (pool, direction, token_in) x hops`,
with 1..=4 hops. The router:
1. runs `transfer_from(token_in[0], CALLER -> router, amount_in)`. The user first approves the router.
2. for each hop, approves `pool_i` for exactly `amt`, then calls `swap_to`. Intermediate hops pay the
   router with `min = 1` (**by design** — user slippage is enforced only on the final hop; see
   [AUDIT-61-PROTOCOL-SEMANTICS.md](AUDIT-61-PROTOCOL-SEMANTICS.md) §3); their returned `amount_out`
   becomes the next `amt`, and a return of 0 faults. This means **intermediate pools must be v9**.
   The last hop pays `recipient` with `min_out` and can be v8 or v9.
3. returns the last hop's return word: `amount_out` from a v9 pool, `0` from a v8 pool.

Any failure faults, so the whole transaction reverts, including the pull and earlier hops. Because
approvals are exact, the router never keeps balances or allowances. A reentrancy lock uses slot `0x0b`.
The router only pulls from `CALLER`, so nobody else can spend an approval given to it.

## CLI
```
boing pool router-deploy [--dry-run --nonce N]
boing pool router-bytecode
boing pool route --router R --hop POOL1:a-to-b --hop POOL2:a-to-b --amount-in 1000 \
  [--min-out M | --slippage-bps 50] [--to X] [--deadline-height H] [--no-approve]
```
Offline dry-runs need `--hop POOL:DIR:TOKEN_A:TOKEN_B` and `--min-out`.

## SDK
`routeBoingSwap`, `buildBoingCustodyRouterSwapTransaction`, `buildBoingCustodyRouterDeployTransaction`,
`encodeBoingCustodyRouterSwapCalldata`, `quoteBoingRoute`, `checkBoingRoutePath` and `boingRouteHop`.
They are byte-identical to the CLI, checked by `tests/boingRouter.test.ts` and `pool-cli-vectors.json`.

## Factory
`native_dex_factory` is a version-agnostic pair **registry**; the VM has no in-contract deploy.
Pools of any version can be registered as they are, so no new template is needed. Registration is
permissionless, so UIs must verify a registered pool's `token_a/token_b` storage before routing.