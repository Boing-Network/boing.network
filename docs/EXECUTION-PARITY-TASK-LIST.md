# Execution parity — code task list

> 👋 **Everyday users:** this is a specialist document. Start at [README.md](README.md) for user / developer / operator paths.
> 🛠️ **Developers:** keep this aligned with shipped code.
> 🛰️ **Operators:** treat the sections below as the working spec for this topic.


**Why “multi-year” came up before:** That label applies to matching **every** external ecosystem toolchain and surface area (multiple foreign VMs, wallets, indexers, audits at once). That is not the same as **shipping useful Boing-native features** on the schedule below.

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

## Track V — Boing VM opcodes & gas (audit-first)

Goal: Expand the Boing VM **incrementally**; each batch is reviewable.

- [x] **V1** — **Integer compare / logic (batch 1):** `LT` (0x10), `GT` (0x11), `EQ` (0x14), `ISZERO` (0x15), `AND`/`OR`/`XOR`/`NOT` (0x16–0x19). Updated `bytecode.rs`, `interpreter.rs`, `gas`.
- [x] **V2** — **Division (batch 2):** `DIV` (`0x04`), `MOD` (`0x06`) — unsigned 256-bit; divisor zero → `VmError::DivisionByZero` (Boing VM; see `TECHNICAL-SPECIFICATION.md` §7).
- [x] **V3** — **More arithmetic (optional batch):** e.g. `ADDMOD`, `MULMOD` if needed by contracts; same spec + QA updates.
- [x] **V4** — Update **`boing-qa`** static bytecode walk (valid opcodes, jump targets) for all new opcode bytes.
- [x] **V5** — Update `docs/TECHNICAL-SPECIFICATION.md` §7 and `docs/QUALITY-ASSURANCE-NETWORK.md` opcode list.
- [x] **V6** — VM unit tests for `LT` + `ISZERO` plus **compare/bitwise matrix** (`LT`/`GT`/`EQ`/`ISZERO`/`AND`/`OR`/`XOR`/`NOT` small-value coverage); **proptest** over arbitrary 256-bit words in `crates/boing-execution/tests/proptest_compare_bitwise.rs`.
- [x] **V7** — **Bit shifts:** `Shl` (`0x1b`), `Shr` (`0x1c`), `Sar` (`0x1d`) — stack top = shift word, second = value; effective count ≡ shift (mod 256) via big-endian low byte. `boing-qa` whitelist, `TECHNICAL-SPECIFICATION.md` §7.2, `tools/boing-vm-assemble.mjs` / `boing-vm-transpile-ir.mjs`, proptest `proptest_shift.rs`, interpreter smoke + `BigInt` SAR encoding (`to_signed_bytes_be`).
- [x] **V8** — **Nested `Call` (`0xf1`):** sub-call with merged logs, memory return buffer, remaining-gas budget, depth limit (`MAX_CALL_DEPTH`), `StorageAccess::get_contract_code`; `boing-qa` whitelist + `TECHNICAL-SPECIFICATION.md` §7.2; `tools/boing-vm-assemble.mjs` / `boing-vm-transpile-ir.mjs`.
- [x] **V9** — **`Mul` (`0x03`) full 256×256 → 256:** interpreter matches **Div** / **MulMod** width (low **256** bits of product); fixes prior low-**64** operand truncation. Unit + **`proptest_mul.rs`**; `TECHNICAL-SPECIFICATION.md` §7.2; native CP pool fee step documented in [NATIVE-AMM-CALLDATA.md](NATIVE-AMM-CALLDATA.md).

---

## Track C — Execution context (caller / contract identity)

Goal: Contracts can implement patterns that need **who called** and **current code address** (same roles as `Caller` / `Address` opcodes in the Boing VM).

- [x] **C1** — **Boing semantics:** At **top-level** `ContractCall`, **`Caller`** = `tx.sender`; **`Address`** = executing contract. **Nested `Call` (`0xf1`):** inner frame **`Caller`** = the contract that issued `Call` (parent’s **`Address`**). See `TECHNICAL-SPECIFICATION.md` §7.2. No native “value” field on calls yet (balances move only via host / other tx types).
- [x] **C2** — Implemented: `Interpreter::run(caller_id, contract_id, …)`; opcodes `Caller` (`0x33`), `Address` (`0x30`).
- [x] **C3** — Gas + `boing-qa` whitelist updated.
- [x] **C4** — `TECHNICAL-SPECIFICATION.md` §7.2; reference token doc for wallet calldata.
- [x] **C5** — Nested **`Call`** wiring: `Interpreter::run_nested`, `VmError::{CallDepthExceeded, CallBufferTooLarge}`; `StateStore` implements `get_contract_code` for sub-calls.
- [x] **C6** — **Canonical reference fungible template bytecode:** implement balances + `transfer` / `mint_first` (or documented subset) in `boing-execution`, `check_contract_deploy_full` **Allow/Unsure** for purpose **`token`**, export hex from `dump_reference_token_artifacts`, pin default in **`boing-sdk`** and [BOING-CANONICAL-DEPLOY-ARTIFACTS.md](BOING-CANONICAL-DEPLOY-ARTIFACTS.md). *(Partner UIs: [E2-PARTNER-APP-NATIVE-BOING.md](E2-PARTNER-APP-NATIVE-BOING.md).)*
- [x] **C6b** — **Secured reference fungible:** `reference_fungible_secured` (`0xFD` init + runtime) with optional denylist, pause, caps, anti-bot, cooldown, transfer-unlock, no-mint, admin ops; **fourth** stdout line from `dump_reference_token_artifacts`; pinned default + resolvers in **`boing-sdk`**; VM **deploy-init gas** budget for large init; docs in [BOING-REFERENCE-TOKEN.md](BOING-REFERENCE-TOKEN.md) / [BOING-CANONICAL-DEPLOY-ARTIFACTS.md](BOING-CANONICAL-DEPLOY-ARTIFACTS.md).

