# Reference NFT owner indexer (D1)

Durable **collection + token id → owner** index for Boing **reference NFT** collections (`mint_batch` / `transfer_nft` / `set_metadata_hash`). Powers Express (and others) beyond the 256-block RPC scan window.

**Not** the DEX LP ERC-721 snapshot (`directory_nft_owner` on `native-dex-indexer`).

## API

Base URL after deploy: `https://boing-nft-owner-indexer.<account>.workers.dev` (or a custom route).

| Method | Path | Notes |
|--------|------|-------|
| **GET** | **`/v1/nfts/by-owner?owner=0x…64`** | Current holdings. Optional `limit` (default 50, max 200), `cursor` (`collection:tokenId`), `collection=` filter. |
| **GET** | **`/v1/nfts/item?collection=&tokenId=`** | Single item ownership row. |
| **GET** | **`/v1/meta`** (alias **`/v1/sync`**) | Cursor tip, lag vs finalized, row counts. |
| **POST** | **`/v1/sync`** | One ingest tick. Requires `Authorization: Bearer $NFT_OWNER_SYNC_SECRET`. |
| **POST** | **`/v1/backfill?from=&to=`** | Force-index inclusive heights (max 501). Same bearer. |
| **GET** | **`/health`** | Plain `ok`. |

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

## Reorgs

Same pattern as OBS-1 (`examples/observer-d1-worker`):

1. Compare `boing_getBlockByHeight(h, false)` hash to `indexed_blocks`.
2. On mismatch, delete `nft_events` + `indexed_blocks` at that height and **rebuild** `nft_ownership` for affected tokens from remaining events.
3. Parent-hash mismatch on extend aborts the tick (next cron retries after rewind).

Disable only for debugging: `BOING_DISABLE_REORG_REWIND=1`.

## Observer proxy

`boing.observer` **`GET /api/account/nfts?network=testnet&id=0x…`** proxies to this Worker when **`NFT_OWNER_INDEXER_URL`** is set. Without it, the route returns **503** with a setup hint (explorer stays RPC-only for NFT lists).

## Express integration

Prefer the Worker URL (or Observer proxy) over the 256-block local scan for gallery lists; still verify ownership with `boing_getContractStorage` before transfers.
