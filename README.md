# Boing Network

Authentic, decentralized L1 blockchain — built from first principles.

## Quick Start

```bash
cargo build
cargo run -p boing-node
```

The node serves **JSON-RPC over HTTP POST** on **`http://127.0.0.1:8545/`** by default (`--rpc-port` to change). Try:

```bash
curl -s -X POST http://127.0.0.1:8545/ -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"boing_health","params":[]}'
```

**Operators:** set **`BOING_CHAIN_ID`** and **`BOING_CHAIN_NAME`** on the process so `boing_getNetworkInfo` and **`boing_health`** expose chain metadata to wallets (see [docs/RPC-API-SPEC.md](docs/RPC-API-SPEC.md)).

**dApp / TypeScript:** the in-repo SDK is **`boing-sdk/`** (`npm install` / `npm run build` there, or use it as a workspace package). See **`boing-sdk/README.md`** and [docs/BOING-DAPP-INTEGRATION.md](docs/BOING-DAPP-INTEGRATION.md).

**Tutorial CLI from repo root:** with **`boing-sdk`** built and **`examples/native-boing-tutorial`** installed, you can run the same scripts as from the tutorial directory — e.g. **`npm run preflight-rpc`**, **`npm run deploy-native-dex-directory`**, **`npm run fetch-native-amm-reserves`**, **`npm run print-native-dex-routes`**, **`npm run indexer-ingest-tick`** — without changing directory. Wrappers live in the root **`package.json`**; full command map: [docs/PRE-VIBEMINER-NODE-COMMANDS.md](docs/PRE-VIBEMINER-NODE-COMMANDS.md) §4.

**Wallet / explorer / partner workstreams:** see [docs/HANDOFF-DEPENDENT-PROJECTS.md](docs/HANDOFF-DEPENDENT-PROJECTS.md) (complements [docs/THREE-CODEBASE-ALIGNMENT.md](docs/THREE-CODEBASE-ALIGNMENT.md)).

**Browser dApps:** additional CORS origins can be set without rebuilding via **`BOING_RPC_CORS_EXTRA_ORIGINS`** (comma-separated list, e.g. `http://localhost:9999,https://my-preview.pages.dev`). **`GET /ws`** supports a **newHeads** WebSocket (handshake in **`boing_getNetworkInfo.developer`**). Machine-readable API: **`boing_getRpcMethodCatalog`** and **`boing_getRpcOpenApi`** (see [docs/RPC-API-SPEC.md](docs/RPC-API-SPEC.md)).

**Ops / Kubernetes:** **`GET /`** returns **405** with **`Allow: GET, POST, OPTIONS`**; **`OPTIONS /`** returns **204** with the same **`Allow`** (discovery / CORS). Optional **`X-Request-Id`** is echoed on responses (or server-generated UUID). **`boing-sdk`:** set **`generateRequestId: true`** on **`BoingClient`** to send a fresh id per HTTP call. **`GET /live`** (process up) and **`GET /ready`** (state lock responsive; optional **`BOING_RPC_READY_MIN_PEERS`**) on the same port as JSON-RPC; paths are under **`boing_getNetworkInfo.developer.http`**. **`boing_health`** includes **`rpc_surface`** (batch max, WS cap, HTTP rate-limit RPS, optional ready peer floor). JSON-RPC **batch** on **`POST /`** (**`BOING_RPC_MAX_BATCH`**, default 32). HTTP **429** responses include **`Retry-After: 1`**. POST body default **8 MiB** (**`BOING_RPC_MAX_BODY_MB`**). WebSocket **`GET /ws`** optional cap: **`BOING_RPC_WS_MAX_CONNECTIONS`** (0 = unlimited).

## Crates

| Crate | Description |
|-------|-------------|
| `boing-primitives` | Types, hashing (BLAKE3), cryptography |
| `boing-consensus` | PoS + HotStuff BFT |
| `boing-state` | State store (Verkle tree target) |
| `boing-execution` | VM + parallel transaction scheduler |
| `boing-automation` | Scheduler, triggers, executor incentives |
| `boing-qa` | Protocol QA: Allow/Reject/Unsure checks for deployment (see [QUALITY-ASSURANCE-NETWORK.md](docs/QUALITY-ASSURANCE-NETWORK.md)) |
| `boing-cli` | `boing init`, `boing dev`, `boing deploy` |
| `boing-p2p` | libp2p networking |
| `boing-node` | Node binary |

## Docs

All project documentation lives in **[docs/](docs/)**. **Canonical index:** [docs/README.md](docs/README.md) (full map of specs, runbooks, and checklists). **Contributing:** [CONTRIBUTING.md](CONTRIBUTING.md).

