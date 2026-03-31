# Pattern: AMM / liquidity on Boing VM (constant product)

**Roadmap:** [BOING-VM-CAPABILITY-PARITY-ROADMAP.md](BOING-VM-CAPABILITY-PARITY-ROADMAP.md) track **F1**.

This document specifies an **application-level** pattern for a minimal **constant-product** style AMM (`x * y = k`) on Boing. It is **not** a consensus change: one or more **contracts** hold reserves in VM storage and expose **`ContractCall`** entrypoints. Deployments must still pass **protocol QA**; declare purpose **`dApp`**, **`tooling`**, or another **valid** category from your governance registry ([QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md)).

---

## Goals

- Match **EVM/Solana–class** “swap pool” behavior **without** EVM.
- Rely on existing VM features: **`SLOAD`/`SSTORE`**, **`CALLER`**, **`ADDRESS`**, arithmetic opcodes, **`LOG*`** for indexers.
- Make **access lists** explicit: every swap touches **pool contract + both reserve ledgers** (or a single contract that tracks both token balances in storage).

---

## Data model (recommended)

**Single contract (simpler QA story):**

- Storage slot layout (example; exact encoding is contract-defined):
  - `reserve_a`, `reserve_b` — `u128` each (two 32-byte words or packed per your convention).
  - Optional: `token_a_contract`, `token_b_contract` — `AccountId` of **reference-token** contracts if liquidity is tokenized; or treat A/B as **native BOING** vs **in-contract ledger** depending on product.

**Two-token reference pattern:** If both legs are [BOING-REFERENCE-TOKEN](BOING-REFERENCE-TOKEN.md) contracts, swaps are implemented as **contract calls** from the pool to each token’s `transfer` / `mint` / `burn` semantics you define—each hop must appear in the **access list** (pool + both tokens + traders as appropriate).

---

## Constant product math

- Invariant: \( R_a \cdot R_b = k \) (up to rounding).
- Swap `da` for `db`: compute `db` from new `R_a' = R_a + da` (less fee) and `R_b' = k / R_a'`, then `db = R_b - R_b'`.
- Implement in VM bytecode with **checked** arithmetic; document **fee** (e.g. 30 bps) in contract ABI notes.

---

## Entrypoints (calldata convention)

Reuse the **96-byte word** style from reference token/NFT for wallet consistency, or document a **custom** layout in your deploy metadata. Example logical methods:

| Method | Role |
|--------|------|
| `add_liquidity` | Pull both assets into pool storage, update LP accounting (or NFT LP receipt). |
| `remove_liquidity` | Burn LP, return reserves pro-rata. |
| `swap` | User specifies asset in, min out; contract updates reserves and emits **`LOG1`** with pool id + amounts for indexers. |

---

## Access lists

- **Swap tx:** at minimum **`read`/`write`** on: sender, pool contract, any token contracts moved, and **any account** read for oracle checks if you add them ([BOING-PATTERN-ORACLE-PRICE-FEEDS.md](BOING-PATTERN-ORACLE-PRICE-FEEDS.md)).
- Use **`boing_simulateTransaction`** + **`suggested_access_list`** during integration; widen with `mergeAccessListWithSimulation` in **`boing-sdk`**.

---

## QA category alignment

- Prefer **`dApp`** or **`tooling`** for generic pools; **`token`** only if the deployed bytecode is primarily a **single** fungible asset, not a full AMM.
- Pre-flight **`boing_qaCheck`** with the same bytecode and category you will submit.

---

## Security notes (non-exhaustive)

- Reentrancy-style issues: Boing VM is not EVM; still **order** state updates and external calls carefully.
- Rounding: favor the pool on rounding to avoid drain.
- Oracle manipulation: if using an on-chain oracle pattern, see the oracle doc; TWAP and multi-block windows are **app-layer** concerns.

---

## Reference implementation (MVP)

- Rust: `crates/boing-execution/src/native_amm.rs` — `constant_product_pool_bytecode`, calldata encoders, unit tests.
- TypeScript: `boing-sdk/src/nativeAmm.ts`.

## References

- [TECHNICAL-SPECIFICATION.md](TECHNICAL-SPECIFICATION.md) — VM, gas, storage
- [BOING-REFERENCE-TOKEN.md](BOING-REFERENCE-TOKEN.md)
- [BOING-DAPP-INTEGRATION.md](BOING-DAPP-INTEGRATION.md)
