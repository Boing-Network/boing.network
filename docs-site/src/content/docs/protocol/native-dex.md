---
title: Native DEX
description: Constant-product pools, factory directory, and L1 discovery RPCs on chain 6913.
---

Boing ships a **native** constant-product AMM and DEX auxiliaries (factory, routers, LP vault / share token) as first-class L1 contracts — not an EVM Uniswap port.

## Discovery

Prefer live network info over hardcoded addresses:

1. Call **`boing_getNetworkInfo`**.
2. Read `developer.dex_discovery_methods` — on public testnet: `boing_listDexPools`, `boing_listDexTokens`, `boing_getDexToken`.
3. Read `end_user.canonical_native_*` hints when the operator has published them (cp pool, factory, multihop router, ledger routers, LP vault, LP share token).

`@boing-network/boing-sdk` merges those hints with app overrides via helpers such as `mergeNativeDexIntegrationDefaults`. Historical embedded 6913 constants apply only when network info is omitted (offline).

## Product UX split

- **Finance Swap** defaults to the network’s canonical constant-product pool.
- Additional factory pairs and multihop flows are oriented around the **Pools** directory / smart-route paths in Finance.

## Specialist specs

Wire formats, CREATE2 salts, and operator bootstrap scripts stay in monorepo docs (`NATIVE-AMM-*`, `NATIVE-DEX-*`, `HANDOFF_*` discovery notes) and tutorial scripts under `examples/native-boing-tutorial/`. The node does **not** auto-create pools or seed reserves on first start.
