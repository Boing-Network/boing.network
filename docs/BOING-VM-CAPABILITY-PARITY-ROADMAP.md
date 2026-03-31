# Boing VM — full capability parity roadmap

This document is the **product-facing** companion to [EXECUTION-PARITY-TASK-LIST.md](EXECUTION-PARITY-TASK-LIST.md). The execution task list tracks **protocol and VM crate work** (receipts, opcodes, QA, RPC). This roadmap tracks **end-to-end capability parity**: what developers and users expect from mature chains (EVM / Solana–class workflows), delivered **through Boing’s native stack** (VM, txs, wallet, SDK), not through Ethereum bytecode compatibility.

---

## What “parity” means here

| In scope | Out of scope (by design) |
|----------|---------------------------|
| Same **categories** of capability: auth, transfers, contract deploy/call, events, simulation, proofs, standards (tokens/NFTs), predictable deploy addresses | **Drop-in** ethers.js / MetaMask EVM signing for Solidity on Boing |
| **Boing-native** APIs: `boing_*` RPC, `boing_signTransaction` / `boing_sendTransaction`, `boing-sdk` | Identical opcode set or ABI encoding as Ethereum |
| Clear **mapping docs**: “On EVM you X; on Boing you Y” | Solana BPF or SPL compatibility |

**Pillar rule:** New protocol behavior still flows through QA and docs ([QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md), [RPC-API-SPEC.md](RPC-API-SPEC.md), `boing-qa`).

---

## Capability matrix (status snapshot)

Use this to prioritize. Update statuses in PRs as items land.

| Capability area | EVM / Solana analogue | Boing today | Target surface |
|-----------------|----------------------|-------------|----------------|
| **Chain read** | `eth_getBalance`, slot reads | `boing_getBalance`, `boing_getAccount`, blocks, proofs | `boing-sdk` + RPC (done) |
| **Simulation** | `eth_call`, `simulateTransaction` | `boing_simulateTransaction`, suggested access list | SDK helpers (partial — see [accessList.ts](../boing-sdk/src/accessList.ts)) |
| **Signed tx submit** | `eth_sendRawTransaction` | `boing_submitTransaction` (hex bincode) | SDK + wallet |
| **Wallet: accounts** | `eth_requestAccounts` | `boing_requestAccounts` + aliases | [BOING-EXPRESS-WALLET.md](BOING-EXPRESS-WALLET.md) |
| **Wallet: message sign** | `personal_sign` | `boing_signMessage` (BLAKE3 + Ed25519) | Express + portal |
| **Wallet: tx sign/send** | `eth_signTransaction` / send | `boing_signTransaction`, `boing_sendTransaction` | Express; dApps must call native methods |
| **Receipts / tx result** | transaction receipts | `ExecutionReceipt`, `boing_getTransactionReceipt`, logs in receipt | Done — see Track R in execution parity list |
| **Events / logs** | EVM logs | `LOG0`–`LOG4`, receipt logs | Done — Track L |
| **Contract identity** | `CALLER`, `ADDRESS` | Same spirit in VM | Done — Track C |
| **Deploy addressing** | CREATE / CREATE2 | Nonce deploy + CREATE2-style salt | Done — Track D |
| **Token / NFT patterns** | ERC-20 / SPL | Reference layouts + QA purpose deploys | [BOING-REFERENCE-TOKEN.md](BOING-REFERENCE-TOKEN.md), [BOING-REFERENCE-NFT.md](BOING-REFERENCE-NFT.md) |
| **Access lists** | — / Solana accounts | Declared read/write sets + simulation hints | Done — Track A |
| **Finality wording** | finalized slots | documented + optional RPC | Done — Track X |
| **JS tx build + sign** | viem/ethers builders | `boing-sdk` bincode + `signTransactionInput` / `signTransactionInputWithSigner`; vitest golden vs Rust | **Phase 1 (shipped)** — see P4 for simulate→submit tutorial |
| **High-level SDK** | Contract class, encode ABI-like calls | Reference calldata encoders + `submit*WithSimulationRetry` / deploy flow | **Partial — Phase 1–2** (no generic contract class yet) |
| **Indexer / explorer** | The Graph, block explorers | Observer spec + receipt/log ingestion + bounded `boing_getLogs` | **Partial — Phase 3** (I1–I3 shipped; heavy indexing still pull/replay) |
| **Assembler / toolchain** | solc, Anchor | Bytecode + QA; no full HLL | **Gap — Phase 2–4** |

---

## Phase 0 — Protocol & VM foundation (largely complete)

Tracked in detail in [EXECUTION-PARITY-TASK-LIST.md](EXECUTION-PARITY-TASK-LIST.md).

- Receipts, VM opcode batches, caller/address, logs, access-list simulation hints, reference token/NFT docs, CREATE2-style deploys, finality docs.

