# Boing Network — Documentation Index

Start with [BOING-NETWORK-ESSENTIALS.md](BOING-NETWORK-ESSENTIALS.md) for the six pillars and design philosophy. This file is the **canonical map** of `docs/`; the repo root [README.md](../README.md) duplicates a short subset for quick navigation. Contributors: see root [CONTRIBUTING.md](../CONTRIBUTING.md).

## Core

| Doc | Description |
|-----|-------------|
| [TECHNICAL-SPECIFICATION.md](TECHNICAL-SPECIFICATION.md) | Crypto, data formats, bytecode, gas, RPC, QA rules |
| [BOING-VM-INDEPENDENCE.md](BOING-VM-INDEPENDENCE.md) | Boing VM only — no foreign chain bytecode engines in protocol |
| [BOING-BLOCKCHAIN-DESIGN-PLAN.md](BOING-BLOCKCHAIN-DESIGN-PLAN.md) | Architecture, tokenomics, design decisions |
| [RUNBOOK.md](RUNBOOK.md) | Node setup, RPC, CLI, monitoring, incidents |
| [RPC-API-SPEC.md](RPC-API-SPEC.md) | JSON-RPC API reference — **Method index** lists every `boing_*` RPC on current `boing-node`; § Native constant-product AMM = canonical testnet pool id |

## Readiness & Launch

| Doc | Description |
|-----|-------------|
| [READINESS.md](READINESS.md) | Beta checklist, six-pillar readiness, launch-blocking path |
| [NETWORK-GO-LIVE-CHECKLIST.md](NETWORK-GO-LIVE-CHECKLIST.md) | Operator order: genesis → bootnodes → public RPC → **`preflight-rpc`** / **`check-testnet-rpc`** → faucet → optional native pool |
| [DEVNET-OPERATOR-NATIVE-AMM.md](DEVNET-OPERATOR-NATIVE-AMM.md) | Self-hosted RPC (e.g. VibeMiner): CORS + chain id, deploy native CP pool, seed liquidity, point Express + boing.finance at your pool |
| [PRE-VIBEMINER-NODE-COMMANDS.md](PRE-VIBEMINER-NODE-COMMANDS.md) | Copy/paste: **`cargo`**, **`boing-node`** flags (**`BOING_RATE_PROFILE`**), tutorial **`npm run`** scripts, SDK **`verify`** |
| [TESTNET-RPC-INFRA.md](TESTNET-RPC-INFRA.md) | **One map:** testnet ops + public RPC + infra (routing tables, env matrix, post-deploy checks) |
| [TESTNET-OPS-RUNBOOK.md](TESTNET-OPS-RUNBOOK.md) | Operator umbrella: go-live order, env matrix, monitoring, Playwright / **OPS-1** pointers |
| [PLAYWRIGHT-E2E-CI-OPS.md](PLAYWRIGHT-E2E-CI-OPS.md) | Why extension E2E skips on default CI; self-hosted / secrets patterns |
| [PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md](PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md) | Before/after steps when upgrading the node behind public JSON-RPC |
| [NEXT-STEPS-FUTURE-WORK.md](NEXT-STEPS-FUTURE-WORK.md) | Consolidated backlog: infra / CI map, where to file enhancements & optimizations, indexer scale, native AMM follow-ups, ops-dependent items, optional small PR slices |
| [TESTNET.md](TESTNET.md) | Join testnet (bootnodes, faucet, single vs multi-node); **§9** ship new **`boing-node`** zips (release tags, D1, `networks.js`); **Testnet Portal** (registration, dashboards, community quests); **Incentivized testnet** (readiness, promotion, mainnet migration, Reddit draft in Appendix A) |
| [INFRASTRUCTURE-SETUP.md](INFRASTRUCTURE-SETUP.md) | Bootnodes, Cloudflare tunnel, **HTTP 405** (tunnel vs Pages-only DNS), VibeMiner tunnel alignment |
| [VIBEMINER-INTEGRATION.md](VIBEMINER-INTEGRATION.md) | One-click node/validator via VibeMiner; **`GET /api/networks`** **`meta`** for desktop sync; **§6** maintainer checklist; appendix: listing form values |

## Quality & Security

