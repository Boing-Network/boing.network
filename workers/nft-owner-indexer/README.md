# Reference NFT owner indexer (D1)

Durable **collection + token id → owner** index for Boing **reference NFT** collections (`mint_batch` / `transfer_nft` / `set_metadata_hash`). Powers Express (and others) beyond the 256-block RPC scan window.

**Not** the DEX LP ERC-721 snapshot (`directory_nft_owner` on `native-dex-indexer`).

## API

Base URL after deploy: `https://boing-nft-owner-indexer.<account>.workers.dev` (or a custom route).

| Method | Path | Notes |
|--------|------|-------|
| **GET** | **`/v1/nfts/by-owner?owner=0x…64`** | Current holdings. Optional `limit` (default 50, max 200), `cursor` (`collection:tokenId`), `collection=` filter. |
| **GET** | **`/v1/nfts/item?collection=&tokenId=`** | Single item ownership row. |
| **GET** | **`/v1/meta`** (alias **`/v1/sync`**) | Cursor tip, lag vs finalized, row counts, `gapRanges` / `gapRangeCount`. |
| **GET** | **`/v1/gaps`** | Just the pruned-height `gapRanges` (see **Pruned RPC / scan gaps** below). |
| **POST** | **`/v1/sync`** | One ingest tick. Requires `Authorization: Bearer $NFT_OWNER_SYNC_SECRET`. |
| **POST** | **`/v1/backfill?from=&to=`** | Force-index inclusive heights (max 501). Same bearer. Response includes `contiguousThrough`, `cursorAdvanced`, `newGapRanges`. |
| **GET** | **`/health`** | Plain `ok`. |
| **GET** | **`/v1/items/by-owner?owner=0x…64`** | Reference **multi-token** (game items) the account touched, with **live** balances read from contract storage. Optional `contract=`, `limit` (≤200), `includeEmpty=1`. |
| **GET** | **`/v1/items/holders?contract=&itemId=`** | Accounts that touched one item type, with live balances. |
| **GET** | **`/v1/items/meta`** | Items cursor (`lastScannedHeight`). |
| **POST** | **`/v1/items/sync`** | Force an items catch-up of up to 40 heights (Free-plan subrequest cap). Same bearer. |

### Multi-token items index (request-driven, no cron)

`migrations/0003_multi_token_items.sql` adds `mt_cursor`, `mt_contracts`, `mt_holdings`. There is **no new cron**: every `/v1/items/*` read first runs a bounded catch-up (`BOING_ITEMS_MAX_BLOCKS_PER_REQUEST`, default 16 heights, at most once per `BOING_ITEMS_MIN_INTERVAL_SEC`, default 10 s) from `BOING_ITEMS_FROM_HEIGHT` (37540 on testnet; the template went live at 37545). Only direct `ContractCall`s to contracts whose installed code is the pinned `boing.reference_multi_token.v1` runtime are indexed (logs carry no emitter address). The table stores *which* (contract, item, account) pairs moved; balances are always read live (within the Free-plan 50-subrequest budget per request; rows past it return `balance: null` for the caller to read itself), so a reorged-out touch only shows a zero balance and needs no rewind. Spec: [docs/BOING-REFERENCE-MULTI-TOKEN.md](../../docs/BOING-REFERENCE-MULTI-TOKEN.md).

### `GET /v1/nfts/by-owner` response shape

```json
{
  "owner": "0x…",
  "items": [
    {
      "collection": "0x…",
      "tokenId": "0x…",
      "owner": "0x…",
      "metadataHash": "0x…" ,
      "lastBlockHeight": 12345,
      "lastTxId": "0x…",
      "lastEventKind": "mint_batch"
    }
  ],
  "nextCursor": "0x…:0x…",
  "indexer": {
    "lastCommittedHeight": 99999,
    "lastCommittedBlockHash": "0x…",
    "chainId": "boing-testnet"
  }
}
```

`metadataHash` may be `null` (option B zero-hash mint, or never set).

## Deploy (Nico)

```bash
cd workers/nft-owner-indexer
npm install
# build SDK decode exports if you changed boing-sdk:
(cd ../../boing-sdk && npm run build)

# 1) Create D1 once
npx wrangler d1 create boing-nft-owner-index
# Paste database_id into wrangler.toml (replace REPLACE_AFTER_D1_CREATE)

# 2) Apply migrations + deploy
npm run d1:apply:remote
npm run secret:sync   # set NFT_OWNER_SYNC_SECRET
npx wrangler deploy

# 3) Optional: point Observer proxy
# In boing.observer Cloudflare / GitHub env:
# NFT_OWNER_INDEXER_URL=https://boing-nft-owner-indexer.<account>.workers.dev
```

Cron default: every **2 minutes** (`*/2 * * * *`). Tune `BOING_MAX_BLOCKS_PER_TICK` (default 16).

Public RPC: `BOING_RPC_URL=https://testnet-rpc.boing.network` (already in `[vars]`). Set `BOING_OMIT_MISSING=1` for pruned nodes.

## Backfill existing history

Cold start indexes from height **0** (or `BOING_BACKFILL_FROM_HEIGHT`) up to finalized via cron ticks.

To accelerate:

