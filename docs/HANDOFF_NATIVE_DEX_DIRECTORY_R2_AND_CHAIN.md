# Handoff: native DEX directory (D1), R2 uploads, and chain-side follow-ups

> 👋 **Everyday users:** pool lists in [boing.finance](https://boing.finance) and [boing.observer/dex/pools](https://boing.observer/dex/pools) read chain (and optionally this Worker). You do not operate it.  
> 🛠️ **Developers:** JSON routes `GET /v1/directory/meta` and `/v1/directory/pools`. SDK: `fetchNativeDexDirectoryMeta`, `collectAllNativeDexDirectoryPools`.  
> 🛰️ **Operators:** this file is the **source of truth** for the Worker (`workers/native-dex-indexer`). Bindings, secrets, snapshot limits, and chain follow-ups live here.

This document is for the **boing.network** engineering agent and protocol owners. It summarizes what **boing.finance + boing.network workers** now provide, what **you must configure** (secrets / variables), and what still requires **Boing node or protocol** work.

```mermaid
flowchart LR
  Node[boing-node RPC] --> Worker[native-dex-indexer]
  Worker --> D1[D1 directory]
  Worker --> R2[R2 manifests]
  D1 --> HTTP[GET /v1/directory/*]
  HTTP --> Apps[finance / observer / SDK]
```

---

## 1. What shipped in code (no action required to read)

### 1.1 Cloudflare Worker `boing-native-dex-indexer`

**Repo path:** `boing.network/workers/native-dex-indexer/`

**Bindings (see `wrangler.toml`):**

| Binding | Resource | Purpose |
|--------|----------|---------|
| `NATIVE_DEX_INDEXER_KV` | KV | Persisted indexer state key `native_dex_indexer_state_v1` (history, etc.) |
| `DIRECTORY_DB` | **D1** `boing-native-dex-directory` | **Server-side pool directory** for cursor pagination |
| `INDEXER_ARCHIVE_R2` | R2 (optional) | Tiny JSON sync manifests per successful directory sync |
| `BOING_TESTNET_RPC_URL` | var | RPC base URL |

**D1 database ID (production, ENAM):** `68eb37e6-f71c-47fa-9475-19d7a56b0db0`  
**Migrations:** `0001_directory_pools.sql` → `0002_directory_pool_events.sql` ( **`directory_pool_events`** includes **`caller_hex`** ) → **`0003_pool_events_caller_and_tip.sql`** ( **`directory_indexer_tip`** + caller index) → **`0004_indexer_tip_parent_hash.sql`** ( **`parent_block_hash`** on tip row) → **`0005_directory_nft_owner.sql`** (LP NFT owner snapshot) → **`0006_directory_receipt_log.sql`** (optional bounded execution-log archive). Apply in order on each D1 (`npx wrangler d1 migrations apply boing-native-dex-directory --remote` from `workers/native-dex-indexer/`).

**HTTP routes:**

| Method | Path | Description |
|--------|------|-------------|
| GET | `/` or `/stats` | Full indexer JSON (same shape as `REACT_APP_BOING_NATIVE_DEX_INDEXER_STATS_URL`). Optional `?pools_page=&pools_page_size=` (1–500) slices `pools[]` only. |
| GET | `/v1/directory/meta` | `{ api, poolCount, eventCount?, nftOwnerRowCount?, receiptLogCount?, latestSyncBatch, indexedTipHeight?, indexedTipBlockHash?, indexedParentBlockHash? }` — D1 + tip row (**`0003`+**). Tip / parent-hash fields are for **skew checks** vs chain; shallow reorg at tip clears pool-event + receipt snapshots (see Worker `invalidateDirectoryPoolEventsIfTipReorged`). |
| GET | `/v1/directory/pools?limit=&cursor=` | **Cursor pagination** over pools from the **last successful indexer sync** (`limit` default 20, max 100). `cursor` = previous page’s last `poolHex` (omit for first page). Response: `pools`, `nextCursor`, `hasMore`, `limit`, `cursor`. |
| GET | `/v1/history/pool/{pool_hex}/events?limit=&cursor=` | **Snapshot only:** parsed **swap / addLiquidity / removeLiquidity** events for that pool from the last sync, over the configured **`NATIVE_DEX_INDEXER_LOG_SCAN_BLOCKS`** window (same depth as indexer stats). **`cursor`** = numeric D1 row **`id`** from **`nextCursor`**; default `limit` 50, max 200. **Not** reorg-safe canonical history — see [PROTOCOL_NATIVE_DEX_RPC_AND_INDEXING_ROADMAP.md](PROTOCOL_NATIVE_DEX_RPC_AND_INDEXING_ROADMAP.md) §3. |
| GET | `/v1/history/user/{caller_hex}/events?limit=&cursor=` | Same snapshot window; events whose **`caller`** matches the account (native AMM **`Log2`** **`topic1`**). Not full receipt history. |
| GET | `/v1/history/receipts?limit=&cursor=&topic0=` | Optional **bounded** execution-log rows from **`getBlockByHeight(..., include_receipts: true)`** for the last **`NATIVE_DEX_INDEXER_RECEIPT_ARCHIVE_BLOCKS`** heights when that var is **> 0** and D1 **`0006`** is applied. **`topic0`** optional (`0x` + 64 hex). Same snapshot semantics as pool events; **not** full-chain archive. |
| GET | `/v1/lp/vault/{vault_hex}/mapping` | Live **`boing_getContractStorage`** via **`boing-sdk`** — **`configured`**, **`poolHex`**, **`shareTokenHex`**. |
| GET | `/v1/lp/positions?owner=` | Live RPC **model A** share rows for each vault in **`NATIVE_DEX_INDEXER_LP_VAULT_HEXES`** (comma-separated) or **`NATIVE_DEX_INDEXER_LP_VAULT_HEX`**, else canonical testnet vault from **`boing-sdk`**. |
| GET | `/v1/lp/nft/positions?owner=&contract=` | D1 **`directory_nft_owner`** rows when **`NATIVE_DEX_INDEXER_LP_NFT_CONTRACT_HEX`** is set (or **`contract`** query param). |
| POST | `/v1/directory/sync` | Runs full indexer build + **refreshes D1** (pools, pool events, NFT owners if configured, optional receipt archive, indexer tip row). Requires header `Authorization: Bearer <DIRECTORY_SYNC_SECRET>` and Worker secret `DIRECTORY_SYNC_SECRET` set. |
| OPTIONS | `*` | CORS preflight for directory endpoints. |

**Cron:** `*/15 * * * *` — runs `buildPayload` and, if `DIRECTORY_DB` is bound, **upserts D1** and deletes rows from older `sync_batch_id` values (full replace semantics per sync), including optional receipt rows when **`NATIVE_DEX_INDEXER_RECEIPT_ARCHIVE_BLOCKS` > 0**.

**Important:** Until the first cron run **or** a successful `POST /v1/directory/sync`, `poolCount` may be `0`. That is expected.

#### 1.1.1 Values worth keeping in runbooks / env docs

These are **not secrets**; save them where your team tracks infra (1Password notes, Notion, etc.).

| Item | Example / meaning |
|------|-------------------|
| **Worker URL** | `https://boing-native-dex-indexer.<account-subdomain>.workers.dev` — base for `/stats`, `/v1/directory/*`. |
| **Worker name** (Wrangler) | `boing-native-dex-indexer` — use with `wrangler secret put …` or `--name` if not in project dir. |
| **D1 database name** | `boing-native-dex-directory` |
| **D1 database ID** | `68eb37e6-f71c-47fa-9475-19d7a56b0db0` (this account; new ID if you recreate DB). |
| **KV namespace binding** | `NATIVE_DEX_INDEXER_KV` — document the namespace **title** in Cloudflare dashboard if you recreate it. |
| **KV state key** | `native_dex_indexer_state_v1` — persisted history blob for the indexer. |
| **Directory API id** | Response field `api`: `boing-native-dex-directory/v1` — clients can branch on this string. |
| **`latestSyncBatch` / `updatedAt`** | ISO timestamp; identifies which indexer run populated D1 (equals `payload.updatedAt` for that sync). Use to detect stale directory vs `/stats`. |
| **`poolCount`** | Rows in D1 after last successful sync — may differ from `GET /stats` `pools.length` only if sync failed midway (should be rare). |
| **Pagination** | `limit` (default 20, max 100), `cursor` / `nextCursor` (lowercase `poolHex`), `hasMore` — stable order by `pool_hex` ascending. |
| **R2 bucket** (GitHub upload) | `boing-finance-native-dex-stats` — same Cloudflare account as Workers; pair with object key e.g. `native-dex-indexer-stats.json`. |
| **Worker secret name** | `DIRECTORY_SYNC_SECRET` — **value is secret**; never commit. |

#### 1.1.2 `GET /v1/directory/pools` — each `pools[]` element (boing-sdk row)

| Field | Meaning |
|-------|---------|
| `poolHex` | 32-byte account id (`0x` + 64 hex) — **join key** for UI with RPC venues / `remoteIndexerStats.pools`. |
| `tokenAHex`, `tokenBHex` | Pool token legs (may be placeholder synth ids on empty storage). |
| `swapCount` | Swaps in the configured log **scan window** (not chain lifetime). |
| `swapCount24h`, `swaps24h` | Same value — swaps in last ~24h wall time (block header timestamps). |
| `volume24hApprox` | Sum of `amountIn` for 24h swaps (string integer). |
| `volumeScanWindowApprox` | Sum of `amountIn` over full scan window. |
| `tvlApprox` | Human-readable reserve summary from indexer (not USD). |
| `tvlUsdApprox` | Present only when `NATIVE_DEX_INDEXER_TOKEN_USD_JSON` (or equivalent) supplies prices. |
| `note` | Scan window, add/remove liquidity counts, etc. |

### 1.2 GitHub Actions (boing.finance)

**Workflow:** `.github/workflows/native-dex-indexer.yml`

- Builds `native-dex-indexer-stats.json` and uploads an artifact.
- **Optional job `upload-r2`:** runs when repo **Variables** `NATIVE_DEX_INDEXER_R2_BUCKET` and `NATIVE_DEX_INDEXER_R2_OBJECT_KEY` are non-empty, and uses secrets `CLOUDFLARE_API_TOKEN` + `CLOUDFLARE_ACCOUNT_ID` to `wrangler r2 object put`.

### 1.3 Pages Function (boing.finance)

- `GET /api/native-dex-indexer-stats` — same indexer JSON; optional `pools_page` / `pools_page_size` (in-repo helper `frontend/functions/lib/sliceIndexerPoolsResponse.mjs`). **No D1** on Pages path unless you add a separate integration.

---

## 2. What operators must do (terminal / dashboards)

### 2.0 Where to set Worker environment variables and secrets

| Kind | Where |
|------|--------|
| **Secrets** (for example **`DIRECTORY_SYNC_SECRET`**) | Dashboard **Secret**, **or** Wrangler from **`workers/native-dex-indexer/`** (`npm run secret:directory-sync`). From repo root use **`npx wrangler secret put DIRECTORY_SYNC_SECRET --cwd workers/native-dex-indexer`** or **`--name boing-native-dex-indexer`**. Never commit secret values. |
| **Plain text vars** | **`wrangler.toml` `[vars]`** (applied on each **`wrangler deploy`**), **or** dashboard **Variables**, **or** `npx wrangler vars put …`. Same key in multiple places can cause confusion — prefer one source of truth. |
| **Local `wrangler dev`** | Copy **`workers/native-dex-indexer/.dev.vars.example`** to **`.dev.vars`** in that folder. One `KEY=value` per line (same names as production). `.dev.vars` is gitignored — do not commit real secrets. |
| **Bindings** (D1 **`DIRECTORY_DB`**, KV, R2) | **`wrangler.toml`** only (`[[d1_databases]]`, `[[kv_namespaces]]`, `[[r2_buckets]]`) — not arbitrary string env vars. |

### 2.1 One-time: Worker secret for manual D1 refresh

```bash
cd boing.network/workers/native-dex-indexer
npm install   # once, so local wrangler is available
npm run secret:directory-sync
# paste a long random string when prompted
```

From **repo root** (no `cd`):

```bash
npx wrangler secret put DIRECTORY_SYNC_SECRET --cwd workers/native-dex-indexer
# or: npx wrangler secret put DIRECTORY_SYNC_SECRET --name boing-native-dex-indexer
```

Or from root **`package.json`**: `npm run native-dex-indexer-secret-put`.

Then (example):

```bash
curl -sS -X POST "https://boing-native-dex-indexer.<your-subdomain>.workers.dev/v1/directory/sync" \
  -H "Authorization: Bearer <same-secret>"
```

### 2.2 R2 bucket + GitHub (item 4)

Bucket **`boing-finance-native-dex-stats`** should exist on the same Cloudflare account as the Worker (create once if missing):

```bash
npx wrangler r2 bucket create boing-finance-native-dex-stats
```

Then in **GitHub → repo → Settings → Secrets and variables → Actions:**

**Repository variables** (recommended values):

| Name | Example value |
|------|----------------|
| `NATIVE_DEX_INDEXER_R2_BUCKET` | `boing-finance-native-dex-stats` |
| `NATIVE_DEX_INDEXER_R2_OBJECT_KEY` | `native-dex-indexer-stats.json` |

**Repository secrets** (already used by frontend deploy):

- `CLOUDFLARE_API_TOKEN` — must include **R2 write** (and typically Workers read if you use the same token elsewhere).
- `CLOUDFLARE_ACCOUNT_ID`

After the next scheduled or manual run of **Native DEX indexer**, the `upload-r2` job should upload the JSON object with `Content-Type: application/json`.

**Optional:** Attach a **custom domain** or **R2 public bucket policy** if you want browsers to fetch the JSON directly; otherwise serve via Worker or signed URLs per your security model.

### 2.3 D1 for preview / another account

If you fork or change Cloudflare account:

```bash
npx wrangler d1 create boing-native-dex-directory
npx wrangler d1 migrations apply boing-native-dex-directory --remote
```

Paste new `database_id` into `wrangler.toml` under `[[d1_databases]]`.

---

## 3. Suggested work for boing.network / protocol (not done in app repos)

These items **cannot** be completed in frontend-only or Worker-only code.

### 3.1 Native unsigned `contract_call` simulate (RPC)

**Shipped in this monorepo:** **`boing_simulateContractCall`** — [RPC-API-SPEC.md](RPC-API-SPEC.md); **`boing-sdk`** **`BoingClient.simulateContractCall`**; **`cargo test -p boing-node --test simulate_contract_call_rpc`**.

**Still operator / product choice:** whether **public** RPC endpoints expose the method (same as other `boing_*` calls today), stricter **rate limits**, or **auth** — not gated by a separate env flag in the first implementation.

**Downstream:** boing.finance can set **`REACT_APP_BOING_RPC_UNSIGNED_SIMULATE_METHOD=boing_simulateContractCall`** when the configured RPC runs a new enough **`boing-node`**; probe **`boing_rpcSupportedMethods`** before relying on it.

### 3.2 “Full” directory beyond indexer snapshot

The D1 API is a **materialized view of the indexer’s `pools[]`** (synced on cron / POST). It is **not** a chain-native event store or subgraph.

**Pool events (`directory_pool_events` + `GET /v1/history/pool/…/events`):** same **snapshot** contract — bounded window, last-sync replace, **no** reorg rewind semantics. Useful for **explorer-style “recent pool activity”** when paired with **`boing-sdk`** **`fetchNativeDexDirectoryPoolEventsPage`**, not for accounting-grade history.

**If you need:**

- Historical pool lifecycle, reorgs, or receipt-level attribution  
- Cross-shard / multi-RPC aggregation  

…plan a **dedicated indexer pipeline** (or node extension) and optionally keep this Worker as a thin cache or replace D1 with that system’s API.

### 3.3 LP NFT semantics & automatic vault routing

**Partially addressed in `boing-sdk` (model A path):** **`resolveNativeAmmVaultPoolMapping`**, **`fetchNativeDexLpVaultSharePositionForOwner`**, **`lpShareTokenBalanceStorageKeyHex`** / **`fetchLpShareTokenBalanceRaw`** — vault storage + LP share token balance via **`boing_getContractStorage`**. See [PROTOCOL_NATIVE_DEX_RPC_AND_INDEXING_ROADMAP.md](PROTOCOL_NATIVE_DEX_RPC_AND_INDEXING_ROADMAP.md) §2.5.

**Still need (protocol / product):**

- Canonical **position model** per network (**NFT** vs share token vs vault-only).  
- Enumeration for **model B** and “all my pools” without scanning every vault.  
- Optional dedicated **`GET /v1/lp/…`** on the Worker if RPC-only composition is not enough for apps.

---

## 4. Optional frontend follow-up (boing.finance)

Not required for the Worker to be useful:

- New env e.g. `REACT_APP_BOING_NATIVE_DEX_DIRECTORY_URL` pointing at `https://…/v1/directory/pools`.
- Pools UI: load **first page** from D1 for large deployments, or hybrid (RPC venues + directory metadata). Keep **merging** with `remoteIndexerStats` by `poolHex` as today.

---

## 5. Quick verification checklist

1. `GET …/v1/directory/meta` → `poolCount` increases after cron or `POST /v1/directory/sync`; after migration **`0002`**, **`eventCount`** reflects **`directory_pool_events`** rows.
2. `GET …/v1/directory/pools?limit=5` → non-empty `pools` when testnet has pools.
3. Use `nextCursor` on the next request until `hasMore` is false.
4. `GET …/v1/history/pool/{0x…64}/events?limit=10` → JSON `events[]` when the pool had swaps or liquidity logs in the indexer scan window (may be empty legitimately).
5. `GET …/v1/history/user/{0x…64}/events?limit=10` → rows where **`callerHex`** matches (same snapshot caveats).
6. `GET …/v1/directory/meta` → **`indexedTipHeight`** / **`indexedTipBlockHash`** populated after a successful sync with migration **`0003`** (compare to node head if you need a coarse freshness check).
7. After setting GitHub vars + secrets, confirm R2 object exists in the dashboard.
8. `GET /stats` still returns the full indexer document for the app.

---

## 6. Deployed Worker URL (this workspace’s last deploy)

`https://boing-native-dex-indexer.nico-chikuji.workers.dev`

Replace with your team’s production hostname if you add a **routes** / **workers.dev** alias or custom domain in Wrangler.
