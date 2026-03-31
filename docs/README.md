# Boing Network — Documentation Index

Start with [BOING-NETWORK-ESSENTIALS.md](BOING-NETWORK-ESSENTIALS.md) for the six pillars and design philosophy.

## Core

| Doc | Description |
|-----|-------------|
| [TECHNICAL-SPECIFICATION.md](TECHNICAL-SPECIFICATION.md) | Crypto, data formats, bytecode, gas, RPC, QA rules |
| [BOING-BLOCKCHAIN-DESIGN-PLAN.md](BOING-BLOCKCHAIN-DESIGN-PLAN.md) | Architecture, tokenomics, design decisions |
| [RUNBOOK.md](RUNBOOK.md) | Node setup, RPC, CLI, monitoring, incidents |
| [RPC-API-SPEC.md](RPC-API-SPEC.md) | JSON-RPC API reference |

## Readiness & Launch

| Doc | Description |
|-----|-------------|
| [READINESS.md](READINESS.md) | Beta checklist, six-pillar readiness, launch-blocking path |
| [TESTNET.md](TESTNET.md) | Join testnet (bootnodes, faucet, single vs multi-node); **Testnet Portal** (registration, dashboards, community quests); **Incentivized testnet** (readiness, promotion, mainnet migration, Reddit draft in Appendix A) |
| [INFRASTRUCTURE-SETUP.md](INFRASTRUCTURE-SETUP.md) | Bootnodes, Cloudflare tunnel |
| [VIBEMINER-INTEGRATION.md](VIBEMINER-INTEGRATION.md) | One-click node/validator via VibeMiner; **appendix:** network listing form values and listing requirements |

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
| [BOING-VM-CAPABILITY-PARITY-ROADMAP.md](BOING-VM-CAPABILITY-PARITY-ROADMAP.md) | **Full-stack parity plan:** EVM/Solana-class workflows via native Boing VM, SDK, wallet, indexer (pairs with [EXECUTION-PARITY-TASK-LIST.md](EXECUTION-PARITY-TASK-LIST.md)) |
| [BOING-PATTERN-AMM-LIQUIDITY.md](BOING-PATTERN-AMM-LIQUIDITY.md) | Constant-product AMM pattern (VM contracts, access lists, QA) |
| [BOING-PATTERN-ORACLE-PRICE-FEEDS.md](BOING-PATTERN-ORACLE-PRICE-FEEDS.md) | Oracle / price feeds (app layer, TWAP, multisig) |
| [BOING-PATTERN-UPGRADE-PROXY.md](BOING-PATTERN-UPGRADE-PROXY.md) | Upgradeable / hub-pointer patterns vs QA |
| [INDEXER-RECEIPT-AND-LOG-INGESTION.md](INDEXER-RECEIPT-AND-LOG-INGESTION.md) | Receipt + `LOG*` ingestion for indexers / explorers (I1–I3; replay vs `boing_getLogs`) |
| [E2-PARTNER-APP-NATIVE-BOING.md](E2-PARTNER-APP-NATIVE-BOING.md) | Partner apps: native Boing path without ethers (E2) |
| [EXECUTION-PARITY-TASK-LIST.md](EXECUTION-PARITY-TASK-LIST.md) | VM / receipts / RPC code tasks (opcodes, QA, persistence) |
| [DEVELOPMENT-AND-ENHANCEMENTS.md](DEVELOPMENT-AND-ENHANCEMENTS.md) | SDK, automation, enhancements; **appendix:** cryptographic verification for decentralized automation |
| [NETWORK-COST-ESTIMATE.md](NETWORK-COST-ESTIMATE.md) | Cost overview |

## Other

| Doc | Description |
|-----|-------------|
| [BOING-EXPRESS-WALLET.md](BOING-EXPRESS-WALLET.md) | Boing Express: bootstrap, integration, Chrome Web Store; **Part 3:** portal connection, sign-in API, rollout & smoke test |
| [BOING-DAPP-INTEGRATION.md](BOING-DAPP-INTEGRATION.md) | dApp checklist: native Boing provider, simulate, submit, auth (pairs with `boing-sdk`) |
| [BOING-SIGNED-TRANSACTION-ENCODING.md](BOING-SIGNED-TRANSACTION-ENCODING.md) | bincode layout + signable hash; JS/Rust parity reference |
| [BOING-RPC-ERROR-CODES-FOR-DAPPS.md](BOING-RPC-ERROR-CODES-FOR-DAPPS.md) | JSON-RPC / QA / pool codes + `explainBoingRpcError`; Express alignment contract |
| [BOING-OBSERVER-AND-EXPRESS.md](BOING-OBSERVER-AND-EXPRESS.md) | **Part 1:** what’s in repo vs what to build for boing.observer and boing.express; **Part 2:** full boing.observer explorer spec and one-shot prompt |
| [THREE-CODEBASE-ALIGNMENT.md](THREE-CODEBASE-ALIGNMENT.md) | **Sync checklist** for boing.network, boing.express, boing.observer (URLs, RPC, chain IDs, cross-links) |
| [DECENTRALIZATION-AND-NETWORKING.md](DECENTRALIZATION-AND-NETWORKING.md) | P2P, discovery, WebRTC signaling |
| [WEBSITE-AND-DEPLOYMENT.md](WEBSITE-AND-DEPLOYMENT.md) | Website and Cloudflare deployment |
| [NOTION-INTEGRATION-SETUP.md](NOTION-INTEGRATION-SETUP.md) | Notion integration |
| [INDEXER-OPERATOR-STATS.md](INDEXER-OPERATOR-STATS.md) | Operator stats indexer & leaderboard (portal) |
| [ACCELERATOR-APPLICATIONS.md](ACCELERATOR-APPLICATIONS.md) | Draft answers for Beacon & Outlier Ventures accelerator applications |
| [Executive-Summary-Pitch-Deck.md](Executive-Summary-Pitch-Deck.md) | Executive summary & pitch deck (source for PDF) |