| Doc | Description |
|-----|-------------|
| [QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md) | Protocol QA: rules, automation, community pool; **Appendix A:** deployer checklist; **Appendix B:** canonical malice definition; **Appendix C:** governance-mutable QA rules (content blocklist, registry JSON) |
| [SECURITY-STANDARDS.md](SECURITY-STANDARDS.md) | Protocol, network, application security |

## Design & Build

| Doc | Description |
|-----|-------------|
| [BOING-DESIGN-SYSTEM.md](BOING-DESIGN-SYSTEM.md) | Site variants, tokens, accessibility |
| [BUILD-ROADMAP.md](BUILD-ROADMAP.md) | Implementation phases |
| [BOING-VM-CAPABILITY-PARITY-ROADMAP.md](BOING-VM-CAPABILITY-PARITY-ROADMAP.md) | **Full-stack capability plan:** native Boing VM, SDK, wallet, indexer (pairs with [EXECUTION-PARITY-TASK-LIST.md](EXECUTION-PARITY-TASK-LIST.md); comparisons in-doc are non-normative) |
| [BOING-L1-DEX-ENGINEERING.md](BOING-L1-DEX-ENGINEERING.md) | **DEX on L1 (non-EVM):** Boing VM bytecode for factory/router/locker vs Solidity reference; QA/deploy checklist; pairs with **boing.finance** `docs/boing-l1-*.md` |
| [BOING-NATIVE-DEX-CAPABILITY.md](BOING-NATIVE-DEX-CAPABILITY.md) | **What ships today:** CP pools + pair directory + single-/multi-hop routers + SDK; limitations vs EVM DEX stacks |
| [NATIVE-DEX-FACTORY.md](NATIVE-DEX-FACTORY.md) | **Pair directory** VM program: `register_pair` / `pairs_count` / `get_pair_at` + `Log3` (pools deployed separately; no in-VM `CREATE`) |
| [NATIVE-DEX-LEDGER-ROUTER.md](NATIVE-DEX-LEDGER-ROUTER.md) | **Ledger router:** `Call` forwarder (**v1** 128-byte; **v2** 160-byte; **v3** 192-byte); **v1** ledger-only pools |
| [NATIVE-DEX-SWAP2-ROUTER.md](NATIVE-DEX-SWAP2-ROUTER.md) | **Two-hop** swap router (one tx, two pools) |
| [NATIVE-DEX-MULTIHOP-SWAP-ROUTER.md](NATIVE-DEX-MULTIHOP-SWAP-ROUTER.md) | **Multihop** swap router (**2–4** pools per tx) |
| [NATIVE-AMM-CALLDATA.md](NATIVE-AMM-CALLDATA.md) | Native **constant-product pool** selectors, storage keys, **`Log2`**, access lists, CREATE2 salts |
| [NATIVE-AMM-INTEGRATION-CHECKLIST.md](NATIVE-AMM-INTEGRATION-CHECKLIST.md) | End-to-end checklist: VM → SDK → wallet → boing.finance (+ optional LP vault / share token) |
| [NATIVE-AMM-LP-VAULT.md](NATIVE-AMM-LP-VAULT.md) | Optional **LP vault** (`configure` / `deposit_add`) |
| [NATIVE-LP-SHARE-TOKEN.md](NATIVE-LP-SHARE-TOKEN.md) | Optional **LP share** fungible (`mint` / `transfer` / `set_minter_once`) |
| [NATIVE-AMM-E2E-SMOKE.md](NATIVE-AMM-E2E-SMOKE.md) | Manual Boing Express + dApp smoke (swap / liquidity) |
| [OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md](OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md) | Published **canonical testnet** native CP pool id (**OPS-1**) |
| [BOING-PATTERN-AMM-LIQUIDITY.md](BOING-PATTERN-AMM-LIQUIDITY.md) | Constant-product AMM pattern (VM contracts, access lists, QA) |
| [BOING-PATTERN-ORACLE-PRICE-FEEDS.md](BOING-PATTERN-ORACLE-PRICE-FEEDS.md) | Oracle / price feeds (app layer, TWAP, multisig) |
| [BOING-PATTERN-UPGRADE-PROXY.md](BOING-PATTERN-UPGRADE-PROXY.md) | Upgradeable / hub-pointer patterns vs QA |
| [INDEXER-RECEIPT-AND-LOG-INGESTION.md](INDEXER-RECEIPT-AND-LOG-INGESTION.md) | Receipt + `LOG*` ingestion for indexers / explorers (I1–I3; replay vs `boing_getLogs`) |
| [OBSERVER-HOSTED-SERVICE.md](OBSERVER-HOSTED-SERVICE.md) | **OBS-1:** hosted observer / durable indexer — architecture, schema, reorgs, read API, deployment |
| [E2-PARTNER-APP-NATIVE-BOING.md](E2-PARTNER-APP-NATIVE-BOING.md) | Partner apps: native Boing path without foreign chain client SDKs (E2) |
| [BOING-CANONICAL-DEPLOY-ARTIFACTS.md](BOING-CANONICAL-DEPLOY-ARTIFACTS.md) | Pinned native fungible + NFT bytecode versioning; form-parity with EVM; **`boing-sdk`** + env matrix |
| [EXECUTION-PARITY-TASK-LIST.md](EXECUTION-PARITY-TASK-LIST.md) | VM / receipts / RPC code tasks (opcodes, QA, persistence) |
| [DEVELOPMENT-AND-ENHANCEMENTS.md](DEVELOPMENT-AND-ENHANCEMENTS.md) | SDK, automation, enhancements; **appendix:** cryptographic verification for decentralized automation |
| [NETWORK-COST-ESTIMATE.md](NETWORK-COST-ESTIMATE.md) | Cost overview |

