# Handoff: Boing Network — decentralized token & pool discovery

**Audience:** Boing Network core / RPC / indexer maintainers  
**Consumer:** `boing.finance` frontend, `boing-sdk`, third-party wallets  
**Goal:** Replace ad-hoc “global token list” expectations with a **verifiable, chain-derived discovery surface** that any client can call without trusting a single operator-hosted database.

---

## 1. Problem statement

- EVM chains have **no native “list all ERC-20s” RPC**. Wallets and dapps must **scan logs**, **read factory state**, or use an **indexer**.
- Boing L1 uses **32-byte VM account ids** for tokens/pools, not `0x40`-byte EVM addresses. The dapp already hydrates **CP pool venues** from RPC and optionally merges a **remote indexer JSON** (`REACT_APP_BOING_NATIVE_DEX_INDEXER_STATS_URL` or user override in `localStorage`).
- Product asks for **“auto-recognition”** and **“global all tokens API”**: that must be defined as **a documented Boing RPC + pagination contract**, not magic—otherwise every client reimplements fragile scans.

---

## 2. Recommended architecture (layers)

| Layer | Responsibility | Trust model |
|--------|-----------------|-------------|
| **L0 — Consensus / state** | Source of truth: factory registry, pool accounts, token ledger metadata | Trust the chain |
| **L1 — Boing JSON-RPC** | Cheap **cursor-paginated** read methods (below) | Trust your RPC node (or run your own) |
| **L2 — Optional indexer** | Pre-aggregated stats, USD, history, search | Trust indexer *or* self-host / verify against L0–L1 |

The dapp will **prefer L0/L1** for “what exists,” and **L2** for enrichment—same pattern as Uniswap (subgraph + contract reads).

---

## 3. Proposed Boing RPC methods (L1)

Implement on the **Boing JSON-RPC** namespace (exact naming is your convention; below uses `boing_` prefix as in existing `boing_getNetworkInfo`, etc.).

### 3.1 `boing_listDexPools` (paginated)

**Purpose:** Enumerate all constant-product pools registered with the native DEX factory (or equivalent), without scanning every block on the client.

**Parameters (JSON object):**

- `cursor` (optional, string): opaque cursor from previous page; omit for first page.
- `limit` (optional, number): default `100`, max `500`.

**Returns:**

```json
{
  "pools": [
    {
      "poolHex": "0x…64hex…",
      "tokenAHex": "0x…64hex…",
      "tokenBHex": "0x…64hex…",
      "feeBps": 30,
      "reserveA": "…decimal string…",
      "reserveB": "…decimal string…",
      "createdAtHeight": 12345
    }
  ],
  "nextCursor": "opaque-or-null"
}
```

**Why:** Unifies swap / route / pool UIs: **any token appearing in a pool is “discoverable”** by definition.

### 3.2 `boing_listDexTokens` (derived “global token set”)

**Purpose:** This is the practical answer when the product asks for a **“global all tokens API”** for Boing Finance: **not** every random ledger account on the network (which is neither definable nor desirable), but a **canonical, paginated universe of trade-relevant tokens** — i.e. **every token id that appears in any registered DEX pool** (plus optional extensions below). That is the same *spirit* as how major EVM DEX UIs scope “the world”: pools first, not infinite ERC-20s.

**Parameters:**

- `cursor`, `limit` as above.
- Optional `minReserveProduct` (string) or `minLiquidityWei` to filter dust/spam at the node.

**Returns:**

```json
{
  "tokens": [
    {
      "id": "0x…64hex…",
      "symbol": "TKN",
      "name": "Example",
      "decimals": 18,
      "poolCount": 2,
      "firstSeenHeight": 12000
    }
  ],
  "nextCursor": null
}
```

**Implementation note:** This can be implemented as a **materialized view** in the node, **derived from factory + token metadata reads**, refreshed on new blocks—clients then get O(1) pages instead of N RPC round-trips.

### 3.3 `boing_getDexToken` (single lookup)

**Parameters:** `{ "id": "0x…64hex…" }`  
**Returns:** same row shape as one element of `tokens`, or `null` if unknown / not in universe.

---

## 4. Indexer / HTTP mirror (L2) — contract for `boing.finance`

The frontend already fetches optional JSON (see `frontend/src/services/nativeDexIndexerClient.js`). **Align the indexer output** with the RPC shapes above so clients can swap **RPC vs HTTPS** without rewriting parsers.

**Minimum fields** (existing / extended):

- `updatedAt` (ISO string)
- `pools[]` with `poolHex`, `tokenAHex`, `tokenBHex`, reserves if available
- `tokenDirectory[]` with `id` (64 hex), `symbol`, `name` (optional `decimals`)

**Versioning:** add `schemaVersion: 1` at the root so consumers can evolve.

---

## 5. Security & spam

- **Do not** imply “safe” from “listed.” UI should treat discovery as **existence**, not endorsement.
- Node-side filters: **minimum liquidity**, **minimum pool age**, optional **fee burn** for registry listing.
- Optional **signed token list** (IPFS + EIP-712) as a **parallel** channel for curated metadata.

---

## 6. Acceptance criteria (for Network team)

1. **Pagination:** stable ordering (e.g. by `id` or `poolHex`), deterministic `nextCursor`.
2. **Load:** p95 response under agreed budget for `limit=100` on mainnet-scale data.
3. **Docs:** OpenRPC / Markdown spec published next to other `boing_*` methods.
4. **SDK:** `boing-sdk` exposes typed helpers (`listDexPoolsPage`, `listDexTokensPage`) wrapping the RPC.
5. **Test vectors:** fixture JSON for indexer + golden RPC responses in CI.