| Doc | Description |
|-----|-------------|
| [**docs/README.md**](docs/README.md) | **Index of all `docs/*.md` files** |
| [**HANDOFF-DEPENDENT-PROJECTS.md**](docs/HANDOFF-DEPENDENT-PROJECTS.md) | **Cross-repo handoff** for boing.express, boing.observer, partner dApps |
| [**BOING-NETWORK-ESSENTIALS.md**](docs/BOING-NETWORK-ESSENTIALS.md) | **Six pillars, design philosophy, priorities, tech stack, key docs — start here** |
| [**TECHNICAL-SPECIFICATION.md**](docs/TECHNICAL-SPECIFICATION.md) | **Single source of truth: crypto, data formats, bytecode, gas, RPC, QA rules** |
| [READINESS.md](docs/READINESS.md) | Beta checklist, six-pillar readiness, launch-blocking path, verification commands |
| [BOING-BLOCKCHAIN-DESIGN-PLAN.md](docs/BOING-BLOCKCHAIN-DESIGN-PLAN.md) | Architecture, design decisions, innovations |
| [RUNBOOK.md](docs/RUNBOOK.md) | Operational runbook for node operators |
| [TESTNET.md](docs/TESTNET.md) | Join testnet (single vs multi-node, bootnodes, faucet); Testnet Portal (registration, dashboards, quests); Incentivized testnet (readiness, promotion, mainnet migration) |
| [DECENTRALIZATION-AND-NETWORKING.md](docs/DECENTRALIZATION-AND-NETWORKING.md) | Advanced P2P, peer discovery, WebRTC signaling, light clients |
| [DEVELOPMENT-AND-ENHANCEMENTS.md](docs/DEVELOPMENT-AND-ENHANCEMENTS.md) | SDK, automation, dApp incentives, enhancement vision; appendix covers cryptographic verification for automation |
| [SECURITY-STANDARDS.md](docs/SECURITY-STANDARDS.md) | Protocol, network, application, and operational security |
| [QUALITY-ASSURANCE-NETWORK.md](docs/QUALITY-ASSURANCE-NETWORK.md) | Protocol-enforced QA: automation + community pool; Appendices A–C (deployer checklist, malice definition, governance-mutable rules) |
| [BUILD-ROADMAP.md](docs/BUILD-ROADMAP.md) | Implementation tasks and phases |
| [NETWORK-COST-ESTIMATE.md](docs/NETWORK-COST-ESTIMATE.md) | Cost overview and economic parameters |
| [RPC-API-SPEC.md](docs/RPC-API-SPEC.md) | JSON-RPC API reference |
| [TESTNET-RPC-INFRA.md](docs/TESTNET-RPC-INFRA.md) | One map: testnet ops, public RPC, and infra (routing + env matrix) |
| [INFRASTRUCTURE-SETUP.md](docs/INFRASTRUCTURE-SETUP.md) | Testnet bootnodes, Cloudflare tunnel, deploy config |
| [WEBSITE-AND-DEPLOYMENT.md](docs/WEBSITE-AND-DEPLOYMENT.md) | Website spec, Cloudflare setup (D1, R2, KV), deployment |
| [BOING-EXPRESS-WALLET.md](docs/BOING-EXPRESS-WALLET.md) | Boing Express wallet: bootstrap, integration, Chrome Web Store, portal sign-in (Part 3) |
| [BOING-OBSERVER-AND-EXPRESS.md](docs/BOING-OBSERVER-AND-EXPRESS.md) | Observer + Express: in-repo vs build-outside; full explorer spec for boing.observer |

## Website

The [boing.network](https://boing.network) website lives in `website/`. It's built with Astro and deploys to Cloudflare Pages. See `website/README.md` and [docs/WEBSITE-AND-DEPLOYMENT.md](docs/WEBSITE-AND-DEPLOYMENT.md) for setup and deployment.

## Ecosystem

| App | URL | Description |
|-----|-----|-------------|
| **Explorer** | [boing.observer](https://boing.observer) | Block explorer: blocks, accounts, search, QA check |
| **Wallet** | [boing.express](https://boing.express) | Non-custodial Boing wallet (web + extension) |

For cross-repo alignment (RPC URLs, chain IDs, canonical links), see [docs/THREE-CODEBASE-ALIGNMENT.md](docs/THREE-CODEBASE-ALIGNMENT.md).

## Priorities

Security → Scalability → Decentralization → Authenticity → Transparency → **True quality assurance** (protocol-enforced QA: only quality assets on-chain; automation + community pool for edge cases; leniency for meme culture; no malice). See [BOING-NETWORK-ESSENTIALS.md](docs/BOING-NETWORK-ESSENTIALS.md) and [QUALITY-ASSURANCE-NETWORK.md](docs/QUALITY-ASSURANCE-NETWORK.md).