---

## Track L — Logs / events (optional, receipt sub-feature)

Goal: Small, bounded **event blobs** for indexers (no heavy log-bloom filter layer unless justified).

- [x] **L1** — Caps: 4 topics × 32 bytes, 1024 bytes data per log, 24 logs per tx (`boing-primitives` constants).
- [x] **L2** — Opcodes `LOG0`..`LOG4` (`0xa0`..`0xa4`).
- [x] **L3** — `ExecutionReceipt.logs`; RPC receipts + `boing_simulateTransaction` include `logs`; bincode shape **breaking** for old receipt files.
- [x] **L4** — **`boing_getLogs`** for filtered log queries (see **R10**; caps in spec).
- [x] **L5** — **Init-code deploys** (`0xFD` prefix on deploy bytecode, `CONTRACT_DEPLOY_INIT_CODE_MARKER` in `boing_primitives`): VM runs init once at deploy; receipt logs + `boing_getLogs` attribution (deploy-derived address); legacy unprefixed deploys unchanged. Spec: **`docs/TECHNICAL-SPECIFICATION.md`** §4.4; indexer: **`INDEXER-RECEIPT-AND-LOG-INGESTION.md`**; RPC filter wording **`RPC-API-SPEC.md`**.

*Dependency:* best done after **R2–R4** minimum.

---

## Track X — RPC: commitment / finality (explicit head vs finalized)

Goal: Honest **finality** wording for BFT (not “instant finality” lies).

- [x] **X1** — Document in `RPC-API-SPEC.md`: what `boing_chainHeight` means vs **safe/finalized** height (define terms for HotStuff / your implementation).
- [x] **X2** — Optional RPC: `boing_getFinalizedHeight` or `boing_getSyncState` returning `{ head, finalized, … }`.
- [x] **X3** — Observer / SDK: display finalized vs pending if exposed.

---

## Track A — Access lists & parallelism (already partially there)

Goal: Make **explicit account touches** (read/write lists) a first-class dev experience for safe parallel execution hints.

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

## Track D — Deterministic deploy addresses (salt-derived)

Goal: Predictable contract addresses from deployer + salt + bytecode hash, without importing a foreign VM.

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

For **SDK, wallet, indexer, and dApp-facing capability planning** (not only crate work), see [BOING-VM-CAPABILITY-PARITY-ROADMAP.md](BOING-VM-CAPABILITY-PARITY-ROADMAP.md) — comparisons there are **illustrative**; behavior is always Boing-defined ([BOING-VM-INDEPENDENCE.md](BOING-VM-INDEPENDENCE.md)). **Tooling IR (T3):** [BOING-MINI-IR.md](BOING-MINI-IR.md) + `tools/boing-vm-transpile-ir.mjs`.

For **backlog themes** after checked-off tracks (indexer scale, NATIVE-AMM follow-ups, testnet/ops pointers), see [NEXT-STEPS-FUTURE-WORK.md](NEXT-STEPS-FUTURE-WORK.md).

For **native AMM** (Boing VM pools → wallets → boing.finance), see [NATIVE-AMM-INTEGRATION-CHECKLIST.md](NATIVE-AMM-INTEGRATION-CHECKLIST.md). **Canonical testnet pool id** is published in [RPC-API-SPEC.md](RPC-API-SPEC.md) / [TESTNET.md](TESTNET.md) §5.3 and as **`CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX`** in **`boing-sdk`**; **future** rotations: [OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md](OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md).

| Area | Location |
|------|----------|
| Opcodes today | `crates/boing-execution/src/bytecode.rs` |
| VM loop | `crates/boing-execution/src/interpreter.rs` |
| Tx / block | `crates/boing-primitives/src/types.rs` |
| RPC | `crates/boing-node/src/rpc.rs` |
| QA static rules | `crates/boing-qa/` |
| Pillars doc | `docs/QUALITY-ASSURANCE-NETWORK.md` |