**Maintenance:** As new opcodes or receipt fields ship, update execution parity list + this matrix row in the same PR.

---

## Phase 1 — dApp and wallet **surface parity** (highest leverage)

Goal: A web dApp can do **full Boing flows without Rust**, using documented patterns.

### Track P — SDK: transaction construction in TypeScript

- [x] **P1** — **Spec** the canonical JS/TS encoding path for `SignedTransaction` (bincode layout **bit-for-bit** with `boing-primitives`; document endianness and enum discriminants — cross-link execution parity bincode note in [BOING-EXPRESS-WALLET.md](BOING-EXPRESS-WALLET.md)). **Done:** [BOING-SIGNED-TRANSACTION-ENCODING.md](BOING-SIGNED-TRANSACTION-ENCODING.md), `boing-sdk/src/bincode.ts`, golden tests vs `dump_bincode` example; Rust `signable_transaction_hash` exported for tooling.
- [x] **P2** — Implement **builder API** in `boing-sdk`: `Transfer`, `ContractCall`, `ContractDeployWithPurpose*`, `AccessList`, nonce fetch via `getAccount`, gas/fee fields per spec. **Done:** `buildTransferTransaction`, `buildContractCallTransaction`, `buildDeployWithPurposeTransaction`, `fetchNextNonce`; gas/fee remain node/VM enforced (no extra fields on `Transaction` yet).
- [x] **P3** — **Signing hook**: accept `sign(bytes) => Promise<Uint8Array>` (Ed25519 signature) so apps can use local keys, hardware, or delegate to wallet via extension messaging if needed; document **recommended** path: Boing Express `boing_signTransaction` for browser dApps. **Done:** `signTransactionInput` (32-byte secret) + `signTransactionInputWithSigner`; browser path still documented in [BOING-DAPP-INTEGRATION.md](BOING-DAPP-INTEGRATION.md).
- [x] **P4** — End-to-end example: `simulate` → adjust access list → `submit` with structured `BoingRpcError` / QA handling (extend [boing-sdk README](../boing-sdk/README.md)). **Done:** `submitFlow.ts`, `explainBoingRpcError`, README section + [examples/native-boing-tutorial](../examples/native-boing-tutorial/).

### Track W — Wallet contract with dApps