## Other

| Doc | Description |
|-----|-------------|
| [../CONTRIBUTING.md](../CONTRIBUTING.md) | **Contributor guide:** build, SDK tests, docs map, PR expectations |
| [BOING-EXPRESS-WALLET.md](BOING-EXPRESS-WALLET.md) | Boing Express: bootstrap, integration, Chrome Web Store; **Part 3:** portal connection, sign-in API, rollout & smoke test |
| [BOING-DAPP-INTEGRATION.md](BOING-DAPP-INTEGRATION.md) | dApp checklist: native Boing provider, simulate, submit, auth (pairs with `boing-sdk`) |
| [BOING-SIGNED-TRANSACTION-ENCODING.md](BOING-SIGNED-TRANSACTION-ENCODING.md) | bincode layout + signable hash; JS/Rust parity reference |
| [BOING-RPC-ERROR-CODES-FOR-DAPPS.md](BOING-RPC-ERROR-CODES-FOR-DAPPS.md) | JSON-RPC / QA / pool codes + `explainBoingRpcError`; Express alignment contract |
| [BOING-OBSERVER-AND-EXPRESS.md](BOING-OBSERVER-AND-EXPRESS.md) | **Part 1:** what’s in repo vs what to build for boing.observer and boing.express; **Part 2:** full boing.observer explorer spec and one-shot prompt |
| [THREE-CODEBASE-ALIGNMENT.md](THREE-CODEBASE-ALIGNMENT.md) | **Sync checklist** for boing.network, boing.express, boing.observer (URLs, RPC, chain IDs, cross-links) |
| [HANDOFF-DEPENDENT-PROJECTS.md](HANDOFF-DEPENDENT-PROJECTS.md) | **Cross-repo handoff:** what ships in this monorepo vs recommended work for **boing.express**, **boing.observer**, partner dApps |
| [DECENTRALIZATION-AND-NETWORKING.md](DECENTRALIZATION-AND-NETWORKING.md) | P2P, discovery, WebRTC signaling |
| [WEBSITE-AND-DEPLOYMENT.md](WEBSITE-AND-DEPLOYMENT.md) | Website and Cloudflare deployment |
| [NOTION-INTEGRATION-SETUP.md](NOTION-INTEGRATION-SETUP.md) | Notion integration |
| [INDEXER-OPERATOR-STATS.md](INDEXER-OPERATOR-STATS.md) | Operator stats indexer & leaderboard (portal) |
| [ACCELERATOR-APPLICATIONS.md](ACCELERATOR-APPLICATIONS.md) | Draft answers for Beacon & Outlier Ventures accelerator applications |
| [Executive-Summary-Pitch-Deck.md](Executive-Summary-Pitch-Deck.md) | Executive summary & pitch deck (source for PDF) |
