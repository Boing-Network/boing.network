# Boing L1 media blobs

> 👋 **Everyday users:** images / GIFs / short clips can live **on Boing itself**. Phase 4 raises the honest live cap to **16 MiB** — still not “50 GB movies on L1.”  
> 🛠️ **Developers:** `PutBlob` (21), `BlobAnnounce` (22), RPC `boing_getBlob*` / `boing_getBlobDasSample`, P2P `boing/media-blobs`, SDK `mediaBlob.ts`.  
> 🛰️ **Operators:** `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`. Optional header: `BOING_BLOBS_ROOT_HEADER_ACTIVATION_HEIGHT`. Fee JSON may set `blob_base_fee_per_kib` and `blob_forever_pin_fee_per_kib`. Sidecar: `{data_dir}/blobs/`.

**Contrast:** the [interface blob store](INTERFACE-REGISTRY.md) packs small JSON into **contract SSTORE** (~24 KiB). Media blobs use a **separate size-scaled fee schedule** (+ optional KiB levies) and a **node sidecar**.

Project design: Cursor store `docs/boing-l1-media-blobs.md`. Merges: [#53](https://github.com/Boing-Network/boing.network/pull/53) · [#54](https://github.com/Boing-Network/boing.network/pull/54) · [#55](https://github.com/Boing-Network/boing.network/pull/55) (`1142be2`).

---

## Caps (Phase 4)

| Knob | Value |
|------|------:|
| Live max payload | **16 MiB** (`MAX_MEDIA_BLOB_BYTES`) |
| Prefer BlobAnnounce above | **1 MiB** (`MAX_MEDIA_BLOB_INLINE_PREFER_BYTES`) |
| DAS chunk size | 64 KiB |
| Max `content_type` | 64 UTF-8 bytes |
| Retention | `Prunable { ttl_blocks }` or `Forever` |
| Max prunable TTL | 1_000_000 blocks |
| Blob id | `BLAKE3(data)` |
| URI | `boing://blob/0x{64 hex}` |
| Gossip topic | `boing/media-blobs` |

**Honest limit:** 16 MiB fits larger images / GIFs / short clips. Multi-minute mp4 and planet-scale DAS markets are still later. Media txs remain gated by `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`.

---

## Tx types

| Variant | Index | Bytes in tx? | Role |
|---------|------:|:------------:|------|
| `PutBlob` | 21 | yes | Inline publish (≤16 MiB; prefer announce above 1 MiB) |
| `BlobAnnounce` | 22 | no | Pay for size + commit id; bytes via P2P/RPC sidecar |

---

## Fee model

```
blob_gas = 21_000 + ceil(size / 32) * 16
effective_gas = blob_gas * (Forever ? 10 : 1)
gas_fee = network gas price × effective_gas
blob_data_fee = ceil(size / 1024) * blob_base_fee_per_kib          # 0 = off
forever_pin  = Forever ? ceil(size / 1024) * blob_forever_pin_fee_per_kib : 0   # 0 = off
fee = gas_fee + extra_fixed_fee + blob_data_fee + forever_pin
```

`blob_forever_pin_fee_per_kib` is the **minimal on-chain Forever pin-rent path** (governance / operator JSON). Full pin markets / renewals remain later.

---

## Header `blobs_root` (opt-in)

When `BOING_BLOBS_ROOT_HEADER_ACTIVATION_HEIGHT` is set and `height ≥` that value, producers set `BlockHeader.blobs_root = Some(root)`. Before activation the field is omitted.

---

## Chunking / DAS hooks (not a full market)

- `chunk_media_blob` → data chunks + **XOR parity** (recover one missing data chunk)
- Merkle root over chunk hashes + `boing_getBlobDasSample` (chunk leaf + siblings)
- Light clients can verify inclusion; this is **not** Reed–Solomon or a paid sampling market yet

---

## NFT binding

1. **Direct:** `metadata_hash = BlobId` → `boing://blob/0x{hash}`.  
2. **Indirect:** JSON `"image": "boing://blob/0x…"`; SDK helpers in `metadataMedia.ts`.

---

## RPC

| Method | Role |
|--------|------|
| `boing_getMediaBlobParams` | Caps, activation, DAS / pin fee notes |
| `boing_getBlobMeta` | Metadata or `null` |
| `boing_getBlob` | `{ meta, dataHex }` or `null` |
| `boing_getBlobDasSample` | Merkle sample for one chunk (+ parity hex) |

---

## Phases

| Phase | Status |
|-------|--------|
| 1 MVP (256 KiB) | **Merged** #53 |
| 2 1 MiB, Announce, P2P, blob fee, soft blobs_root | **Merged** #54 |
| 3 Header opt-in, DAS/pin stubs, 16 MiB target | **Merged** #55 (`1142be2`) |
| 4 Live 16 MiB, XOR chunking + DAS sample RPC, Forever pin fee | This PR (draft) |

---

## Env / CLI

```bash
BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT=1
# Optional:
# BOING_BLOBS_ROOT_HEADER_ACTIVATION_HEIGHT=1
# network_fee_config.json:
#   "blob_base_fee_per_kib": "0",
#   "blob_forever_pin_fee_per_kib": "0"
boing-node --media-blobs-activation-height 1 --data-dir ./data
```
