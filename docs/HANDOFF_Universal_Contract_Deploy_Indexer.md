# Handoff: Universal contract deploy listing + live updates

**Goal:** Offer an HTTP (and optional **SSE**) surface that lists **predicted contract account ids** for every **`ContractDeploy` / `ContractDeployWithPurpose` / `ContractDeployWithPurposeAndMetadata`** transaction, and stays current as new blocks are committed.

**This is not** `boing_listDexTokens` or the native DEX directory Worker (DEX-scoped pools/tokens). It is a **chain-wide deploy feed** backed by an **indexer** that reads full **`transactions`** from **`boing_getBlockByHeight`**.

**Related:** [HANDOFF_Boing_Network_Global_Token_Discovery.md](HANDOFF_Boing_Network_Global_Token_Discovery.md) (DEX discovery consumers; pin **`boing-sdk@^0.3.1`**). Node RPC reference: [RPC-API-SPEC.md](RPC-API-SPEC.md) (`boing_getBlockByHeight`). Receipt/log indexers: [INDEXER-RECEIPT-AND-LOG-INGESTION.md](INDEXER-RECEIPT-AND-LOG-INGESTION.md).

---

## Components (this repo)

| Piece | Location |
|-------|-----------|
| **Extract + tx id** | **`boing-sdk`** — `universalContractDeployIndex.ts`: **`rpcTransactionJsonToTransactionInput`**, **`transactionIdFromUnsignedRpcTransaction`** (BLAKE3(bincode(`Transaction`)) = node **`tx_id`**), **`extractUniversalContractDeploymentsFromBlock`** / **`extractUniversalContractDeploymentsFromBlockJson`**, **`CONTRACT_DEPLOY_INIT_CODE_MARKER`**, types **`UniversalContractDeploymentRow`**, **`UniversalContractDeployPayloadKind`**. Matches node rules for nonce-derived and CREATE2 deploys (including init-code / double prediction). Published as **`boing-sdk@0.3.1+`**. |
| **Hosted indexer (optional)** | **`workers/deploy-registry-indexer`** — Cloudflare Worker + **D1** + **cron** ingest + **batched** SQL + **parallel** block RPC. Operator README: [../workers/deploy-registry-indexer/README.md](../workers/deploy-registry-indexer/README.md). |

---

## Architecture (hosted Worker)

1. **Cron** (default: every minute, `wrangler.toml` `[triggers].crons`) runs **`syncDeployRegistry`**.
2. Read **`ingest_state.next_height`** from D1; compare to **`boing_chainHeight`**; ingest at most **`DEPLOY_REGISTRY_MAX_BLOCKS_PER_TICK`** blocks per tick.
3. For each wave, fetch up to **`DEPLOY_REGISTRY_PARALLEL_FETCHES`** heights in parallel via **`boing_getBlockByHeight(height, false)`** (no receipts required for deploy listing).
4. **`extractUniversalContractDeploymentsFromBlock`** produces rows; **`INSERT OR IGNORE`** into **`contract_deployments`** in batches of up to **100** statements per D1 **`batch()`** call.
5. Advance **`next_height`**; write **`ingest_state.last_sync`** JSON (success or failure telemetry).

**Catch-up:** increase **`DEPLOY_REGISTRY_MAX_BLOCKS_PER_TICK`** / **`DEPLOY_REGISTRY_PARALLEL_FETCHES`** within CPU limits, and/or call **`POST /v1/sync`** in a loop with a configured bearer secret (same bounded pass as cron).

---

## HTTP API (Worker)

Base URL is your deployed Worker origin (e.g. `https://<name>.<subdomain>.workers.dev`). All JSON responses use CORS **`Access-Control-Allow-Origin: *`** unless noted.

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Liveness. Optional **`?deep=1`**: runs **`SELECT 1`** on D1 — response includes **`db: true`** or **503** with error. |
| GET | `/v1/status` | **`schemaVersion`: 2** — ingest **`nextHeight`**, chain **`tipHeight`**, **`blocksPending`**, resolved **`config`** (`maxBlocksPerTick`, `parallelFetches`), **`lastSync`** (last ingest telemetry from D1, or **`null`** before first successful sync on a new deploy). **`Cache-Control: no-store`**. |
| GET | `/v1/deployments?limit=&cursor=` | **`schemaVersion`: 1** — global cursor pagination by monotonic table **`id`** (`cursor` = last **`id`** from previous page; opaque **`nextCursor`**). |
| GET | `/v1/deployments/by-tx/{0x…64}?limit=` | Rows for **`tx_id_hex`** (CREATE2 + init-code can yield **2** rows for one tx). |
| GET | `/v1/deployments/by-block/{height}?limit=` | Rows in **`block_height`**, ordered by **`tx_index`**, then **`id`**. |
| GET | `/v1/deployments/by-sender/{0x…64}?limit=&cursor=` | Filter **`sender_hex`**, same cursor semantics as global list (**`id`**). |
| GET | `/v1/deployments/stream?since_id=` | **SSE** (`text/event-stream`): **`data:`** JSON rows; **`event: ping`** when idle (~**2.5s**); ~**100ms** between polls when rows were returned; **`LIMIT 50`** per poll; closes cleanly when the client aborts. |
| GET | `/v1/contract/{0x…64}` | Single row by **`contract_hex`** (**404** if unknown). |
| POST | `/v1/sync` | One bounded ingest pass (same limits as cron). Requires **`DEPLOY_REGISTRY_SYNC_SECRET`** and header **`Authorization: Bearer <secret>`**; **503** if secret unset; **401** if token wrong. Response includes **`indexedBlocks`**, **`rowsInserted`**, **`nextHeight`**, **`chainTip`**, **`durationMs`**. |
| OPTIONS | `*` | CORS preflight (**`GET`**, **`POST`**; allows **`Authorization`**, **`Content-Type`**). |

