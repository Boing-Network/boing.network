# Boing L1 media blobs

> 👋 **Everyday users:** images / GIFs / short clips can live **on Boing**. Live cap **16 MiB** — still not multi-minute high-bitrate video.  
> 🛠️ **Developers:** PutBlob **21**, BlobAnnounce **22**, RenewMediaBlobPin **23**, PayBlobSample **24**; RPC `boing_getBlob*` / `boing_getBlobDasSample`; SDK `mediaBlob.ts`.  
> 🛰️ **Operators:** `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`. Optional header verify: `BOING_BLOBS_ROOT_HEADER_ACTIVATION_HEIGHT`. Fee JSON: `blob_base_fee_per_kib`, `blob_forever_pin_fee_per_kib`, `blob_pin_renew_fee_per_kib_block`, `blob_sample_fee_per_chunk`.

Project design: Cursor store `docs/boing-l1-media-blobs.md`. Merges: #53 · #54 · #55 · #56 (`78a2cab`).

---

## Caps

| Knob | Value |
|------|------:|
| Live max | **16 MiB** |
| Prefer Announce above | **1 MiB** |
| DAS chunk | 64 KiB |
| RS parity shards | **2** (recover up to 2 missing data shards) |
| Video pin horizon hint | ~150_000 blocks (docs only) |

Honest: 16 MiB ≠ multi-minute 1080p. Use Announce + renewals for large Forever pins.

---

## Tx types

| Variant | Index | Role |
|---------|------:|------|
| PutBlob | 21 | Inline publish |
| BlobAnnounce | 22 | Commit id+size; bytes via P2P/RPC |
| RenewMediaBlobPin | 23 | Extend `pin_until_height` (sidecar) |
| PayBlobSample | 24 | Pay sample fee (market scaffold) |

---

## Fees

```
Put/Announce: gas×retention + kib×blob_base_fee_per_kib
              + (Forever ? kib×blob_forever_pin_fee_per_kib : 0)
Renew:        gas_base + kib×extend_blocks×blob_pin_renew_fee_per_kib_block
Sample:       gas_base + blob_sample_fee_per_chunk
```

All KiB levies `0` = off.

---

## Erasure / DAS

- **XOR** (Phase 4): one parity, one erasure  
- **Reed–Solomon** (Phase 5): `chunk_media_blob_rs` / `recover_media_blob_rs` with 2 parity shards  
- Light-client sample: `boing_getBlobDasSample` + merkle inclusion  
- Paid sampling market: fee hook only (`PayBlobSample`) — not a full matcher yet

---

## Header `blobs_root`

Always present in bincode as a `Hash` (ZERO before activation). When header activation is on, producers set `blobs_root_of_transactions` and import verifies. Legacy headers without the field decode as ZERO.

---

## Phases

| Phase | Status |
|-------|--------|
| 1–4 | **Merged** (#53–#56) |
| 5 RS + renew/sample fees | This PR (draft) |

---

## Env

```bash
BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT=1
# BOING_BLOBS_ROOT_HEADER_ACTIVATION_HEIGHT=1
# network_fee_config.json:
#   "blob_forever_pin_fee_per_kib": "0",
#   "blob_pin_renew_fee_per_kib_block": "0",
#   "blob_sample_fee_per_chunk": "0"
```
