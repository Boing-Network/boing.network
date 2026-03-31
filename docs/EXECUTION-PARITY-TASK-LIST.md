# Execution parity — code task list

**Why “multi-year” came up before:** That label applies to **full ecosystem parity** with EVM *and* Solana (Solidity toolchain, full opcode sets, precompiles, BPF programs, SPL, wallets, indexers, audits). That is not the same as **shipping useful Boing features** inspired by those chains.

**What is realistic with focused work:** Individual **tracks** below are on the order of **days to a few weeks** each (spec + implementation + tests + doc), depending on review and whether the change touches consensus/persistence. Several tracks can run in parallel if people split crates.

**Pillar rule:** Any new opcode, receipt field, or tx type must update **QA / docs** where applicable (`QUALITY-ASSURANCE-NETWORK.md`, `RPC-API-SPEC.md`, `boing-qa` static checks).

---

## How to use this list

- Work **top to bottom** within a track unless dependencies say otherwise.
- Check boxes in PRs (edit this file in the same PR as the feature, or follow your team’s habit).
- Prefer **small PRs**: e.g. “receipt type + persistence” before “full RPC.”

---

## Track R — Receipts & execution results (indexer-friendly summaries)

Goal: Every included tx has a **deterministic execution summary** clients and indexers can fetch (status, gas, return data, optional logs), in the same spirit as common chain receipts but **Boing-defined** types and RPC.

- [x] **R1** — Specify JSON + binary shape for `ExecutionReceipt` (or equivalent): `tx_id`, `block_height`, `tx_index`, `success`, `gas_used`, `return_data` (cap length, e.g. 24 KiB), optional `error`. Documented in `docs/RPC-API-SPEC.md` (logs deferred).
- [x] **R2** — Add Rust types in `crates/boing-primitives` (serde/bincode as needed); version if wire format evolves.
- [x] **R3** — During block application in `boing-node` / `boing-execution`, **produce** one receipt per tx (failed txs: receipt recorded then execution error returned — block still fails atomically today).
- [x] **R4** — Persist receipts in `crates/boing-node` persistence (`chain/blocks/receipts_{height}.bin`). Older nodes: missing file → no receipts until new blocks.
- [x] **R5** — Add `receipts_root` to `BlockHeader` (`boing-primitives`): Merkle over `BLAKE3(bincode(receipt))` per tx order (same tree shape as `tx_root`). **Breaking** for persisted `*.bin` blocks: reset chain data or re-bootstrap nodes that used the pre-root format.
- [x] **R6** — RPC: `boing_getTransactionReceipt` by tx id hex.
- [x] **R7** — RPC: extend `boing_getBlockByHeight` / `boing_getBlockByHash` with flag `include_receipts` (default false).
- [x] **R8** — Align `boing_simulateTransaction` response fields with receipt shape where possible (`return_data` hex).
- [x] **R9** — Integration tests: deploy + call + failed simulation → receipts persisted and returned over RPC.
- [x] **R10** — Optional bounded RPC **`boing_getLogs`** (block range + optional `address` / `topics`); documented in `RPC-API-SPEC.md`; indexer guidance in `INDEXER-RECEIPT-AND-LOG-INGESTION.md`.

---

## Track V — VM opcodes & gas (EVM-inspired, audit-first)

Goal: Expand the Boing VM **incrementally**; each batch is reviewable.

- [x] **V1** — **Integer compare / logic (batch 1):** `LT` (0x10), `GT` (0x11), `EQ` (0x14), `ISZERO` (0x15), `AND`/`OR`/`XOR`/`NOT` (0x16–0x19). Updated `bytecode.rs`, `interpreter.rs`, `gas`.
- [x] **V2** — **Division (batch 2):** `DIV` (`0x04`), `MOD` (`0x06`) — unsigned 256-bit; divisor zero → `VmError::DivisionByZero` (Boing VM; opcode bytes match EVM labels only where noted in spec).
- [x] **V3** — **More arithmetic (optional batch):** e.g. `ADDMOD`, `MULMOD` if needed by contracts; same spec + QA updates.
- [x] **V4** — Update **`boing-qa`** static bytecode walk (valid opcodes, jump targets) for all new opcode bytes.
- [x] **V5** — Update `docs/TECHNICAL-SPECIFICATION.md` §7 and `docs/QUALITY-ASSURANCE-NETWORK.md` opcode list.
- [x] **V6** — VM unit tests for `LT` + `ISZERO` plus **compare/bitwise matrix** (`LT`/`GT`/`EQ`/`ISZERO`/`AND`/`OR`/`XOR`/`NOT` small-value coverage); **proptest** over arbitrary 256-bit words in `crates/boing-execution/tests/proptest_compare_bitwise.rs`.

---

## Track C — Execution context (caller / contract identity)

Goal: Contracts can implement patterns that need **who called** and **current code address** (EVM `CALLER` / `ADDRESS` spirit).

- [x] **C1** — **Boing semantics:** `CALLER` = transaction signer (`tx.sender`); `ADDRESS` = contract account whose code is executing. No native “value” field on calls yet (balances move only via host / other tx types).
- [x] **C2** — Implemented: `Interpreter::run(caller_id, contract_id, …)`; opcodes `Caller` (`0x33`), `Address` (`0x30`).
- [x] **C3** — Gas + `boing-qa` whitelist updated.
- [x] **C4** — `TECHNICAL-SPECIFICATION.md` §7.2; reference token doc for wallet calldata.

