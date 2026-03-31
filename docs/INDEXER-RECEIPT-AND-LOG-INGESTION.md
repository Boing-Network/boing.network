# Indexer: receipt and log ingestion (I1–I3)

**Roadmap:** [BOING-VM-CAPABILITY-PARITY-ROADMAP.md](BOING-VM-CAPABILITY-PARITY-ROADMAP.md) tracks **I1** (this spec), **I2** (`boing_getLogs`), **I3** (`boing-sdk` helpers).

This spec is for **off-chain indexers** (Workers, daemons, explorer backends) that want **Solana/EVM-class** visibility into **execution results** and **events**. The **canonical replay path** remains **blocks + receipts**; **`boing_getLogs`** is an optional, bounded shortcut (see below).

---

## RPC sources

| Method | Use |
|--------|-----|
| `boing_chainHeight` | Tip height |
| `boing_getBlockByHeight(height, true)` | Block + aligned **`receipts[]`** (same order as `transactions`) — **primary** source for full history and idempotent upserts |
| `boing_getTransactionReceipt(tx_id)` | Single receipt when you already know **`tx_id`** (32-byte hex, BLAKE3 tx id) |
| `boing_getLogs` | **Optional:** filtered log rows across up to **128** inclusive heights and **2048** rows per call ([RPC-API-SPEC.md](RPC-API-SPEC.md)); use for targeted backfill, debugging, or thin pipelines — **not** a replacement for full block replay at scale |

Receipt shape (see [RPC-API-SPEC.md](RPC-API-SPEC.md)):

```json
{
  "tx_id": "0x...",
  "block_height": 123,
  "tx_index": 0,
  "success": true,
  "gas_used": 21000,
  "return_data": "0x",
  "logs": [{ "topics": ["0x..."], "data": "0x" }],
  "error": null
}
```

- **`logs`:** Boing VM **`LOG0`–`LOG4`** — topics are 32-byte words hex; `data` is arbitrary hex (bounded per protocol).

---

## Ingestion loop (recommended)

1. Persist **`last_indexed_height`** (and optional **`last_indexed_hash`** for reorg detection).
2. Each tick: `H = boing_chainHeight`. For `h` in `(last_indexed_height, H]`:
   - Fetch `boing_getBlockByHeight(h, true)`.
   - If `receipts[i]` is non-null, **upsert** by **`tx_id`** primary key with block metadata.
   - If `null` (old node / missing file), optionally skip or backfill later.
3. **Idempotency:** `INSERT OR REPLACE` / `ON CONFLICT(tx_id) DO UPDATE` on **`tx_id`**.

---

## Log indexing

- **Denormalize** logs into a child table: `(tx_id, log_index, topic0, topic1, …, data, block_height, contract_id)`.
- **Topic0** convention: many systems use a **discriminant** word; Boing has no enforced ABI—index **raw** topics and decode in app-specific workers (AMM, NFT).
- **Contract id for logs:** When ingesting from **`boing_getBlockByHeight(…, true)`** or **`boing_getTransactionReceipt`**, derive the emitting contract from the **parent transaction** (`ContractCall.contract`, or deploy-derived / CREATE2 address for deploy payloads). Rows from **`boing_getLogs`** include an **`address`** field when the node can attribute the log (same rules); use it to skip payload decoding for those rows.
- **SDK:** Normalization and receipt-only helpers live in **`boing-sdk`** (`receiptLogs.ts`, `BoingClient.getLogs`) — see [BOING-VM-CAPABILITY-PARITY-ROADMAP.md](BOING-VM-CAPABILITY-PARITY-ROADMAP.md) **I3**.

---

## Reorgs

- Until the node exposes **finalized** height semantics ([RPC-API-SPEC.md](RPC-API-SPEC.md) / `boing_getSyncState`), indexers should:
  - Prefer **`finalized_height`** when available for **durable** index; or
  - Keep a **short** tail window and **rewind** if `parent_hash` breaks the chain.

---

## Volume, filters, and `boing_getLogs` vs replay

- **Full index / explorer backends** should still drive the main loop from **`boing_getBlockByHeight(h, true)`** (or archive replay): you get **transactions + receipts** in one consistent snapshot per height, with straightforward idempotency on **`tx_id`**.
- **`boing_getLogs`** is useful when you already know **height bounds** and optional **`address` / `topics`** filters—for example incremental “events only” workers, operator dashboards, or narrowing a bug search. Respect node caps (**128** blocks, **2048** logs per request); page the chain in **≤128-height windows** if you use it for wider ranges. On **`429` / rate limits**, back off; prefer batch **block+receipt** fetches for heavy catch-up.
- **`boing_getContractStorage`** can backfill **state** for known layouts ([BOING-REFERENCE-TOKEN.md](BOING-REFERENCE-TOKEN.md), [BOING-REFERENCE-NFT.md](BOING-REFERENCE-NFT.md)).

---

## Next steps (outside this doc)

| Item | Where |
|------|--------|
| **Wallet RPC error fidelity** | **boing.express:** preserve JSON-RPC **`code`** and **`data`** when proxying the node (roadmap **W2**). |
| **Public RPC policy** | See [RUNBOOK.md](RUNBOOK.md) § **Public RPC operators and `boing_getLogs`**. |
| **Constructor logs on deploy** | Today the VM **does not** execute deploy bytecode at deploy time for log emission; logs appear on **`ContractCall`**. Tracked as **L5** in [EXECUTION-PARITY-TASK-LIST.md](EXECUTION-PARITY-TASK-LIST.md). |

---

## Relation to operator stats indexer

[INDEXER-OPERATOR-STATS.md](INDEXER-OPERATOR-STATS.md) focuses on **proposer** counts. This doc focuses on **execution receipts and logs**. A single Worker can do **both** in one pass over `boing_getBlockByHeight`.

---

## References

- [BOING-OBSERVER-AND-EXPRESS.md](BOING-OBSERVER-AND-EXPRESS.md) — explorer RPC table
- [EXECUTION-PARITY-TASK-LIST.md](EXECUTION-PARITY-TASK-LIST.md) — Track R / L
- [RPC-API-SPEC.md](RPC-API-SPEC.md) — `boing_getLogs` params, caps, errors
