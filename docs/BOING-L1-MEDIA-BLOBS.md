# Boing L1 media blobs

> 👋 **Everyday users:** images / GIFs / short clips can live **on Boing**. Live cap **16 MiB** — still not multi-minute high-bitrate video.  
> 🛠️ **Developers:** PutBlob **21**, BlobAnnounce **22**, RenewMediaBlobPin **23**, PayBlobSample **24**; RPC `boing_getBlob*` / `boing_getBlobDasSample` / `boing_getBlobDasLightClientBatch`; node-local sample offer RPCs; SDK `mediaBlob.ts`.  
> 🛰️ **Operators:** `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`. Optional header verify: `BOING_BLOBS_ROOT_HEADER_ACTIVATION_HEIGHT`. Fee JSON: `blob_base_fee_per_kib`, `blob_forever_pin_fee_per_kib`, `blob_pin_renew_fee_per_kib_block`, `blob_sample_fee_per_chunk`.

Project design: Cursor store `docs/boing-l1-media-blobs.md`. Merges: #53 · #54 · #55 · #56 (`78a2cab`) · #57 (`e021831`). Header boot-compat: #66 (`0259581`).

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
| Phase 6 segment size | ≤ live 16 MiB each (`PHASE6_VIDEO_SEGMENT_TARGET_BYTES`) |

Honest: 16 MiB ≠ multi-minute 1080p. Use Announce + renewals + segment manifests for large Forever pins. **Do not** raise the live Put/Announce cap without an explicit activation gate.

---

## Tx types

| Variant | Index | Role |
|---------|------:|------|
| PutBlob | 21 | Inline publish |
| BlobAnnounce | 22 | Commit id+size; bytes via P2P/RPC |
| RenewMediaBlobPin | 23 | Extend `pin_until_height` (sidecar) |
| PayBlobSample | 24 | Pay protocol sample fee |

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

## Erasure / DAS / sampling

- **XOR** (Phase 4): one parity, one erasure  
- **Reed–Solomon** (Phase 5): `chunk_media_blob_rs` / `recover_media_blob_rs` with 2 parity shards  
- Single sample: `boing_getBlobDasSample` + merkle inclusion  
- **DAS light-client batch** (Phase 6): `boing_getBlobDasLightClientBatch` — height-bound deterministic merkle samples (`das_light_client_seed`); **not** VRF; **not** a succinct aggregate  
- **Sample market** (Phase 6): node-local offer book in the blob sidecar (`boing_putBlobSampleOffer` / `list` / `match`) + `MediaBlobSampleMarket` helpers. Pairs with on-chain `PayBlobSample` fees. **Not consensus** — no on-chain matcher yet  

---

## Segmented video path (Phase 6)

- `split_into_live_cap_segments` / SDK `splitIntoLiveCapSegments` — each segment ≤ **16 MiB**  
- `MediaBlobSegmentManifest` + `manifest_id` — ordered BlobIds; design total ≤ 256 MiB  
- Publish each segment with PutBlob/Announce under today’s live cap  

---

## Header `blobs_root`

Optional in bincode (`None` / omitted before activation). When header activation is on, producers set `Some(blobs_root_of_transactions)` (incl. `Some(ZERO)` for empty media) and import verifies. Legacy headers without the field decode as `None` so historical [`Block::hash`] values stay stable (#66).

---

## Phases

| Phase | Status |
|-------|--------|
| 1–4 | **Merged** (#53–#56) |
| 5 RS + renew/sample fees | **Merged** (#57 · `e021831`) |
| 6 Sampling market / DAS LC / segmented video | **This PR (draft)** — stronger than stubs; still not a full on-chain market |

### Phase 6 implemented here

| Item | Location |
|------|----------|
| Sample offer book + best-price match + expiry | `MediaBlobSampleMarket` + sidecar RPC |
| DAS light-client batch (merkle, height-bound) | `build_das_light_client_batch*` + `boing_getBlobDasLightClientBatch` |
| Segment split / manifest / id | `split_into_live_cap_segments`, `manifest_from_segment_bytes` |
| SDK | **0.5.9** helpers + client RPCs |

### Still later

- On-chain sample matcher / VRF-bound sampling  
- Succinct DAS light-client proofs  
- Raising the live blob cap / activating the 256 MiB design target  

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
