# Boing Network Whitepaper

**Quality, enforced by the protocol.**

November 2026 · Public testnet (chain **6913**) · [boing.network](https://boing.network)

> This document is the protocol and product whitepaper for Boing Network: an independent Layer 1 under active public testnet. It is not a fundraising prospectus and does not announce a mainnet launch date. Facts that change with tip height should be re-checked on live RPC.

**Companion reading**

| Document | Role |
|----------|------|
| [BOING-NETWORK-ESSENTIALS.md](BOING-NETWORK-ESSENTIALS.md) | Plain-language product snapshot |
| [BOING-NETWORK-TECHNICAL-ESSENTIALS.md](BOING-NETWORK-TECHNICAL-ESSENTIALS.md) | Integrator / operator fact sheet |
| [BOING-TOKENOMICS-AND-INCENTIVIZED-TESTNET.md](BOING-TOKENOMICS-AND-INCENTIVIZED-TESTNET.md) | Plain-language tokenomics + incentivized-testnet framing (**BN-155:** 1B genesis, uncapped afterward; testnet ≠ mainnet claim) |
| [BOING-INITIAL-TOKENOMICS.md](BOING-INITIAL-TOKENOMICS.md) | Initial allocation draft (Proposed vs Undecided); supply ceiling locked uncapped after 1B genesis |
| [FEATURES.md](FEATURES.md) | Shipped / gated / deferred timeline with evidence |
| [SIX-PILLARS.md](SIX-PILLARS.md) | Priority order when goals collide |
| [QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md) | Deploy quality gate narrative |
| [RPC-API-SPEC.md](RPC-API-SPEC.md) | Canonical JSON-RPC catalog |
| [TECHNICAL-SPECIFICATION.md](TECHNICAL-SPECIFICATION.md) | Crypto, VM ISA, gas, QA rules |

---

## 1. Abstract

Boing Network is an independent Layer 1 blockchain designed so that **security, authenticity, and deploy-time quality checks are protocol responsibilities**, not after-the-fact marketplace moderation. Consensus is proof-of-stake HotStuff BFT. Execution uses a purpose-built stack VM (not the EVM). State commits through sparse Merkle trees. Cryptography is BLAKE3 for digests and Ed25519 for signatures and account identity.

The chain ships with first-party surfaces that matter in practice: a non-custodial wallet (**Boing Express**), a native constant-product DEX and app hub (**Boing Finance**), a block explorer (**Boing Observer**), L1 media blobs for large on-chain content, and a **Quality Gate** that screens contract and token deploys before they land on the ledger.

**Public testnet is live** on chain id **6913** (`Boing Testnet`). **Mainnet (planned chain id 6914) is not live.** No mainnet date is asserted here. Independent security audits are not finished. Day-to-day public traffic still often rides a small hosted validator set even though multi-validator machinery exists in code and ops.

---

## 2. Motivation

Most public chains treat “quality” as a social layer: explorers label scams after the fact, app stores ban listings, and users absorb the cost of deceptive deploys. Boing treats deploy screening as a **protocol-visible decision** — Allow, Reject, or Unsure — with automation first and a community review path for edge cases (canonical on-chain QA policy and public Unsure pool semantics activate at height **1700**; see §6 and [FEATURES.md](FEATURES.md)).

Separately, many “new L1” efforts inherit another chain’s VM, account model, and cultural defaults. Boing’s authenticity claim is narrower and stronger: **one network, one account model, one execution ISA**, with familiar UX patterns where they help integrators — without pretending chain **6913** is an EVM chain.

Pillar order when goals collide ([SIX-PILLARS.md](SIX-PILLARS.md)):

**Security → Scalability → Decentralization → Authenticity → Transparency → True quality assurance**

True QA is last because it is a product of the others working; it is not allowed to punch a hole in consensus safety to look strict.

---

## 3. Design principles

1. **Correctness before throughput.** Failed or underfunded transactions must not stall block production; safety properties belong in protocol rules and gas metering.
2. **Native execution.** Boing VM bytecode only. No foreign bytecode engine in consensus.
3. **Honest product surfaces.** Wallet, explorer, DEX, and docs must reflect what the node actually exposes (RPC method catalogs are unit-tested against `BOING_RPC_SUPPORTED_METHODS`).
4. **Height-gated change.** Breaking or sensitive protocol behavior activates at published heights or env-configured heights — not silent flag flips.
5. **No invented readiness.** Testnet tip, activation fields, and gated features are re-verified on `boing_health` / `boing_getNetworkInfo`. Calendar claims for mainnet are out of scope until announced.

---

## 4. Architecture

### 4.1 Chain identity

| Fact | Value |
|------|--------|
| Public testnet chain id | **6913** (`0x1b01`) — *Boing Testnet* |
| Planned mainnet chain id | **6914** (`0x1b02`) — **not live** |
| Public RPC | `https://testnet-rpc.boing.network/` |
| Hosted public node | Fly **`boing-testnet-1` only** (`boing-testnet-2` retired **2026-10-24**) |
| Bootnode | `/ip4/169.155.48.188/tcp/4001` |
| Node | Rust `boing-node` |
| Hashing | BLAKE3 (32-byte digests) |
| Signatures / accounts | Ed25519; 32-byte pubkey = `AccountId` (`0x` + 64 hex) |
| Consensus | PoS + HotStuff BFT over libp2p (TCP, Noise, gossipsub, request-response) |
| Execution | Boing VM (custom stack ISA) |
| State | Sparse Merkle trees (account + per-contract storage); Verkle is a stated target |

Addresses are **not** Ethereum addresses. Wallets and dApps must not treat **6913** as an EVM chain id.

Transaction signing uses domain **`BOING_TX_V2`**; optional priority tips use **`BOING_TX_PRIORITY`**. Wire decoding must use `SignedTransaction::from_bincode` / `Block::from_bincode` — zero tips and empty signature vectors are omitted on the wire ([BOING-SIGNED-TRANSACTION-ENCODING.md](BOING-SIGNED-TRANSACTION-ENCODING.md)).

### 4.2 Consensus and networking

Blocks target roughly **two-second** intervals when transactions are pending; empty-chain idle behavior avoids needless empty blocks. Parallel scheduling uses signed **access lists**; disjoint read/write sets may execute in the same batch.

Open validator registration, stake-derived epochs, VRF leader election, and equivocation reporting exist in the codebase and ops surface. Public day-to-day traffic has often used a **small hosted validator path**; live `boing_getNetworkInfo.consensus.validator_count` should be treated as authoritative over marketing copy. Equivocation slash activation is gated at height **1600**.

### 4.3 Account and storage trees

Account leaves commit:

```text
BLAKE3(balance_le || nonce_le || stake_le || code_hash || storage_root)
```

From height **1500** on testnet **6913**:

- **Storage-tree v2** — per-contract storage routes on all **256** key bits (fixes 64-bit prefix collision).
- **Account-tree v3** — account routing uses full 256-bit ids.
- **VM hardening** — init-code QA, memory gas, stack depth **1024**.

Strict JUMPI and oversized `NATIVE_TRANSFER` reporting activate at **1600**. Details: [STORAGE-TREE-V2-ACTIVATION.md](STORAGE-TREE-V2-ACTIVATION.md), [ACCOUNT-TREE-V3-ACTIVATION.md](ACCOUNT-TREE-V3-ACTIVATION.md).

### 4.4 Boing VM

Deterministic stack machine (`boing-execution`). Opcode families include arithmetic, context (`ADDRESS`, `CALLER`, `BLOCKHEIGHT`, …), memory/storage, control flow, `CALL` / `CREATE2`, logs, and native-value opcodes active from height **1150** (`BALANCE`, `CALLVALUE`, `SELFBALANCE`, `NATIVE_TRANSFER`). Blake3 opcode `0x20` is **env-gated** (unset = never). Bytecode size cap under QA: **32 KiB**. Contract-call interpreter budget: **`GAS_PER_CONTRACT_CALL = 40_000_000`**. Full ISA: [TECHNICAL-SPECIFICATION.md](TECHNICAL-SPECIFICATION.md).

### 4.5 Denomination and fees

- **1 BOING = 10¹⁸** base units (`u128` ledger; decimal strings on RPC).
- Testnet **6913**: whole-BOING below height **1050**; base units from **1050**.
- Gas price constant **10¹²** base units per gas before micro BPS.
- Fee market v1: recommended `execution_gas_fee_bps = 100` → about **0.00021 BOING** per 21k transfer (active from **1188**). Optional `priority_tip` for ordering.
- Media blob / Forever pin fees are a **separate** fee path from execution gas.

---

## 5. Current testnet snapshot

*Probed `https://testnet-rpc.boing.network/` on **2026-11-02**. Tip moves — re-check before claiming later gates crossed.*

| Fact | Value |
|------|--------|
| Chain | **6913** / Boing Testnet |
| Tip (probe) | **1523** |
| Public RPC | Cloudflare Worker gateway → Fly `boing-testnet-1` |
| Recommended SDK | `@boing-network/boing-sdk@^0.5.39` |
| Recommended wallet | Boing Express **0.1.24** (min advisory **0.1.18**) |
| DEX discovery | `boing_listDexPools`, `boing_listDexTokens`, `boing_getDexToken` |
| Canonical CP pool / factory | Published under `end_user` — prefer live fields |

### Selected activation heights (6913)

| Height | Feature | Status near tip ~1523 |
|-------:|---------|------------------------|
| 1050 | 18-decimal native BOING | Crossed |
| 1150 | Native value on calls + related opcodes | Crossed |
| 1188 | Micro fees (`execution_gas_fee_bps=100`) | Crossed |
| 1200 | Media blob Put/Announce admission | Live |
| **1306** | PHASE7 **256 MiB** blob consensus cap | Live |
| ~1317 | Phase 34 private-by-default ACL | Confirm on live media params |
| **1500** | Storage-tree v2 + account-tree v3 + VM hardening | **Crossed** |
| **1600** | Equivocation slash + strict JUMPI / NATIVE_TRANSFER reporting | **Gated** |
| **1700** | Canonical on-chain QA policy + public Unsure pool semantics | **Gated** |
| unset | Blake3 opcode `0x20`; governance consensus txs 37–39; BLS committee flip | Off until env height set |

Authoritative timeline with tags and confidence: [FEATURES.md](FEATURES.md). Compact integrator sheet: [BOING-NETWORK-TECHNICAL-ESSENTIALS.md](BOING-NETWORK-TECHNICAL-ESSENTIALS.md).

---

## 6. Quality Gate

Protocol QA screens **deploy** bytecode (and related metadata) before admission:

- Outcomes: **Allow / Reject / Unsure**
- Preflight: `boing_qaCheck` with structured rejection codes for wallets and UIs
- Rule catalog: [QA-GATE-RULES.md](QA-GATE-RULES.md)
- Product narrative and deployer checklist: [QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md)

Intent: reduce malicious or deceptive junk while still allowing playful meme and community projects under transparent rules. Live registry lists on public RPC may be empty or size-capped until policy is applied — query `boing_getQaRegistry`.

**Honesty constraint:** canonical on-chain QA policy and the public Unsure review pool are **gated at height 1700**. Until tip crosses that height, do not claim the public pool is live. Governance of rule lists via time-lock remains partial / roadmap.

The Quality Gate does not replace user diligence. It reduces risk and makes decisions auditable.

---

## 7. Native DEX

Boing’s native constant-product AMM, factory, routers, and LP vaults are **Boing VM contracts** — not an imported EVM DEX. Selectors and CREATE2 salts live in [NATIVE-AMM-CALLDATA.md](NATIVE-AMM-CALLDATA.md); capability overview in [BOING-NATIVE-DEX-CAPABILITY.md](BOING-NATIVE-DEX-CAPABILITY.md).

**Discovery is layered** (clients should merge; prefer live RPC):

1. L1 RPC — `boing_listDexPools` / `boing_listDexTokens` / `boing_getDexToken` (also advertised under `boing_getNetworkInfo.developer.dex_discovery_methods`)
2. `end_user` — published canonical factory / default CP pool ids
3. Optional indexer / directory Worker (`workers/native-dex-indexer`)
4. Hydrated pool venue metadata in Finance / SDK for UI labels

Product orientation: Finance **Swap** targets the network default CP pool; additional factory pairs and multihop sit behind **Pools** / Smart route. Planet-scale subgraph history remains deferred / partial.

---

## 8. L1 media blobs

Large media belongs on a **blob fee path**, not account `SSTORE`, so NFT art does not bid up everyday transfer gas.

- Phases **1–33** live; Put/Announce admission from height **1200**
- **PHASE7 256 MiB** consensus cap from height **1306**
- Phase **34** download ACL (Grant/Revoke) shipped; private-by-default activation should be confirmed on live params
- Block JSON uses `0x` hex byte fields; large vectors stub with `include_code: false`
- PutBlob **publish sessions** (`boing_requestPutBlobSession`) are **Express-side**, not node RPC
- Deferred for product claims: Phase 34.1 capability tickets; real ReservedRealV1 DAS verification

Protocol detail: [BOING-L1-MEDIA-BLOBS.md](BOING-L1-MEDIA-BLOBS.md), [HANDOFF_BLOB_DOWNLOAD_ACL.md](HANDOFF_BLOB_DOWNLOAD_ACL.md).

---

## 9. Product surfaces

| Surface | Role | Notes |
|---------|------|-------|
| **Boing Express** | Non-custodial wallet | Recommended **0.1.24**; `window.boing`; v2 signing; PutBlob publish sessions |
| **Boing Finance** | Swaps, pools, tokens, NFTs | Native VM `AccountId`s — not EVM addresses on 6913 |
| **Boing Observer** | Explorer | Public testnet RPC via same-origin proxy; slim blocks; QA visibility |
| **Website / faucet** | Docs, portal, faucet | [boing.network](https://boing.network) |
| **@boing-network/boing-sdk** | TypeScript client | **^0.5.39** on npm (scoped package name required) |

Integration entry: [BOING-DAPP-INTEGRATION.md](BOING-DAPP-INTEGRATION.md). Everyday overview: [BOING-NETWORK-ESSENTIALS.md](BOING-NETWORK-ESSENTIALS.md).

---

## 10. RPC and integrator contract

JSON-RPC 2.0 over HTTP POST. Public gateway: `https://testnet-rpc.boing.network/`. Method catalog is enforced in CI against node internals ([RPC-API-SPEC.md](RPC-API-SPEC.md)).

Integrator-relevant groups include health/discovery (`boing_health`, `boing_getNetworkInfo`), chain reads (prefer `include_code: false` for large blocks), submit/simulate, QA (`boing_qaCheck`), DEX discovery, token metadata, media blobs, and operator methods (fail-closed without `BOING_OPERATOR_RPC_TOKEN`).

Browser apps need correct **CORS** or a same-origin proxy. Public gateway and explorer proxies cap JSON bodies near **6 MiB** and reject truncated bodies.

---

## 11. What is not claimed

Until explicitly announced and activated:

- **Mainnet** (chain **6914**) — not live; **no launch date** in this document
- Independent security audits — not finished
- Public Unsure QA pool as tip-live behavior — gated at **1700**
- Blake3 opcode, governance consensus txs 37–39, BLS media committee flip — env-gated off until set
- Bridges, light clients, DHT/WebRTC incentivized relays — deferred
- Incentivized testnet rewards — not currently running (path and pots: [BOING-TOKENOMICS-AND-INCENTIVIZED-TESTNET.md](BOING-TOKENOMICS-AND-INCENTIVIZED-TESTNET.md); draft tables: [BOING-INITIAL-TOKENOMICS.md](BOING-INITIAL-TOKENOMICS.md))
- Durable planet-scale Observer (OBS-1) and full DEX subgraph history — deferred / partial

Plans and backlog: [BUILD-ROADMAP.md](BUILD-ROADMAP.md), [NEXT-STEPS-FUTURE-WORK.md](NEXT-STEPS-FUTURE-WORK.md), [FEATURES.md](FEATURES.md) “Explicitly not yet”.

---

## 12. Conclusion

Boing Network is building an L1 where **deploy quality is a first-class protocol concern**, execution is native rather than rented, and product surfaces (wallet, DEX, explorer, blobs) are wired to what the chain actually exposes. Public testnet **6913** is the live proving ground. This whitepaper will track activations and product truth as tip advances; it will not invent mainnet calendars.

*Boing Network — Quality, enforced by the protocol.*  
*Aligned with FEATURES / Technical Essentials and a live RPC probe on 2026-11-02; re-check tip and activation fields before asserting later gates.*
