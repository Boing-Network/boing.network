# Boing Network — Essentials

> **One place for the essentials of the Boing blockchain network:** the six pillars, design philosophy, priorities, tech stack, and pointers to the rest of the docs.

---

## What Is Boing Network?

**Boing Network** is an authentic, decentralized **L1 blockchain** built from first principles. It is optimized for efficiency, free from dependencies on other chains, and committed to **100% transparency** and **true quality assurance** at the protocol layer. This document summarizes the core commitments and where to go for detail.

---

## The Six Pillars

The Boing blockchain prioritizes, in order:

| # | Pillar | What it means |
|---|--------|----------------|
| **1** | **Security** | Safety and correctness over speed. |
| **2** | **Scalability** | Throughput and efficient resource use. |
| **3** | **Decentralization** | Permissionless participation at every layer. |
| **4** | **Authenticity** | Unique architecture and identity (not a fork or framework). |
| **5** | **Transparency** | 100% openness in design, governance, and operations — the foundation for community trust. |
| **6** | **True quality assurance** | Top-notch standards with built-in automation: only assets that meet protocol-defined rules and security bar are allowed on-chain. All currently known edge cases are resolved by the automated regulatory QA system (Allow or Reject); **leniency for meme culture** (meme/community/entertainment are valid purposes), with **no maliciousness or malignancy** allowed. The community QA pool is only for genuinely unknown or policy-mandated cases. See [QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md). |

---

## Design Philosophy

- **Authentic** — Our own architecture, not a fork or framework.
- **Independent** — Zero reliance on Solana, Cosmos, Ethereum, or other chains.
- **Optimal** — Adopt the best ideas from the ecosystem when they are demonstrably superior.
- **Unique** — A distinct identity and technical story.
- **Decentralized** — Absolute decentralization as a foundational requirement.
- **Transparent** — 100% transparent in how we build, govern, and operate — trust through verifiability, not promises.

---

## Priorities (Order of Precedence)

When trade-offs arise, the network applies this order:

1. **Security** → 2. **Scalability** → 3. **Decentralization** → 4. **Authenticity** → 5. **Transparency** → 6. **True quality assurance**

---

## Tech Stack at a Glance

| Layer | Technology |
|-------|------------|
| **Language** | Rust |
| **Hashing** | BLAKE3 |
| **Signatures** | Ed25519 |
| **Consensus** | PoS + HotStuff BFT |
| **State** | Sparse Merkle tree (Verkle target) |
| **Execution** | Custom VM (stack-based; opcodes inspired by EVM, simplified) |
| **Networking** | libp2p (TCP, Noise, gossipsub, request-response) |
| **Governance** | Phased (proposal → cooling → execution); time-locked |

---

## Crates (Implementation)

| Crate | Role |
|-------|------|
| `boing-primitives` | Types, BLAKE3, Ed25519, Transaction, Block, AccountId |
| `boing-consensus` | PoS + HotStuff BFT |
| `boing-state` | State store, state root, checkpoints |
| `boing-execution` | VM, BlockExecutor, TransactionScheduler |
| `boing-tokenomics` | Block emission, dApp incentives |
| `boing-governance` | Time-locked governance, slashing appeal |
| `boing-automation` | Scheduler, triggers, executor incentives, verification |
| `boing-qa` | Protocol QA: Allow/Reject/Unsure for deployments |
| `boing-cli` | `boing init`, `boing dev`, `boing deploy` |
| `boing-p2p` | libp2p networking |
| `boing-node` | Node binary (RPC, mempool, block producer, chain) |

---

## Key Documents

| Document | Use it for |
|----------|------------|
| [TECHNICAL-SPECIFICATION.md](TECHNICAL-SPECIFICATION.md) | Single source of truth: crypto, data formats, bytecode, gas, RPC, QA rules |
| [BOING-BLOCKCHAIN-DESIGN-PLAN.md](BOING-BLOCKCHAIN-DESIGN-PLAN.md) | Full architecture, innovations, tokenomics, design decisions |
| [QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md) | Sixth pillar: QA rules, automation, community pool, meme leniency, no malice; Appendix A: deployer checklist; Appendix B: canonical malice definition |
| [BUILD-ROADMAP.md](BUILD-ROADMAP.md) | Implementation phases and progress |
| [RUNBOOK.md](RUNBOOK.md) | Running nodes, RPC, CLI, monitoring, incidents |
| [TESTNET.md](TESTNET.md) | Join testnet, Testnet Portal (registration, quests), Incentivized testnet (readiness, promotion, mainnet migration) |
| [RPC-API-SPEC.md](RPC-API-SPEC.md) | JSON-RPC methods and error codes |
| [SECURITY-STANDARDS.md](SECURITY-STANDARDS.md) | Protocol, network, application, and operational security |
| [DEVELOPMENT-AND-ENHANCEMENTS.md](DEVELOPMENT-AND-ENHANCEMENTS.md) | SDK, automation, vision; network-wide enhancements (implemented and planned) |

All docs live in **[docs/](docs/)**. The [README](../README.md) in the repo root lists the full doc set.

---

## Quick Reference

| Item | Default / convention |
|------|----------------------|
| **RPC port** | 8545 |
| **Address (AccountId)** | 32 bytes, hex-encoded (64 hex chars; optional `0x` prefix) |
| **Transaction format** | Bincode-serialized; signed with Ed25519 over BLAKE3 signable hash |
| **Node binary** | `cargo run -p boing-node` |
| **CLI** | `boing` (init, dev, deploy, metrics register, completions) |

---

## Transparency Commitment

We commit to 100% transparency in:

- **Protocol design** — Open design docs, public specs, auditable code.
- **Tokenomics** — Emission, fee splits, burn — on-chain and documented.
- **Governance** — On-chain proposals, votes, outcomes.
- **Validator & staking** — Clear slashing, reward formulas, distribution.
- **Security & audits** — Audit reports public; known risks disclosed.
- **Development** — Open source; roadmap and trade-offs discussed openly.

---

*Boing Network — Authentic. Decentralized. Optimal. Sustainable. Quality-assured.*
