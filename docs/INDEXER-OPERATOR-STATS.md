# Operator Stats Indexer & RPC — What You Need

> 👋 **Everyday users:** this is a specialist document. Start at [README.md](README.md) for user / developer / operator paths.
> 🛠️ **Developers:** keep this aligned with shipped code.
> 🛰️ **Operators:** treat the sections below as the working spec for this topic.


This document describes **what to build** so the Testnet Portal can show per-operator stats (blocks proposed, rank, uptime) on the [Operator Hub Leaderboard](https://boing.network/testnet/operators/leaderboard) and [My Dashboard](https://boing.network/testnet/operators/dashboard).

---

## Goal

- **Leaderboard:** List all operators ranked by blocks proposed (and optionally uptime).
- **My Dashboard:** For a given account ID, show *your* blocks proposed, your rank, and an uptime indicator.

The portal already has:

- **D1 database** with a `blocks` table: `(height, hash, parent_hash, proposer, tx_count, created_at)`. The `proposer` column is the 32-byte account ID (hex) of the validator who produced the block.
- **RPC** on the node: `boing_chainHeight`, `boing_getBlockByHeight`, etc. Block payload includes `header.proposer`.

You need **one** of the following (or both, with the indexer feeding the portal API).

---

## Option A: Indexer (recommended)

An **indexer** is a process that reads chain data from the node and writes it into a store (here, D1) so the portal can query it quickly.

### 1. Data flow

1. **Indexer** (Worker cron or external daemon) runs periodically (e.g. every 10–30 seconds).
2. It calls the **testnet node RPC**:
   - `boing_chainHeight` → current height.
   - For each new height (or range), `boing_getBlockByHeight(height)` → get block with `header.proposer`, `header.timestamp`, etc.
3. It **INSERTs** into D1 `blocks`:
   - `height`, `hash`, `parent_hash`, `proposer` (hex), `tx_count`, `created_at` (from block timestamp or now).

### 2. D1 schema (already in place)

```sql
CREATE TABLE IF NOT EXISTS blocks (
  height INTEGER PRIMARY KEY,
  hash TEXT NOT NULL,
  parent_hash TEXT,
  proposer TEXT NOT NULL,   -- 32-byte hex (0x...)
  tx_count INTEGER,
  created_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_blocks_proposer ON blocks(proposer);
```

Add the index if not present (see [schema.sql](../website/schema.sql)).

### 3. Portal API (implemented)

- **GET `/api/portal/operator-stats?account_id_hex=0x...`**  
  Returns: `{ ok, blocks_proposed, rank, total_operators, uptime_estimate }` by querying D1 `blocks` (e.g. `COUNT(*) WHERE proposer = ?`, and rank from `GROUP BY proposer ORDER BY COUNT(*) DESC`). If the indexer has not written any blocks yet, the API returns `blocks_proposed: null` and the dashboard shows "—".

- **GET `/api/portal/operator-leaderboard`**  
  Returns: `{ ok, operators: [ { rank, proposer, blocks_proposed, first_block_at, last_block_at }, ... ], total_operators, total_blocks }` for the Leaderboard page. The Leaderboard page fetches this on load and renders the table (or a placeholder when empty).

### 4. What you need to build

| Component | What to build |
|-----------|----------------|
| **Indexer** | A scheduled job (e.g. Cloudflare Worker with cron, or a small daemon) that: (1) reads `boing_chainHeight` from the public testnet RPC; (2) for each new height, fetches the block via `boing_getBlockByHeight`; (3) normalizes `proposer` to 32-byte hex; (4) INSERTs into D1 `blocks` (ignore duplicates by height). |
| **RPC URL** | Use the same testnet RPC as the rest of the portal (e.g. `PUBLIC_TESTNET_RPC_URL` or `https://testnet-rpc.boing.network/`). |
| **D1 binding** | The indexer must have write access to the same D1 database as the portal (`boing-network-db`). For a Worker, bind D1 in `wrangler.toml` and run the indexer in a cron trigger. |

### 5. Uptime (optional)

- **Simple:** From `blocks` table, per proposer: `MIN(created_at)` and `MAX(created_at)` → "Active from X to Y" or "First block / last block".
- **Richer:** A separate table or job that records "last seen block by proposer" and computes a time-window-based uptime (e.g. "produced at least one block in the last 24h").

---

## Option B: RPC on the node

Alternatively, add a **new RPC method** on the node that computes operator stats from chain state (no D1).

### 1. New method

- **`boing_getValidatorStats`** (or `boing_getProposerCounts`)  
  Params: `[]` or `[account_id_hex]`.  
  Returns: For each validator (or the requested one): `account_id_hex`, `blocks_proposed`, optionally `rank` and `uptime`.

### 2. Implementation

- The node has access to the chain (blocks in memory or storage). Iterate from genesis to tip (or over a sliding window), count blocks per `header.proposer`, then rank.
- Expose this via the existing JSON-RPC server so the portal (or any client) can call it.

### 3. Portal

- **Portal API** can proxy to the node: e.g. GET `/api/portal/operator-stats?account_id_hex=...` calls the testnet node’s `boing_getValidatorStats([account_id_hex])` and returns the result. No indexer or D1 blocks table required for stats (D1 may still be used for portal_registrations, etc.).

---

## Recommendation

- **Short term:** Implement **Option A** (indexer) so the existing D1 `blocks` table is populated. Then the existing portal API can read from D1 and the Leaderboard + My Dashboard show real data.
- **Long term:** If the node gains a `boing_getValidatorStats`-style method (Option B), the portal can optionally use that instead of or in addition to D1 for stats.

---

## Files to touch

| Area | File(s) |
|------|---------|
| D1 schema | `website/schema.sql` — add `CREATE INDEX IF NOT EXISTS idx_blocks_proposer ON blocks(proposer);` if missing. |
| Portal API | `website/functions/api/portal/operator-stats.js` — GET handler that reads from D1 `blocks` and returns per-account stats and rank. `website/functions/api/portal/operator-leaderboard.js` — GET handler that returns ranked list for the Leaderboard page. |
| Indexer | New Worker or cron (e.g. `website/functions/cron/index-blocks.js` or a separate repo) that calls RPC and writes to D1. |
| Node (Option B only) | `crates/boing-node` (or RPC layer) — add `boing_getValidatorStats` / `boing_getProposerCounts`. |

Once the indexer is running and writing blocks to D1, the portal’s operator-stats API will return real numbers and the Operator Hub Leaderboard and My Dashboard will show them automatically.