```bash
# Chunked backfill (501 heights max per call)
curl -X POST -H "Authorization: Bearer $NFT_OWNER_SYNC_SECRET" \
  "https://boing-nft-owner-indexer…/v1/backfill?from=0&to=500"
# then from=501&to=1000, …
# Or repeatedly: POST /v1/sync until GET /v1/meta shows lagVsFinalized ≈ 0
```

After a full rewind/wipe, re-run migrations are not required — delete D1 rows or recreate the DB, then backfill again.

## Authorization replay (no-op transfers)

The reference template (v1–v3) ends an unauthorized `transfer_nft` or `set_metadata_hash` with `STOP`,
so its receipt says `success: true` although storage did not change. The indexer therefore replays the
template's own rules before recording an event (`src/extract.ts`, `src/admin.ts`):

- an owned token moves only when `tx.sender` is its current owner;
- an unowned token (lazy mint) and `mint_batch` need the collection admin (read once per collection
  from the template admin slot `0x00…e0` with `boing_getContractStorage`);
- `set_metadata_hash` needs the admin or the current owner;
- a contract whose admin slot is empty is not indexed as a reference collection.

**After deploying this change, wipe and rebuild the D1 index** (see "Backfill existing history"): rows
written by older builds may contain ownership from no-op transfers, and later owner checks build on them.

## Template v4 collections

Collections running the pinned **template v4** runtime (`docs/BOING-REFERENCE-NFT.md` § Template v4) fault
on every failed check and log every change, so no replay is needed. For each collection seen in a block
the indexer reads `boing_getContractCode` once (cached per isolate; installed code never changes) and,
for v4, records events from the receipt logs of successful direct calls only: `MINT_BATCH` (metadata
hashes come from the successful calldata), `TRANSFER` (covers owner, operator and approved-spender
transfers) and `METADATA`. Logs carry no emitter address today; v4 makes no nested calls, so every log
in a direct call's receipt came from the collection. Approval and freeze logs are not stored.

## D1 write budget (free plan)

D1 on the free plan allows about 100k written rows a day. The ingest loop writes an `indexed_blocks` row
for every block that has reference NFT events, and for event-less blocks only the **last one of each
tick** (where the cursor lands and the tip reorg check compares hashes). Before this, every height wrote
a row, which is ~92k rows a day at 128 blocks every 2 minutes, so a backfill alone used most of the daily
allowance. A reorg fork search that meets an unstored height treats it as a mismatch and walks lower, so a
rewind may re-index a few extra empty heights but never skips one. `stats.indexedBlocks` in `/v1/meta`
counts stored rows, not heights.

## Reorgs

Same pattern as OBS-1 (`examples/observer-d1-worker`):

1. Compare `boing_getBlockByHeight(h, false)` hash to `indexed_blocks`.
2. On mismatch, delete `nft_events` + `indexed_blocks` at that height and **rebuild** `nft_ownership` for affected tokens from remaining events.
3. Parent-hash mismatch on extend aborts the tick (next cron retries after rewind).

Disable only for debugging: `BOING_DISABLE_REORG_REWIND=1`.

## Pruned RPC / scan gaps

Public RPC nodes prune history beyond a window, so `BOING_OMIT_MISSING=1` (the default) skips
missing heights instead of failing the whole tick. Skipped heights are **never silently folded
into the committed cursor** — the indexer uses `boing-sdk`'s gap helpers
(`summarizeIndexerFetchGaps`, `nextContiguousIndexedHeightAfterOmittedFetch`,
`mergeInclusiveHeightRanges`, `subtractInclusiveRangeFromRanges`) the same way
`examples/observer-d1-worker` does for OBS-1:

- The **global ingest cursor** (`GET /v1/meta` → `lastCommittedHeight`) only advances to the
  longest *contiguous* prefix of a cron window — it stops right before a pruned height instead of
  jumping past it, even though any present blocks past the hole are still indexed (just not yet
  counted as "committed").
- Every pruned height is recorded in D1 **`block_height_gaps`**, surfaced at **`GET /v1/gaps`**
  and inside **`GET /v1/meta`** (`gapRanges` / `gapRangeCount`).
- **`POST /v1/backfill?from=&to=`** re-attempts a specific range (e.g. one you point at an
  archive/unpruned RPC, or re-point `BOING_RPC_URL` at temporarily). It always re-indexes whatever
  blocks are present (even a historical range behind the current cursor — the global cursor is
  left untouched unless the backfilled window directly continues it) and reconciles
  `block_height_gaps`: a height that comes back present this time is removed from the gap table;
  a height still missing stays recorded.

Operator loop for a reported gap:

```bash
curl "$WORKER_URL/v1/gaps"
# { "gapRanges": [{ "fromHeight": 1200, "toHeight": 1250 }], ... }

curl -X POST -H "Authorization: Bearer $NFT_OWNER_SYNC_SECRET" \
  "$WORKER_URL/v1/backfill?from=1200&to=1250"
# repeat against an archive RPC if the default pruned public RPC still can't serve that range
```

## Observer proxy

`boing.observer` **`GET /api/account/nfts?network=testnet&id=0x…`** proxies to this Worker when **`NFT_OWNER_INDEXER_URL`** is set. Without it, the route returns **503** with a setup hint (explorer stays RPC-only for NFT lists).

## Express integration

Prefer the Worker URL (or Observer proxy) over the 256-block local scan for gallery lists; still verify ownership with `boing_getContractStorage` before transfers.
