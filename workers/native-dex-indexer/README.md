# Native DEX indexer (Cloudflare Worker)



Serves JSON compatible with `REACT_APP_BOING_NATIVE_DEX_INDEXER_STATS_URL` on the Boing web app: pool rows (including **24h swap/volume** from block timestamps), optional **USD TVL** via `NATIVE_DEX_INDEXER_TOKEN_USD_JSON`, **`tokenDirectory`** for token pickers, **KV-persisted** reserve `history`, and a **D1-backed** paginated directory API.



## Setup



1. From this directory: `npm install`. **Wrangler** resolves **`@noble/*`** imports from paths under **`boing-sdk/dist/`**, so **`boing-sdk/node_modules`** must exist: run **`npm ci`** (or `npm install`) in **`../../boing-sdk`** before **`wrangler deploy`** (the GitHub Action does both). The Worker **`file:`** link does not install into **`boing-sdk/`** for you.

2. **KV:** `wrangler.toml` binds `NATIVE_DEX_INDEXER_KV` (production + `preview_id` for `wrangler dev`). For another Cloudflare account, create namespaces and paste `id` / `preview_id`.

3. **D1:** `wrangler.toml` binds `DIRECTORY_DB` → `boing-native-dex-directory`. For a new account: `npx wrangler d1 create boing-native-dex-directory` then `npx wrangler d1 migrations apply boing-native-dex-directory --remote` (runs `0001` … `0006` from `migrations/`) and update `database_id`.

4. **Manual sync secret (optional):** Wrangler must see **`workers/native-dex-indexer/wrangler.toml`** (Worker name **`boing-native-dex-indexer`**). From **this directory**: `npm run secret:directory-sync` (or `npx wrangler secret put DIRECTORY_SYNC_SECRET`). From **repo root**: `npx wrangler secret put DIRECTORY_SYNC_SECRET --cwd workers/native-dex-indexer` or `npx wrangler secret put DIRECTORY_SYNC_SECRET --name boing-native-dex-indexer`. Enables `POST /v1/directory/sync` with `Authorization: Bearer …`.

5. Set any `REACT_APP_BOING_NATIVE_AMM_POOL` / factory overrides as Worker vars (same names as the frontend build).

6. Deploy: `npm run deploy`

### Where to set variables and secrets

| Kind | Where |
|------|--------|
| **Secrets** (for example `DIRECTORY_SYNC_SECRET`) | Dashboard **Secret**, **or** `npm run secret:directory-sync` from **`workers/native-dex-indexer/`** (not repo root unless you pass **`--cwd`** / **`--name`** — see setup step 4). Never put secrets in `wrangler.toml` or git. |
| **Plain text vars** | **`[vars]` in `wrangler.toml`** (applied on `wrangler deploy`), **or** dashboard **Variables**, **or** `npx wrangler vars put KEY --value "..."` (Wrangler 4+). Use one source of truth to avoid conflicting values. |
| **Local `wrangler dev`** | Copy `.dev.vars.example` to **`.dev.vars`** in this folder (same `KEY=value` lines as production). Wrangler ignores `.dev.vars` in git by default — do not commit real secrets. |

Bindings (**D1**, **KV**, **R2**) are configured in `wrangler.toml`, not as arbitrary env strings.



## Endpoints



- `GET /stats` or `GET /` — full indexer JSON (cron + on-demand refresh into KV). Optional `pools_page`, `pools_page_size` (1–500) slices `pools` and adds `poolsPageMeta`.

- `GET /v1/directory/meta` — `{ poolCount, eventCount?, nftOwnerRowCount?, receiptLogCount?, latestSyncBatch, indexedTipHeight?, indexedTipBlockHash?, indexedParentBlockHash? }` from D1 (tip + parent hash after migration `0004`; NFT count after `0005`; receipt count after `0006` when archiving is enabled).

- `GET /v1/directory/pools?limit=&cursor=` — cursor pagination (default `limit` 20, max 100). First page: omit `cursor`. Next page: `cursor=<last poolHex from previous response>`.

- `GET /v1/history/pool/{pool_hex}/events?limit=&cursor=` — **snapshot** of parsed pool **`Log2`** events (swap / add / remove) for the last indexer scan window; **not** reorg-safe history. `cursor` = D1 row id from `nextCursor` (default `limit` 50, max 200).

- `GET /v1/history/user/{caller_hex}/events?limit=&cursor=` — same snapshot; filtered by event **`caller`** (native AMM log topic1).

