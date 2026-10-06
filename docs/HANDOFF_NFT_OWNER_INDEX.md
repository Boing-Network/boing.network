# HANDOFF: Durable reference NFT owner index

> 🛰️ **Operators:** Worker path **`workers/nft-owner-indexer/`**. See that README for API + deploy. This handoff is the short product pointer.

## Problem

Express / Observer discover reference NFTs by scanning ≤ **256** recent blocks for `mint_batch` / `transfer_nft`. Older holdings disappear from the wallet gallery unless the user already watched them.

## Solution

Cloudflare Worker + D1 that:

1. Replays blocks with receipts (cron + optional backfill).
2. Decodes reference NFT calldata (`boing-sdk` **`decodeReferenceNftCalldata`**).
3. Stores event history + current ownership.
4. Serves **`GET /v1/nfts/by-owner?owner=`**.
5. Rewinds on tip hash mismatch (OBS-1 pattern).

## Normative docs in-repo

- Worker: [`workers/nft-owner-indexer/README.md`](../workers/nft-owner-indexer/README.md)
- Calldata: [`BOING-REFERENCE-NFT.md`](BOING-REFERENCE-NFT.md)
- OBS-1 reorg model: [`OBSERVER-HOSTED-SERVICE.md`](OBSERVER-HOSTED-SERVICE.md) §6

## Related (do not confuse)

| Surface | What it indexes |
|---------|-----------------|
| **This worker** | Reference NFT collections (template v1–v3 calldata) |
| `native-dex-indexer` **`directory_nft_owner`** | Optional ERC-721 **LP position** NFT Transfer logs |
| Observer `/api/asset/nft` | Live storage probe for one collection+tokenId |
| Express local scan | ≤256-block wallet-side discovery |

## Deploy checklist

1. `npx wrangler d1 create boing-nft-owner-index` → paste id into `workers/nft-owner-indexer/wrangler.toml`
2. `npm run d1:apply:remote` && `npm run deploy` from that folder
3. `wrangler secret put NFT_OWNER_SYNC_SECRET`
4. Backfill via `POST /v1/backfill` chunks or wait for cron catch-up
5. Set Observer **`NFT_OWNER_INDEXER_URL`** so **`/api/account/nfts`** proxies the Worker
