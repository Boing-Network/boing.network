# Boing L1 media blobs

> 👋 **Everyday users:** this is how images / GIFs / short clips can live **on Boing itself** instead of only on HTTPS or IPFS. Phase 2 raised the honest cap to **1 MiB**. Phase 3 scaffolds header `blobs_root` + a **16 MiB target** path — still not “50 GB movies on L1.”  
> 🛠️ **Developers:** `PutBlob` (index **21**), `BlobAnnounce` (**22**), RPC `boing_getBlob*`, P2P topic `boing/media-blobs`, SDK `mediaBlob.ts`.  
> 🛰️ **Operators:** `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT` / `--media-blobs-activation-height`. Optional Phase 3: `BOING_BLOBS_ROOT_HEADER_ACTIVATION_HEIGHT`. Optional `blob_base_fee_per_kib` in `network_fee_config.json`. Sidecar: `{data_dir}/blobs/`.

**Contrast:** the [interface blob store](INTERFACE-REGISTRY.md) packs small JSON into **contract SSTORE** (~24 KiB). Media blobs use a **separate size-scaled fee schedule** (+ optional independent KiB levy) and a **node sidecar** — do not stuff art into SSTORE.

Project design: Cursor store `docs/boing-l1-media-blobs.md`. Phase 1 [#53](https://github.com/Boing-Network/boing.network/pull/53) (`964807a`). Phase 2 [#54](https://github.com/Boing-Network/boing.network/pull/54) (`4d7596e`).

---

## Caps

| Knob | Value |
|------|------:|
| Live max payload | **1 MiB** (`MAX_MEDIA_BLOB_BYTES`) |
| Phase 3 **target** max (not live) | **16 MiB** (`PHASE3_TARGET_MAX_MEDIA_BLOB_BYTES`) |
| DAS chunk hint (scaffold) | 64 KiB (`MEDIA_BLOB_DAS_CHUNK_BYTES`) |
| Max `content_type` | 64 UTF-8 bytes |
| Retention | `Prunable { ttl_blocks }` or `Forever` |
| Max prunable TTL | 1_000_000 blocks |
| Blob id | `BLAKE3(data)` (32 bytes) |
| URI | `boing://blob/0x{64 hex}` |
| Gossip topic | `boing/media-blobs` |

**Honest limit:** live admission is still **1 MiB**. Raising to the 16 MiB target requires a coordinated hard-fork (raise `MAX_MEDIA_BLOB_BYTES` + DAS/chunking). Do not claim multi-minute mp4 on L1 yet.

---

## Tx types

| Variant | Index | Bytes in tx? | Role |
|---------|------:|:------------:|------|
| `PutBlob` | 21 | yes | Inline publish (≤1 MiB live) |
| `BlobAnnounce` | 22 | no | Pay for size + commit id; bytes via P2P/RPC sidecar |

---

## Fee model

```
blob_gas = 21_000 + ceil(size / 32) * 16
effective_gas = blob_gas * (Forever ? 10 : 1)
gas_fee = network gas price × effective_gas
blob_data_fee = ceil(size / 1024) * NetworkFeePolicy.blob_base_fee_per_kib   # 0 = off
fee = gas_fee + extra_fixed_fee + blob_data_fee
```

`blob_base_fee_per_kib` is the **independent blob fee market knob**. Forever pin **rents** (`media_blob_pin_rent_kib_blocks`) are scaffolded only — not charged on-chain yet.

---

## Header `blobs_root` (Phase 3 opt-in)

`blobs_root_of_transactions(txs)` = BLAKE3(`BOING_BLOBS_ROOT_V1` ‖ blob ids in tx order).

When `BOING_BLOBS_ROOT_HEADER_ACTIVATION_HEIGHT` / `--blobs-root-header-activation-height` is set and `height ≥` that value, producers set `BlockHeader.blobs_root = Some(root)` (including `Hash::ZERO` when the block has no media txs). Before activation the field is **omitted** so historical encodings and block hashes stay unchanged. Full DAS / sampling / light-client proofs remain later.

---

## NFT binding

Reference NFT `metadata_hash` ([BOING-REFERENCE-NFT.md](BOING-REFERENCE-NFT.md)):

1. **Direct:** `metadata_hash = BlobId` when the token *is* the media file. Resolvers try `boing://blob/0x{hash}`.  
2. **Indirect (recommended):** JSON with `"image": "boing://blob/0x…"` (optional `animation_url` / `content_hash`); `metadata_hash = BLAKE3(JSON)`. SDK: `tokenMetadataJsonWithBlobImage`, `metadataHashWordToFetchUrls`, `blobIdHexFromBoingUri` → `getBlob`.

---

## RPC

| Method | Role |
|--------|------|
| `boing_getMediaBlobParams` | Caps, activation, gossip topic, Phase 3 scaffold fields |
| `boing_getBlobMeta` | Metadata or `null` |
| `boing_getBlob` | `{ meta, dataHex }` or `null` |

---

## Phases

| Phase | Status |
|-------|--------|
| 0 Design | Done |
| 1 MVP PutBlob + sidecar + RPC (256 KiB) | **Merged** #53 (`964807a`) |
| 2 1 MiB, BlobAnnounce, P2P, blob_base_fee_per_kib, soft blobs_root, NFT SDK | **Merged** #54 (`4d7596e`) |
| 3 Header `blobs_root` opt-in, DAS/chunk/pin-rent stubs, 16 MiB target path | This PR (draft) — live cap still 1 MiB; no sampling market yet |

---

## Env / CLI

```bash
BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT=1
# Optional Phase 3 (all validators must agree):
# BOING_BLOBS_ROOT_HEADER_ACTIVATION_HEIGHT=1
# network_fee_config.json may include:
#   "blob_base_fee_per_kib": "0"
boing-node --media-blobs-activation-height 1 \
  # --blobs-root-header-activation-height 1 \
  --data-dir ./data
```
