# Boing-native product capability matrix (EVM | Solana | Boing L1)

> Everyday users: use [boing.finance](https://boing.finance) with the right wallet (EVM / Solana / Boing Express).
> Developers: this matrix tracks **product** parity of capabilities on **Boing-native** (VM + Express + Observer + finance Boing path), not Solidity-on-Boing.
> Operators: rows marked **needs operator deploy** require publishing non-zero AccountIds via oing_getNetworkInfo.end_user / env.

Cross-links: [HANDOFF-DEPENDENT-PROJECTS.md](HANDOFF-DEPENDENT-PROJECTS.md), [BOING-NATIVE-DEX-CAPABILITY.md](BOING-NATIVE-DEX-CAPABILITY.md).

*Last updated: 2026-09-25.*

## Legend

| Status | Meaning |
|--------|---------|
| **done** | Shipped in-repo; works when RPC / wallet / contracts are live |
| **app-wired** | UI + SDK / Express path complete; may still show honest gates when module ids are zero |
| **needs operator deploy** | Code ready; published pool / factory / router / locker ids (or EVM TokenFactory) required |
| **external** | Intentionally multi-chain UX via aggregator / third party (not Boing VM) |
| **planned** | Product gap or protocol design still open |

## Capability matrix

| Capability | EVM (finance) | Solana (finance) | Boing-native (L1 6913) |
|------------|---------------|------------------|-------------------------|
| **Deploy fungible token** | TokenFactory live on listed chains; honest gate when zero | SPL + Metaplex | **app-wired** — reference / secured bytecode + Express contract_deploy_*; QA on node |
| **Deploy NFT collection** | EVM NFT flows / external | Metaplex metadata path | **app-wired** — reference NFT collection deploy via Express |
| **Swap** | Boing DEX when factory+router live; else LI.FI (+ Uniswap/Pancake V2 quote fallback) | Jupiter | **app-wired** — native CP pool when pool id published; trade hub always shown on 6913 with honest gate when pool is zero |
| **Create pool** | DEXFactory or Uniswap/Pancake V2 ddLiquidity | Raydium external links | **app-wired** — deploy CP pool + seed; 
egister_pair when factory id published |
| **Add / remove liquidity** | Boing router or Uniswap/Pancake V2 | External | **done** — Liquidity page + trade hub; paste pool AccountId OK |
| **Multi-pair / smart route** | Boing router when live | Jupiter | **app-wired** — factory + multihop router; **needs operator deploy** for live ids + registered pairs |
| **Liquidity locker** | LiquidityLocker when live | — | **needs operator deploy** — module id via env / end_user |
| **Pools / directory / markets** | GeckoTerminal + EVM pools page | GeckoTerminal | **app-wired** — Observer `/dex/pools` + finance Pools tab; Worker `/v1/directory/meta` returns honest `emptyDirectory` / `emptyDirectoryNote` until ops sync |

| **Portfolio** | EVM balances / The Graph / Alchemy | SOL + SPL | **app-wired** (2026-09) — oing_getAccount BOING + stake; LP via Swap → Your liquidity |
| **Bridge** | LI.FI / external | Not in initial scope | **app-wired** — honest `/bridge` scaffold + banner (not LI.FI execution, not a Boing VM bridge); live transfers still need protocol + ops |

| **Analytics / charts** | Token charts + markets board | Markets board | **partial** — reserve history + optional indexer stats |
| **Explorer transparency** | Chain explorers | Solscan-class | **app-wired** — Observer account/tx (32-byte), DEX directory, ClaimUnbond + QaPoolVote |
| **Wallet signing** | MetaMask / EVM injected | Phantom / Solflare | **done** — Boing Express only (`contract_call` + `access_list` + simulate; one-shot suggested access-list merge on send; approval UI documents auto-merge) |


## What still needs operators (not app code)

1. Publish non-zero **canonical CP pool** (and ideally factory / multihop router / locker) on hosted testnet via BOING_CANONICAL_NATIVE_* → end_user hints.
2. Bootstrap **register_pair** / seed liquidity so smart route and directory are non-empty.
3. Optional: run native-dex-indexer cron / `POST /v1/directory/sync` after publishing ids (Worker empty-path honesty is already code-complete).
4. **Boing VM bridge protocol** + live transfer execution (app scaffold only today).
5. EVM mainnet DEXFactory deploys remain a separate finance contracts/ operator track (not Boing VM).

## Dependent-repo checklist (this pass — 2026-09-25 continued)

| Repo | Focus |
|------|--------|
| **boing.finance** | Always-on honest `/bridge` scaffold banner; wire `getBoingL1FullDexReadiness` into trade hub + Native VM; Portfolio + always-on L1 trade hub (prior) |
| **boing.express** | Approval pipeline copy for one-shot `access_list` auto-merge; simulate/send path (prior) |
| **boing.observer** | ClaimUnbond narrative clarified as Boing-native (not EVM); ClaimUnbond + QaPoolVote display (prior) |
| **boing.network** | This matrix; Worker `/v1/directory/meta` empty-directory honesty + README (no EVM/Solana runtime deps) |