---

## Track L — Logs / events (optional, receipt sub-feature)

Goal: Small, bounded **event blobs** for indexers (not full Ethereum log bloom unless justified).

- [x] **L1** — Caps: 4 topics × 32 bytes, 1024 bytes data per log, 24 logs per tx (`boing-primitives` constants).
- [x] **L2** — Opcodes `LOG0`..`LOG4` (`0xa0`..`0xa4`).
- [x] **L3** — `ExecutionReceipt.logs`; RPC receipts + `boing_simulateTransaction` include `logs`; bincode shape **breaking** for old receipt files.
- [x] **L4** — **`boing_getLogs`** for filtered log queries (see **R10**; caps in spec).
- [ ] **L5** (future) — If **deploy-time** bytecode execution ever emits logs (constructor / init run on `ContractDeploy`), update **`docs/RPC-API-SPEC.md`**, **`INDEXER-RECEIPT-AND-LOG-INGESTION.md`**, and **`boing_getLogs`** attribution in `rpc.rs` in the **same PR** (today logs on deploy txs are empty until the VM runs init code).

*Dependency:* best done after **R2–R4** minimum.

---

## Track X — RPC: commitment / finality (Solana-inspired clarity)

Goal: Honest **finality** wording for BFT (not “instant finality” lies).

- [x] **X1** — Document in `RPC-API-SPEC.md`: what `boing_chainHeight` means vs **safe/finalized** height (define terms for HotStuff / your implementation).
- [x] **X2** — Optional RPC: `boing_getFinalizedHeight` or `boing_getSyncState` returning `{ head, finalized, … }`.
- [x] **X3** — Observer / SDK: display finalized vs pending if exposed.

---

## Track A — Access lists & parallelism (already partially there)

Goal: Make **Solana-style explicit touches** a first-class dev experience.

- [x] **A1** — Document required `access_list` rules for `ContractCall` / deploy in `TECHNICAL-SPECIFICATION.md` (read vs write keys).
- [x] **A2** — RPC: `boing_simulateTransaction` returns **`suggested_access_list`** (heuristic) and **`access_list_covers_suggestion`** on success and failure.
- [x] **A3** — `boing-sdk`: `mergeAccessListWithSimulation`, `accessListFromSimulation`, `accountsFromSuggestedAccessList`, `simulationCoversSuggestedAccessList` (`accessList.ts`).

---

## Track T — Fungible / NFT standards (protocol or VM-only)

Goal: **Purpose + specs** QA for token-like deploys (`QUALITY-ASSURANCE-NETWORK.md` §5.2).

- [x] **T1** — **Decision:** **(b)** Contract bytecode + optional reference ABI; no new `TransactionPayload` for token ops in this iteration. Documented in `docs/BOING-REFERENCE-TOKEN.md`.
- [x] **T2** — Reference **calldata** layout (`transfer` / `mint_first` selectors) + Rust/SDK encoders; full token bytecode left to deployers (must pass QA).
- [x] **T3** — Minimal NFT standard (owner, transfer, optional metadata hash) + QA rules.
- [x] **T4** — RPC read helpers if needed (`boing_getTokenBalance` etc.) or rely on contract storage + explorer.

*Can start after **V** and **R** if contracts need richer VM; or **T** first if standards are contract-only on current VM.*

---

## Track D — Deterministic deploy addresses (CREATE2-style)

Goal: Predictable contract addresses without full EVM compatibility.

- [x] **D1** — Spec: salt + deployer + bytecode hash → `AccountId` scheme.
- [x] **D2** — Implement in deploy path; ensure **no collision** with Ed25519-derived accounts (namespace bit or prefix).
- [x] **D3** — QA: same bytecode + purpose rules apply.

---

## Suggested first sprint (example ~1–2 weeks of focused work)

1. **R1–R4, R6** — receipts end-to-end without header root (fastest indexer win).
2. **V1** — one opcode batch + QA + spec.
3. **X1** — documentation only (parallel).

Then: **R5** or **C** or **T1** depending on product priority.

---

## References

For **SDK, wallet, indexer, and dApp-facing parity** (not only crate work), see [BOING-VM-CAPABILITY-PARITY-ROADMAP.md](BOING-VM-CAPABILITY-PARITY-ROADMAP.md).

For **native AMM** (Boing VM pools → wallets → boing.finance), see [NATIVE-AMM-INTEGRATION-CHECKLIST.md](NATIVE-AMM-INTEGRATION-CHECKLIST.md).

| Area | Location |
|------|----------|
| Opcodes today | `crates/boing-execution/src/bytecode.rs` |
| VM loop | `crates/boing-execution/src/interpreter.rs` |
| Tx / block | `crates/boing-primitives/src/types.rs` |
| RPC | `crates/boing-node/src/rpc.rs` |
| QA static rules | `crates/boing-qa/` |
| Pillars doc | `docs/QUALITY-ASSURANCE-NETWORK.md` |