---

## 7. What `boing.finance` implemented on the client (this repo)

Until L1 RPC exists end-to-end:

- **EVM (Sepolia / any chain with `dexFactory`):** client scans **`PairCreated`** logs on the configured factory (chunked `eth_getLogs`) and merges those token addresses into Swap / Create Pool pickers (including **zero-balance** tokens that already have pools).
- **Boing native:** token picker merge order prefers **pooled venues + indexer + env list** over static defaults; users can set an **indexer stats URL override** in the UI (stored in `localStorage`) and refresh integration.
- **Indexer URL resolution:** `resolveNativeDexIndexerStatsUrl()` in `frontend/src/services/nativeDexIndexerClient.js` — **localStorage override** key `boing_native_dex_indexer_stats_url_override_v1` → then build-time env `REACT_APP_BOING_NATIVE_DEX_INDEXER_STATS_URL`. UI: **Boing native trade** section → “Discovery: custom indexer URL”.
- **Indexer JSON:** `extractTokenDirectoryFromIndexer` accepts either `tokenDirectory` or **`tokens`** at the payload root (forward-compatible with the proposed RPC naming).

---

## 8. Contacts / follow-ups

- Frontend expects **64-char hex** token ids (with `0x`) for native VM tokens.
- Coordinate **event / account layouts** with whoever owns the native DEX factory & pool account schemas so `boing_listDexPools` never drifts from `hydrateCpPoolVenuesFromRpc` reality.

---

*Document generated for cross-repo handoff; keep in sync when RPC names or fields ship.*

### Implementation note (boing.network)

- **`params.factory`** overrides **`BOING_CANONICAL_NATIVE_DEX_FACTORY`** on **`boing_listDexPools`**, **`boing_listDexTokens`**, **`boing_getDexToken`**.
- **`createdAtHeight` / `firstSeenHeight`:** filled from committed receipts (factory **`Log3`**) when **`light`** is not set; **`light: true`** or **`enrich: false`** skips scans.
- **Token labels:** recent blocks (env **`BOING_DEX_TOKEN_METADATA_SCAN_BLOCKS`**, default **8192**) are scanned newest-first for **`ContractDeployWithPurposeAndMetadata`**; response includes **`metadataSource`** (`deploy` \| `abbrev`).
- **Receipt scan cap:** env **`BOING_DEX_DISCOVERY_MAX_RECEIPT_SCANS`** (default **500000** per RPC call; **`0`** = unlimited). **`includeDiagnostics: true`** returns counters (**`receiptScans`**, **`receiptScanCapped`**, deploy scan stats on token methods).
- **Decimals override:** env **`BOING_DEX_TOKEN_DECIMALS_JSON`** — JSON object of **`"0x" + 64 hex` → number** for **`boing_listDexPools`** (**`tokenADecimals`** / **`tokenBDecimals`** per leg), **`boing_listDexTokens`**, and **`boing_getDexToken`** (default **18** when omitted).
- **Indexer / SDK:** **`buildNativeDexIndexerStatsForClient`** merges **`createdAtHeight`**, **`tokenADecimals`**, and **`tokenBDecimals`** from **`boing_listDexPools`** per canonical factory when RPC supports it.

**Cross-repo handoff** for **`boing.finance`**, **`boing.observer`**, **`boing.express`**: § **Consumer apps** below.

---

## Consumer apps (`boing.finance`, `boing.observer`, `boing.express`)

**SDK:** **`boing-sdk@^0.3.1`** — `listDexPoolsPage`, `listDexTokensPage`, `getDexToken`.

### Integration checklist (all three)

1. **Node / RPC** — Target JSON-RPC implements **`boing_listDexPools`**, **`boing_listDexTokens`**, **`boing_getDexToken`**. Operators set **`BOING_CANONICAL_NATIVE_DEX_FACTORY`** (live testnet: **`0x58112627…`**) or clients pass **`params.factory`**.
2. **Pagination** — Treat **`nextCursor`** as opaque.
3. **Fast path** — **`light: true`** when you only need directory + reserves.
4. **Trust UX** — Discovery = existence, not endorsement; use **`minReserveProduct`** / **`minLiquidityWei`** where appropriate.

### boing.finance

- Merge **`listDexTokensPage`** / **`listDexPoolsPage`** with indexer JSON by **`poolHex`**.
- Use per-leg **`tokenADecimals`** from pool rows; **`decimals`** on token rows.
- Probe **`boing_rpcSupportedMethods`** / **`preflightRpc`** for graceful degradation.

### boing.observer

- **`/dex/pools`**, **`/dex/tokens`**: factory from **`boing_getNetworkInfo.end_user.canonical_native_dex_factory`**.
- Account page: optional “In DEX universe” badge via **`boing_getDexToken`**.
- **`includeDiagnostics: true`** for ops dashboards only.

### boing.express

- Forward discovery JSON-RPC unchanged; document in OpenAPI / partner docs.
- Cache keys include **`factory` + cursor + limit + light`**; respect node rate limits.

### Quick reference

| Method | Role |
|--------|------|
| **`boing_listDexPools`** | Paginated pools + reserves + **`createdAtHeight`**. |
| **`boing_listDexTokens`** | DEX-derived token set (tokens in any listed pool). |
| **`boing_getDexToken`** | Single-row lookup or **`null`**. |

**Verify:** `cargo test -p boing-node --test native_dex_factory_rpc_happy_path`; **`npm test`** in **`boing-sdk/`**.
