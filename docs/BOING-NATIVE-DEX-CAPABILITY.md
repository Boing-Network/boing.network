# Boing VM native DEX: what exists today

This document answers whether a **usable DEX-style product** can run **only** on Boing VM (no foreign execution engine on L1), and what is **not** replicated from typical EVM DEX stacks.

## What you can ship today (Boing VM only)

| Capability | Mechanism |
|------------|-----------|
| Constant-product pool | `constant_product_pool_bytecode` … **`v5`** — swap, add/remove liquidity, configurable fee (v3/v4), reference-token hooks (v2/v4), **`swap_to`** / **`remove_liquidity_to`** (v5) |
| Pair discovery / registry | `native_dex_factory_bytecode` — `register_pair`, `pairs_count`, `get_pair_at` ([NATIVE-DEX-FACTORY.md](./NATIVE-DEX-FACTORY.md)) |
| Single-hop forward through a contract | Ledger routers **v1–v3** ([NATIVE-DEX-LEDGER-ROUTER.md](./NATIVE-DEX-LEDGER-ROUTER.md)) |
| Two-hop swap in **one** tx | `native_dex_swap2_router_bytecode` ([NATIVE-DEX-SWAP2-ROUTER.md](./NATIVE-DEX-SWAP2-ROUTER.md)) |
| Multihop swap (**2–4** pools) in **one** tx | `native_dex_multihop_swap_router_bytecode` ([NATIVE-DEX-MULTIHOP-SWAP-ROUTER.md](./NATIVE-DEX-MULTIHOP-SWAP-ROUTER.md)); SDK `nativeDexSwap2Router.ts` |
| Fungible assets | [BOING-REFERENCE-TOKEN.md](./BOING-REFERENCE-TOKEN.md) templates + pool **`CALL`** |
| Optional **vault + fungible LP share** (product path) | [NATIVE-AMM-LP-VAULT.md](./NATIVE-AMM-LP-VAULT.md), [NATIVE-LP-SHARE-TOKEN.md](./NATIVE-LP-SHARE-TOKEN.md) — not required for bare pool swaps |
| Deterministic deploy addresses | CREATE2 ([TECHNICAL-SPECIFICATION.md](./TECHNICAL-SPECIFICATION.md)), salts in `native_amm` / factory / routers |
| Access lists + SDK | `boing-sdk` encoders, simulation merge, factory scan helpers, swap2 / multihop calldata builders |
| Off-chain quotes + path search | `boing-sdk` **`nativeDexRouting.ts`**: CP quotes, **`rankDirectCpPools`**, multi-hop **`findBestCpRoutes`** (≤ configurable hops), even-split aggregation heuristic, **`hydrateCpPoolVenuesFromRpc`**, **`fetchCpRoutingFromDirectoryLogs`** (logs → venues → routes) — execution still via your txs / multihop router calldata |

Together, these are enough for **swaps**, **liquidity**, **registry**, **routing**, and **multi-hop (2–4 pools)** flows **if** the app builds calldata and deploys pools with separate **`ContractDeploy`** transactions.

## Known limitations (not EVM parity)

| Limitation | Detail |
|------------|--------|
| No **`CREATE` from bytecode** | A contract **cannot** deploy another contract in the same execution. “Factory deploys pair” = **one `ContractDeploy` tx per pool**, then optional `register_pair`. ([NATIVE-DEX-FACTORY.md](./NATIVE-DEX-FACTORY.md)) |
| No Uniswap V3–style **concentrated liquidity** in canonical bytecode | Only the shipped **constant-product** programs; new curves need new VM contracts. |
| **LP** is pool-internal accounting by default | Not an ERC-20 LP token unless you add a separate token contract and wire it in product design. |
| **On-chain O(1) (tokenA, tokenB) → pool** | Pair directory uses indexed / scan patterns; no keccak mapping in baseline factory. |
| **More than four pools** in one multihop tx | Canonical multihop router caps at **4** sequential pool **`Call`s**; chain txs or a new router revision for longer paths ([NATIVE-DEX-MULTIHOP-SWAP-ROUTER.md](./NATIVE-DEX-MULTIHOP-SWAP-ROUTER.md)). |
| **TWAP / oracles** | Application and indexer concern unless you add oracle contracts ([BOING-PATTERN-ORACLE-PRICE-FEEDS.md](./BOING-PATTERN-ORACLE-PRICE-FEEDS.md)). |

## Bottom line

**Yes:** a **functional** DEX (swap, LP, list pairs, route, **2-hop** and **multihop** bundles) can exist **entirely** on Boing VM with the artifacts above plus reference tokens and off-chain/indexer support.

**No:** it is **not** a byte-for-byte Uniswap clone; factory-in-one-tx, concentrated liquidity, and permissionless in-contract pool creation are **out of scope** for pure VM bytecode until the protocol adds in-contract deploy or new precompiles.

For engineering checklists, see [BOING-L1-DEX-ENGINEERING.md](./BOING-L1-DEX-ENGINEERING.md).