**Row shape** (all deployment GETs): **`id`**, **`contract_hex`**, **`block_height`**, **`tx_index`**, **`tx_id_hex`**, **`sender_hex`**, **`payload_kind`**, optional **`purpose_category`**, **`asset_name`**, **`asset_symbol`**.

---

## D1 schema and migrations

Migrations live in **`workers/deploy-registry-indexer/migrations/`**.

| File | Purpose |
|------|---------|
| **`0001_contract_deployments.sql`** | **`ingest_state`** (`k`, `v`); **`contract_deployments`** (unique **`contract_hex`**, indexes on **`id`** and **`(block_height, tx_index)`**). |
| **`0002_indexes_and_telemetry.sql`** | Secondary indexes on **`tx_id_hex`** and **`sender_hex`**; Worker writes **`ingest_state`** row **`last_sync`** = JSON telemetry (not created by SQL — application-managed). |

Apply locally: **`npx wrangler d1 migrations apply <database_name> --local`**. Apply to production: **`npx wrangler d1 migrations apply <database_name> --remote`**.

**`ingest_state` keys**

| Key | Value |
|-----|--------|
| **`next_height`** | Next block height to ingest (decimal string). |
| **`last_sync`** | JSON: **`ok`**, **`at`** (ISO time), **`durationMs`**; on success **`indexedBlocks`**, **`rowsInserted`**, **`nextHeight`**, **`chainTip`**; on failure **`error`** (truncated). |

---

## Environment variables and secrets

Configured in **`workers/deploy-registry-indexer/wrangler.toml`** (`[vars]`) or Wrangler secrets.

| Name | Kind | Description |
|------|------|-------------|
| **`DEPLOY_REGISTRY_RPC_URL`** | var | Boing JSON-RPC base URL (must return full **`transactions`** on **`boing_getBlockByHeight`**). |
| **`DEPLOY_REGISTRY_FROM_HEIGHT`** | var | First height to index (inclusive); default **`0`**. |
| **`DEPLOY_REGISTRY_MAX_BLOCKS_PER_TICK`** | var | Cap blocks processed per cron or **`POST /v1/sync`** (max **256** in code). |
| **`DEPLOY_REGISTRY_PARALLEL_FETCHES`** | var | Concurrent **`getBlockByHeight`** calls per wave (**1–16**). |
| **`DEPLOY_REGISTRY_SYNC_SECRET`** | **secret** | Enables **`POST /v1/sync`**. Set with **`npm run secret:sync`** in the Worker package ( **`wrangler secret put DEPLOY_REGISTRY_SYNC_SECRET`** ). |

D1 binding: **`DEPLOY_REGISTRY_DB`** in **`wrangler.toml`** (`[[d1_databases]]`).

---

## Operator checklist

1. **`cd workers/deploy-registry-indexer && npm install`**
2. Create D1 if needed: **`npx wrangler d1 create boing-deploy-registry`** — set **`database_id`** in **`wrangler.toml`**.
3. **`npx wrangler d1 migrations apply boing-deploy-registry --remote`** (and **`--local`** for **`wrangler dev`**).
4. Point **`DEPLOY_REGISTRY_RPC_URL`** at a node whose **`boing_getBlockByHeight`** returns **`transactions`** matching **`boing_primitives::Block`** serde JSON.
5. Tune **`DEPLOY_REGISTRY_MAX_BLOCKS_PER_TICK`** and **`DEPLOY_REGISTRY_PARALLEL_FETCHES`** so each tick stays within Worker CPU limits; backlog drains over multiple ticks or via **`POST /v1/sync`**.
6. Optional: set **`DEPLOY_REGISTRY_SYNC_SECRET`** for operator-driven catch-up.
7. **`npx wrangler deploy`** — confirm **`GET /v1/status`** and **`GET /health?deep=1`**.

---

## Client integration

- **Pagination:** `GET /v1/deployments?cursor={last_id}&limit=100` — treat **`nextCursor`** as opaque.
- **Observability:** `GET /v1/status` — cursor vs tip, **`blocksPending`**, effective batch sizes, **`lastSync`**.
- **Liveness:** `GET /health?deep=1` — confirms D1 from the Worker.
- **Single contract:** `GET /v1/contract/0x…` (**64** hex chars).
- **By tx / block / sender:** filtered routes above (indexed columns after migration **0002**).
- **Near-real-time:** `GET /v1/deployments/stream?since_id=` (SSE) or **`BoingNewHeadsWs`** + periodic **`GET /v1/deployments`**.
- **Burst ingest (ops):** `POST /v1/sync` with bearer secret (same bounded work as one cron tick).
- **Verification:** recompute addresses with **`predictNonceDerivedContractAddress`** / **`predictCreate2ContractAddress`** from the same deploy transaction JSON if auditing the indexer.

---

## Limits and trust

- **Existence-only:** rows appear only when the indexer **successfully fetched** a block and parsed deploy txs; no token safety or spam scoring.
- **Missed blocks:** if RPC omits a height or returns empty, the cursor still advances; use **`boing-sdk`** gap helpers or reseed **`next_height`** for deliberate backfill.
- **Not every account:** only **`ContractDeploy*`** payloads; EOAs and contracts created outside these txs do not appear.
- **SSE:** best-effort; clients should **reconnect** and pass the last seen **`id`** as **`since_id`**.
- **Worker limits:** long SSE streams and heavy **`POST /v1/sync`** loops are subject to Cloudflare CPU and subrequest limits; prefer reasonable **`MAX_BLOCKS_PER_TICK`** and spacing for production.