- `GET /v1/history/receipts?limit=&cursor=&topic0=` — optional **bounded** execution-log rows from the last `NATIVE_DEX_INDEXER_RECEIPT_ARCHIVE_BLOCKS` heights (requires D1 `0006` and that var **> 0**). `topic0` optional filter (`0x` + 64 hex). Same snapshot / reorg caveats as pool events.

- `GET /v1/lp/vault/{vault_hex}/mapping` — RPC read of vault configure storage (**model A**).

- `GET /v1/lp/positions?owner=` — live RPC share balances for each vault in `NATIVE_DEX_INDEXER_LP_VAULT_HEXES` (or canonical testnet vault when unset).

- `GET /v1/lp/nft/positions?owner=&contract=` — D1 rows from ERC-721 **`Transfer`** indexing when `NATIVE_DEX_INDEXER_LP_NFT_CONTRACT_HEX` is set (or pass `contract` query param).

- `POST /v1/directory/sync` — rebuild indexer + refresh D1 (pools, events, tip row); requires `DIRECTORY_SYNC_SECRET` and `Authorization: Bearer <secret>`.



Cron (`*/15 * * * *`) refreshes KV **and** D1 when `DIRECTORY_DB` is bound.



## Env vars



| Var | Purpose |

|-----|---------|

| `BOING_TESTNET_RPC_URL` | Boing JSON-RPC base URL |

| `NATIVE_DEX_INDEXER_RPC_URL` | Overrides RPC for this worker |

| `NATIVE_DEX_INDEXER_REGISTER_FROM_BLOCK` | Factory `register_pair` merge (optional) |

| `NATIVE_DEX_INDEXER_LOG_SCAN_BLOCKS` | Per-pool log depth (default 8000) |

| `NATIVE_DEX_INDEXER_TOKEN_USD_JSON` | Static USD per token |

| `NATIVE_DEX_INDEXER_TOKEN_DIRECTORY_JSON` | Extra `tokenDirectory` rows (JSON array) |

| `NATIVE_DEX_INDEXER_API_DISABLE` | `1` to stop cron + disable GET |

| `DIRECTORY_SYNC_SECRET` | **Secret** (not in `wrangler.toml`) for `POST /v1/directory/sync` |
| `NATIVE_DEX_INDEXER_LP_VAULT_HEXES` | Comma-separated vault **`AccountId`** hex list for **`GET /v1/lp/positions`** (defaults to canonical testnet vault from **`boing-sdk`**) |
| `NATIVE_DEX_INDEXER_LP_NFT_CONTRACT_HEX` | ERC-721 contract for LP NFT **`Transfer`** indexing + **`GET /v1/lp/nft/positions`** default `contract` |
| `NATIVE_DEX_INDEXER_RECEIPT_ARCHIVE_BLOCKS` | When **> 0**, sync fills **`directory_receipt_log`** from blocks with receipts (default **0** = off; needs D1 migration **`0006`**) |
| R2 `INDEXER_ARCHIVE_R2` | Optional binding in `wrangler.toml` — small JSON manifest per successful directory sync |

## Operator values to keep handy

| Item | Notes |
|------|--------|
| Worker URL | `https://boing-native-dex-indexer.<you>.workers.dev` |
| Worker name | `boing-native-dex-indexer` |
| D1 | `boing-native-dex-directory` · id in `wrangler.toml` |
| KV key | `native_dex_indexer_state_v1` |
| Directory API | response `api`: `boing-native-dex-directory/v1` |
| Sync batch | `latestSyncBatch` in `/v1/directory/meta` (= indexer `updatedAt` for that run) |
| R2 (CI) | Bucket `boing-finance-native-dex-stats` + GitHub vars for object key |

Pool row fields (`poolHex`, tokens, `swapCount` vs `swapCount24h`, volumes, `tvlUsdApprox`) — see **`docs/HANDOFF_NATIVE_DEX_DIRECTORY_R2_AND_CHAIN.md`** §1.1.2.

## Related



- **Handoff (protocol + ops):** `boing.network/docs/HANDOFF_NATIVE_DEX_DIRECTORY_R2_AND_CHAIN.md`

- Core logic: `boing-sdk` → `nativeDexIndexerStats.ts`

- App CLI: `boing.finance/frontend` → `npm run indexer:native-dex`

- App Pages: `/api/native-dex-indexer-stats` (KV optional; no D1 unless extended)

- GitHub R2 upload: `boing.finance/.github/workflows/native-dex-indexer.yml` (optional job `upload-r2`)


