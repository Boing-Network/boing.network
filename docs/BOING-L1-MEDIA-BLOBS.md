# Boing L1 media blobs

> 👋 **Everyday users:** images / GIFs / short clips can live **on Boing**. Live cap **16 MiB** — still not multi-minute high-bitrate video.  
> 🛠️ **Developers:** PutBlob **21**, BlobAnnounce **22**, RenewMediaBlobPin **23**, PayBlobSample **24**; RPC `boing_getBlob*` / `boing_getBlobDasSample`; SDK `mediaBlob.ts`. Phase 6 scaffolds: sampling-market match, DAS light-client batch, segment manifests (`blobs_phase6` / SDK helpers).  
> 🛰️ **Operators:** `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`. Optional header verify: `BOING_BLOBS_ROOT_HEADER_ACTIVATION_HEIGHT`. Fee JSON: `blob_base_fee_per_kib`, `blob_forever_pin_fee_per_kib`, `blob_pin_renew_fee_per_kib_block`, `blob_sample_fee_per_chunk`.

Project design: Cursor store `docs/boing-l1-media-blobs.md`. Merges: #53 · #54 · #55 · #56 (`78a2cab`) · #57 (`e021831`).

---

## Caps

| Knob | Value |
|------|------:|
| Live max | **16 MiB** |
| Prefer Announce above | **1 MiB** |
| DAS chunk | 64 KiB |
| RS parity shards | **2** (recover up to 2 missing data shards) |
| Video pin horizon hint | ~150_000 blocks (docs only) |
| Phase 6 design target | **256 MiB** total via segments (`PHASE6_TARGET_MAX_MEDIA_BLOB_BYTES`) — **not live** |

Honest: 16 MiB ≠ multi-minute 1080p. Use Announce + renewals + (Phase 6) segment manifests for large Forever pins.

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
- Paid sampling market: fee hook (`PayBlobSample`) + Phase 6 **offer/match** helpers (`match_sample_offer`) — not a full on-chain matcher yet  
- DAS light-client batch: `MediaBlobDasLightClientBatch` / `das_light_client_sample_indices` (merkle samples only; no succinct aggregate yet)

---

## Header `blobs_root`

Optional in bincode (`None` / omitted before activation). When header activation is on, producers set `Some(blobs_root_of_transactions)` (incl. `Some(ZERO)` for empty media) and import verifies. Legacy headers without the field decode as `None` so historical [`Block::hash`] values stay stable.

---

## Phases

| Phase | Status |
|-------|--------|
| 1–4 | **Merged** (#53–#56) |
| 5 RS + renew/sample fees | **Merged** (#57 · `e021831`) |
| 6 Sampling market / DAS LC / segmented video | **Scaffold** (this PR — draft) |

### Phase 6 scaffold (not activated)

| Item | Location |
|------|----------|
| Sample offer / match | `blobs_phase6::MediaBlobSampleOffer` / `match_sample_offer` |
| DAS light-client batch verify | `MediaBlobDasLightClientBatch::validate_structure` |
| Segmented video manifest | `MediaBlobSegmentManifest` + `PHASE6_TARGET_*` |
| SDK | `matchSampleOffer`, `validateSegmentManifest`, `dasLightClientSampleIndices` (0.5.9) |

Still later: on-chain matcher, VRF-bound sampling, succinct proofs, raising the live cap.

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
