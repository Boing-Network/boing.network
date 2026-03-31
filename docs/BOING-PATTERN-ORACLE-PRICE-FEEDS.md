# Pattern: Oracle / on-chain price feeds (app layer)

**Roadmap:** [BOING-VM-CAPABILITY-PARITY-ROADMAP.md](BOING-VM-CAPABILITY-PARITY-ROADMAP.md) track **F4**.

Boing L1 does **not** require a dedicated “oracle opcode.” Price and feed semantics are **contracts + conventions**, optionally fed by **multisig**, **governance**, or **TWAP** math over AMM reserves ([BOING-PATTERN-AMM-LIQUIDITY.md](BOING-PATTERN-AMM-LIQUIDITY.md)).

---

## Design A — Multisig / committee feed

1. Deploy a **small contract** whose storage holds:
   - `price` (e.g. u128 fixed-point) and `last_update_block` or `timestamp` (from tx context if exposed, or monotonic nonce).
2. **`ContractCall`** `set_price` restricted to **`CALLER`** in a stored allowlist of **`AccountId`** (committee members).
3. Consumers (AMM, lending) **`SLOAD`** the feed contract in their swap/borrow path; declare the feed account in **access lists**.

**QA:** Usually **`dApp`** or **`tooling`**. Document signer set off-chain or in deploy metadata (`description_hash`).

---

## Design B — TWAP from AMM reserves

1. AMM contract logs **`LOG2`** (or similar) each swap with **reserves snapshot** and **height** (if available to contract) or sequential **counter**.
2. Off-chain indexers ([INDEXER-RECEIPT-AND-LOG-INGESTION.md](INDEXER-RECEIPT-AND-LOG-INGESTION.md)) compute TWAP; on-chain consumers can instead store **cumulative** sums in contract storage updated each block/swap (heavier but self-contained).

---

## Design C — Intent / delayed update

Align with any network **intent** or **automation** features when available: price updates as **signed intents** verified by a contract—still **`ContractCall`**-based.

---

## Manipulation resistance (guidance)

- Single-block spot prices are manipulable; prefer **TWAP**, **median of N sources**, or **committee caps** (`max_delta_bps` per update).
- Access lists must include **every** contract read for the feed and the AMM leg.

---

## References

- [RPC-API-SPEC.md](RPC-API-SPEC.md) — `boing_getContractStorage` for read helpers
- [QUALITY-ASSURANCE-NETWORK.md](QUALITY-ASSURANCE-NETWORK.md)
