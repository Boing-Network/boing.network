# Boing Network

Authentic, decentralized L1 blockchain — built from first principles.

## Quick Start

```bash
cargo build
cargo run -p boing-node
```

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

All project documentation lives in **[docs/](docs/)**:

| Doc | Description |
|-----|-------------|
| [**BOING-NETWORK-ESSENTIALS.md**](docs/BOING-NETWORK-ESSENTIALS.md) | **Six pillars, design philosophy, priorities, tech stack, key docs — start here** |
| [**TECHNICAL-SPECIFICATION.md**](docs/TECHNICAL-SPECIFICATION.md) | **Single source of truth: crypto, data formats, bytecode, gas, RPC, QA rules** |
| [BETA-READINESS.md](docs/BETA-READINESS.md) | Beta checklist, validator & developer quick starts, verification commands |
| [BOING-BLOCKCHAIN-DESIGN-PLAN.md](docs/BOING-BLOCKCHAIN-DESIGN-PLAN.md) | Architecture, design decisions, innovations |
| [RUNBOOK.md](docs/RUNBOOK.md) | Operational runbook for node operators |
| [TESTNET.md](docs/TESTNET.md) | Join testnet: single vs multi-node, bootnodes, faucet |
| [VIBEMINER-INTEGRATION.md](docs/VIBEMINER-INTEGRATION.md) | One-click mining/validator integration (VibeMiner) |
| [DECENTRALIZATION-AND-NETWORKING.md](docs/DECENTRALIZATION-AND-NETWORKING.md) | Advanced P2P, peer discovery, WebRTC signaling, light clients |
| [DEVELOPMENT-AND-ENHANCEMENTS.md](docs/DEVELOPMENT-AND-ENHANCEMENTS.md) | SDK, automation, dApp incentives, enhancement vision |
| [SECURITY-STANDARDS.md](docs/SECURITY-STANDARDS.md) | Protocol, network, application, and operational security |
| [QUALITY-ASSURANCE-NETWORK.md](docs/QUALITY-ASSURANCE-NETWORK.md) | Protocol-enforced QA: only quality assets allowed on-chain; automation + community pool; §16 enhancements |
| [BUILD-ROADMAP.md](docs/BUILD-ROADMAP.md) | Implementation tasks and phases |
| [AUTOMATION-VERIFICATION.md](docs/AUTOMATION-VERIFICATION.md) | Cryptographic verification for decentralized automation |
| [NETWORK-COST-ESTIMATE.md](docs/NETWORK-COST-ESTIMATE.md) | Cost overview and economic parameters |
| [RPC-API-SPEC.md](docs/RPC-API-SPEC.md) | JSON-RPC API reference |
| [INFRASTRUCTURE-SETUP.md](docs/INFRASTRUCTURE-SETUP.md) | Testnet bootnodes, Cloudflare tunnel, deploy config |
| [WEBSITE-AND-DEPLOYMENT.md](docs/WEBSITE-AND-DEPLOYMENT.md) | Website spec, Cloudflare setup (D1, R2, KV), deployment |
| [BOING-EXPRESS-WALLET.md](docs/BOING-EXPRESS-WALLET.md) | Boing Express wallet: bootstrap, integration & Chrome Web Store |

## Website

The [boing.network](https://boing.network) website lives in `website/`. It's built with Astro and deploys to Cloudflare Pages. See `website/README.md` and [docs/WEBSITE-AND-DEPLOYMENT.md](docs/WEBSITE-AND-DEPLOYMENT.md) for setup and deployment.

## Priorities

Security → Scalability → Decentralization → Authenticity → Transparency → **True quality assurance** (protocol-enforced QA: only quality assets on-chain; automation + community pool for edge cases; leniency for meme culture; no malice). See [BOING-NETWORK-ESSENTIALS.md](docs/BOING-NETWORK-ESSENTIALS.md) and [QUALITY-ASSURANCE-NETWORK.md](docs/QUALITY-ASSURANCE-NETWORK.md).
