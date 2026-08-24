# Boing Network — Documentation Index

Start with [BOING-NETWORK-ESSENTIALS.md](BOING-NETWORK-ESSENTIALS.md) for the six pillars and design philosophy. This file is the **canonical map** of `docs/`; the repo root [README.md](../README.md) duplicates a short subset for quick navigation. Contributors: [CONTRIBUTING.md](../CONTRIBUTING.md).

## Core

| Doc | Description |
|-----|-------------|
| [TECHNICAL-SPECIFICATION.md](TECHNICAL-SPECIFICATION.md) | Crypto, data formats, bytecode, gas, RPC, QA rules |
| [BOING-VM-INDEPENDENCE.md](BOING-VM-INDEPENDENCE.md) | Boing VM only — no foreign chain bytecode engines in protocol |
| [BOING-BLOCKCHAIN-DESIGN-PLAN.md](BOING-BLOCKCHAIN-DESIGN-PLAN.md) | Architecture, tokenomics, design decisions |
| [RUNBOOK.md](RUNBOOK.md) | Node setup, RPC, CLI, monitoring, incidents |
| [RPC-API-SPEC.md](RPC-API-SPEC.md) | JSON-RPC API reference; § Native AMM = canonical pool; § DEX discovery |

## Readiness, testnet & ops

| Doc | Description |
|-----|-------------|
| [READINESS.md](READINESS.md) | Beta checklist, six pillars, launch-blocking path |
| [TESTNET-RPC-INFRA.md](TESTNET-RPC-INFRA.md) | **Operator hub:** go-live order, env matrix, verification, monitoring, QA apply |
| [TESTNET.md](TESTNET.md) | Join testnet, portal, incentivized program; **§9.1** node zip release checklist |
| [PRE-VIBEMINER-NODE-COMMANDS.md](PRE-VIBEMINER-NODE-COMMANDS.md) | Copy/paste RPC smoke and tutorial `npm run` commands |
| [INFRASTRUCTURE-SETUP.md](INFRASTRUCTURE-SETUP.md) | Bootnodes, Cloudflare tunnel, VibeMiner alignment |
| [FLY-IO.md](FLY-IO.md) | Hosted two-node testnet on Fly.io (validator + peering full node) |
| [PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md](PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md) | Upgrade node behind public JSON-RPC |
| [DEVNET-OPERATOR-NATIVE-AMM.md](DEVNET-OPERATOR-NATIVE-AMM.md) | Self-hosted RPC + deploy native DEX stack |
| [VIBEMINER-INTEGRATION.md](VIBEMINER-INTEGRATION.md) | One-click node; **`/api/networks`**; Appendix A two-node Windows |
| [PLAYWRIGHT-E2E-CI-OPS.md](PLAYWRIGHT-E2E-CI-OPS.md) | Extension E2E CI limits |
| [WEBSITE-AND-DEPLOYMENT.md](WEBSITE-AND-DEPLOYMENT.md) | boing.network site deploy |

## Roadmaps & backlog

| Doc | Description |
|-----|-------------|
| [BUILD-ROADMAP.md](BUILD-ROADMAP.md) | Implementation phases (historical + remaining) |
| [NEXT-STEPS-FUTURE-WORK.md](NEXT-STEPS-FUTURE-WORK.md) | Backlog router — infra, native AMM, indexer, ops |
| [BOING-VM-CAPABILITY-PARITY-ROADMAP.md](BOING-VM-CAPABILITY-PARITY-ROADMAP.md) | Full-stack capability matrix |
| [EXECUTION-PARITY-TASK-LIST.md](EXECUTION-PARITY-TASK-LIST.md) | VM / receipts / RPC code tasks |
| [DEVELOPMENT-AND-ENHANCEMENTS.md](DEVELOPMENT-AND-ENHANCEMENTS.md) | Strategic vision (non-normative TBD sections) |
| [PROTOCOL_NATIVE_DEX_RPC_AND_INDEXING_ROADMAP.md](PROTOCOL_NATIVE_DEX_RPC_AND_INDEXING_ROADMAP.md) | Protocol drafts: simulate, LP positions, indexer |

## Quality & security

| Doc | Description |
|-----|-------------|
| [QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md) | Protocol QA, community pool, content blocklist governance |
| [config/CANONICAL-QA-REGISTRY.md](config/CANONICAL-QA-REGISTRY.md) | QA JSON configs + `npm run apply-public-testnet-qa-policy` |
| [SECURITY-STANDARDS.md](SECURITY-STANDARDS.md) | Protocol, network, application security |

## Native DEX & AMM