- [x] **W1** — Publish a **minimal dApp integration guide** (checklist): connect → chain id → simulate → sign → submit; error codes when locked or user rejects. **Done:** [BOING-DAPP-INTEGRATION.md](BOING-DAPP-INTEGRATION.md).
- [x] **W2** — Align **Boing Express** error strings / RPC codes with [RPC-API-SPEC.md](RPC-API-SPEC.md) so SDK can map them. **Done (network repo):** [BOING-RPC-ERROR-CODES-FOR-DAPPS.md](BOING-RPC-ERROR-CODES-FOR-DAPPS.md) + `explainBoingRpcError`. **Done (boing.express):** node JSON-RPC **`code`**, **`message`**, and **`data`** forwarded on `RpcClientError` / `window.boing.request` failures (`data.boingCode === 'BOING_NODE_JSONRPC'` + nested `rpc`).
- [x] **W3** — Optional: **EIP-6963** + namespaced `boing` hint documented for Express ([BOING-EXPRESS-WALLET.md](BOING-EXPRESS-WALLET.md#eip-6963-optional-multi-wallet-discovery)); [BOING-DAPP-INTEGRATION.md](BOING-DAPP-INTEGRATION.md) §8 cross-link. Wallet adoption still optional.

### Track E — Reference dApps

- [x] **E1** — **Single canonical tutorial repo or monorepo package**: deploy reference token bytecode + `contract_call` transfer using only SDK + Express (no ethers for chain txs). **Done:** [examples/native-boing-tutorial](../examples/native-boing-tutorial/) (Node SDK scripts; browser path documented via Express).
- [x] **E2** — **boing.finance** (or partner app): one **native Boing** deploy path for at least one user-facing flow (e.g. “deploy on Boing” via `boing_sendTransaction`), documented as the pattern others copy. **Done:** [E2-PARTNER-APP-NATIVE-BOING.md](E2-PARTNER-APP-NATIVE-BOING.md); **boing.finance** Deploy Token links to this guide when a native Boing account is connected.

---

## Phase 2 — **Application-pattern** parity (DeFi / NFT / gaming)

Goal: Document and ship **patterns** that match what EVM/Solana ecosystems do, implemented as **Boing contracts + conventions** (not new L1 features unless required).

- [x] **F1** — **AMM / liquidity**: spec minimal constant-product or orderbook-on-state pattern using current VM; reference bytecode or pseudocode; QA category alignment. **Done:** [BOING-PATTERN-AMM-LIQUIDITY.md](BOING-PATTERN-AMM-LIQUIDITY.md).
- [x] **F2** — **NFT marketplace / royalties**: extend reference NFT doc with optional metadata keys and example call sequences. **Done:** [BOING-REFERENCE-NFT.md](BOING-REFERENCE-NFT.md) § Marketplace / royalties.
- [x] **F3** — **Upgrade / proxy patterns** (if allowed by QA): document immutability vs allowed delegate patterns; if forbidden, document **why** in QA doc. **Done:** [BOING-PATTERN-UPGRADE-PROXY.md](BOING-PATTERN-UPGRADE-PROXY.md) + [QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md) Appendix D.
- [x] **F4** — **Oracle / price feeds**: app-layer design (multi-sig update, TWAP on-contract) — no chain oracle opcode required initially. **Done:** [BOING-PATTERN-ORACLE-PRICE-FEEDS.md](BOING-PATTERN-ORACLE-PRICE-FEEDS.md).

---

## Phase 3 — Observability and indexing

Goal: **Solana/EVM-class** visibility into chain history and contract activity.

- [x] **I1** — Receipt + log ingestion spec for [BOING-OBSERVER-AND-EXPRESS.md](BOING-OBSERVER-AND-EXPRESS.md) / indexer operators (schemas, replay rules). **Done:** [INDEXER-RECEIPT-AND-LOG-INGESTION.md](INDEXER-RECEIPT-AND-LOG-INGESTION.md).
- [x] **I2** — Bounded **`boing_getLogs`** (block range + optional `address` / `topics`); caps in [RPC-API-SPEC.md](RPC-API-SPEC.md); integration test `receipts_rpc_integration.rs`.
- [x] **I3** — **SDK:** `BoingClient.getLogs`, `receiptLogs.ts` helpers (`normalizeTopicWord`, `iterBlockReceiptLogs`, topic filters); types `GetLogsFilter`, `RpcLogEntry`.

---

## Phase 4 — Tooling and language ergonomics

Goal: Shrink the gap to “write a contract in a high-level language.” Long-horizon; order depends on product bets.

- [ ] **T1** — **Assembler** for Boing bytecode with gas metadata (even if minimal).
- [ ] **T2** — **Source map / debug** hook between bytecode and line-level debugging in simulator or test harness.
- [ ] **T3** — Evaluate **subset transpiler** from a small DSL or restricted Solidity-like IR (only if VM semantics stabilize — tie to [EXECUTION-PARITY-TASK-LIST.md](EXECUTION-PARITY-TASK-LIST.md) Track V).

---

## Phase 5 — Ongoing VM / protocol depth

Continue incremental **Track V**-style work from the execution parity list when contracts need it: arithmetic helpers, precompiles (e.g. verified crypto primitives), stricter gas accounting, formal semantics notes.

Each item: spec → `boing-execution` → `boing-qa` → `TECHNICAL-SPECIFICATION.md`.

---

## Suggested first sprint (start the roadmap)

1. ~~**P1–P2** (spec + builder in `boing-sdk`)~~ — shipped; run `npm test` in `boing-sdk`.
2. ~~**W1** (dApp integration guide)~~ — shipped.
3. ~~**P4** + **E1**~~ — shipped (`submitFlow`, tutorial package).
4. **E2** + **W3** (optional) + **boing.express** W2 follow-through — next.

Review weekly: update the **matrix** statuses and checkboxes above.

---

## References

| Doc / code | Role |
|------------|------|
| [EXECUTION-PARITY-TASK-LIST.md](EXECUTION-PARITY-TASK-LIST.md) | VM, receipts, RPC, QA crate tasks |
| [BUILD-ROADMAP.md](BUILD-ROADMAP.md) | Historical L1 implementation phases |
| [BOING-EXPRESS-WALLET.md](BOING-EXPRESS-WALLET.md) | Injected provider methods, signing |
| [RPC-API-SPEC.md](RPC-API-SPEC.md) | JSON-RPC |
| [TECHNICAL-SPECIFICATION.md](TECHNICAL-SPECIFICATION.md) | VM, txs, gas |
| [boing-sdk/README.md](../boing-sdk/README.md) | TypeScript client |
| [BOING-PATTERN-AMM-LIQUIDITY.md](BOING-PATTERN-AMM-LIQUIDITY.md), [BOING-PATTERN-ORACLE-PRICE-FEEDS.md](BOING-PATTERN-ORACLE-PRICE-FEEDS.md), [BOING-PATTERN-UPGRADE-PROXY.md](BOING-PATTERN-UPGRADE-PROXY.md) | Phase 2 app patterns |
| [INDEXER-RECEIPT-AND-LOG-INGESTION.md](INDEXER-RECEIPT-AND-LOG-INGESTION.md) | Phase 3 receipt/log indexing + `boing_getLogs` vs full replay |
| `crates/boing-primitives`, `crates/boing-execution` | Source of truth for encoding |

---

*Document version: 1.0 — introduced to coordinate full-stack Boing VM parity; edit in PRs alongside feature work.*