| Doc | Description |
|-----|-------------|
| [BOING-NATIVE-DEX-CAPABILITY.md](BOING-NATIVE-DEX-CAPABILITY.md) | What ships today vs EVM parity gaps |
| [NATIVE-AMM-INTEGRATION-CHECKLIST.md](NATIVE-AMM-INTEGRATION-CHECKLIST.md) | End-to-end integration + manual E2E smoke |
| [NATIVE-AMM-CALLDATA.md](NATIVE-AMM-CALLDATA.md) | Pool selectors, storage, Log2, CREATE2 salts |
| [NATIVE-DEX-FACTORY.md](NATIVE-DEX-FACTORY.md) | Pair directory VM |
| [NATIVE-DEX-LEDGER-ROUTER.md](NATIVE-DEX-LEDGER-ROUTER.md) | Ledger forwarders v1–v3 |
| [NATIVE-DEX-SWAP2-ROUTER.md](NATIVE-DEX-SWAP2-ROUTER.md) | Two-hop router |
| [NATIVE-DEX-MULTIHOP-SWAP-ROUTER.md](NATIVE-DEX-MULTIHOP-SWAP-ROUTER.md) | Multihop router (2–6 hops) |
| [NATIVE-AMM-LP-VAULT.md](NATIVE-AMM-LP-VAULT.md) | LP vault |
| [NATIVE-LP-SHARE-TOKEN.md](NATIVE-LP-SHARE-TOKEN.md) | LP share token |
| [OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md](OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md) | Published canonical pool (**OPS-1**) |
| [OPS-CANONICAL-TESTNET-NATIVE-DEX-AUX.md](OPS-CANONICAL-TESTNET-NATIVE-DEX-AUX.md) | Predicted CREATE2 aux addresses |
| [OPS-FRESH-TESTNET-BOOTSTRAP.md](OPS-FRESH-TESTNET-BOOTSTRAP.md) | Fresh chain + new operator key |
| [NATIVE-DEX-OPERATOR-DEPLOYMENT-RECORD.md](NATIVE-DEX-OPERATOR-DEPLOYMENT-RECORD.md) | Operator deployment snapshots |
| [NATIVE-DEX-FULL-STACK-OUTPUT.md](NATIVE-DEX-FULL-STACK-OUTPUT.md) | `deploy-native-dex-full-stack` JSON reference |
| [NATIVE-DEX-LIMITS-RATIONALE.md](NATIVE-DEX-LIMITS-RATIONALE.md) | Why native DEX differs from EVM |
| [BOING-L1-DEX-ENGINEERING.md](BOING-L1-DEX-ENGINEERING.md) | L1 DEX engineering overview |

## Indexer & observer

| Doc | Description |
|-----|-------------|
| [INDEXER-RECEIPT-AND-LOG-INGESTION.md](INDEXER-RECEIPT-AND-LOG-INGESTION.md) | Receipt + log ingestion spec |
| [OBSERVER-HOSTED-SERVICE.md](OBSERVER-HOSTED-SERVICE.md) | Hosted observer architecture (OBS-1) |
| [BOING-OBSERVER-AND-EXPRESS.md](BOING-OBSERVER-AND-EXPRESS.md) | Explorer + wallet build spec |
| [INDEXER-OPERATOR-STATS.md](INDEXER-OPERATOR-STATS.md) | Operator stats / leaderboard |

## dApps, wallet & cross-repo

| Doc | Description |
|-----|-------------|
| [BOING-DAPP-INTEGRATION.md](BOING-DAPP-INTEGRATION.md) | dApp checklist + SDK patterns |
| [BOING-EXPRESS-WALLET.md](BOING-EXPRESS-WALLET.md) | Boing Express wallet spec |
| [BOING-RPC-ERROR-CODES-FOR-DAPPS.md](BOING-RPC-ERROR-CODES-FOR-DAPPS.md) | JSON-RPC / QA error codes |
| [BOING-SIGNED-TRANSACTION-ENCODING.md](BOING-SIGNED-TRANSACTION-ENCODING.md) | bincode layout |
| [THREE-CODEBASE-ALIGNMENT.md](THREE-CODEBASE-ALIGNMENT.md) | Sync boing.network / express / observer |
| [HANDOFF-DEPENDENT-PROJECTS.md](HANDOFF-DEPENDENT-PROJECTS.md) | Cross-repo work backlog |
| [HANDOFF_Boing_Network_Global_Token_Discovery.md](HANDOFF_Boing_Network_Global_Token_Discovery.md) | L1 DEX discovery RPC + consumer apps |
| [HANDOFF_NATIVE_DEX_DIRECTORY_R2_AND_CHAIN.md](HANDOFF_NATIVE_DEX_DIRECTORY_R2_AND_CHAIN.md) | Native DEX directory Worker (source of truth) |
| [HANDOFF_Universal_Contract_Deploy_Indexer.md](HANDOFF_Universal_Contract_Deploy_Indexer.md) | Universal deploy registry Worker |

## Patterns, design & misc

| Doc | Description |
|-----|-------------|
| [BOING-DESIGN-SYSTEM.md](BOING-DESIGN-SYSTEM.md) | Site design tokens |
| [BOING-CANONICAL-DEPLOY-ARTIFACTS.md](BOING-CANONICAL-DEPLOY-ARTIFACTS.md) | Pinned fungible / NFT bytecode |
| [BOING-REFERENCE-TOKEN.md](BOING-REFERENCE-TOKEN.md) | Reference fungible |
| [BOING-REFERENCE-NFT.md](BOING-REFERENCE-NFT.md) | Reference NFT |
| [BOING-PATTERN-AMM-LIQUIDITY.md](BOING-PATTERN-AMM-LIQUIDITY.md) | AMM pattern |
| [BOING-PATTERN-ORACLE-PRICE-FEEDS.md](BOING-PATTERN-ORACLE-PRICE-FEEDS.md) | Oracle pattern (future) |
| [BOING-PATTERN-UPGRADE-PROXY.md](BOING-PATTERN-UPGRADE-PROXY.md) | Upgrade proxy pattern (future) |
| [DECENTRALIZATION-AND-NETWORKING.md](DECENTRALIZATION-AND-NETWORKING.md) | P2P, discovery roadmap |
| [BOING-INFRASTRUCTURE-INDEPENDENCE.md](BOING-INFRASTRUCTURE-INDEPENDENCE.md) | Hosting independence |
| [NETWORK-COST-ESTIMATE.md](NETWORK-COST-ESTIMATE.md) | Cost overview |
| [E2-PARTNER-APP-NATIVE-BOING.md](E2-PARTNER-APP-NATIVE-BOING.md) | Partner native Boing apps |
| [Executive-Summary-Pitch-Deck.md](Executive-Summary-Pitch-Deck.md) | PDF source (pitch deck) |
| [ACCELERATOR-APPLICATIONS.md](ACCELERATOR-APPLICATIONS.md) | Draft accelerator answers |
