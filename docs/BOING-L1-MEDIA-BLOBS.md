# Boing L1 media blobs

> 👋 **Everyday users:** images / GIFs / short clips can live **on Boing**. Default live cap **16 MiB**.  
> 🛠️ **Developers:** PutBlob **21**, BlobAnnounce **22**, RenewMediaBlobPin **23**, PayBlobSample **24**, RegisterBlobSampleOffer **25**, SettleBlobSampleMatch **26**; DAS RPCs; SDK `mediaBlob.ts`.  
> 🛰️ **Operators:** `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`. Optional header: `BOING_BLOBS_ROOT_HEADER_ACTIVATION_HEIGHT`. Optional **gated** 256 MiB raise: `BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT` (**unset = stay 16 MiB — Fly-safe**). Fee JSON: `blob_*` fields as below.

Project design: Cursor store `docs/boing-l1-media-blobs.md`. Merges: #53–#57 · #65 (`79b6995`) · header boot-compat #66.

---

## Caps

| Knob | Value |
|------|------:|
| Live max (default) | **16 MiB** |
| Prefer Announce above | **1 MiB** |
| DAS chunk | 64 KiB |
| RS parity shards | **2** |
| Phase 6/7 design / activated max | **256 MiB** — only if Phase 7 cap activation is set |

**Do not** set `BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT` on public testnet until all validators agree — a mismatched cap breaks admission.

---

## Tx types

| Variant | Index | Role |
|---------|------:|------|
| PutBlob | 21 | Inline publish |
| BlobAnnounce | 22 | Commit id+size; bytes via P2P/RPC |
| RenewMediaBlobPin | 23 | Extend `pin_until_height` (sidecar) |
| PayBlobSample | 24 | Pay protocol sample fee (scaffold) |
| RegisterBlobSampleOffer | 25 | On-chain sample offer (provider = sender) |
| SettleBlobSampleMatch | 26 | Settle match vs provider; sample fee |

---

## Fees

```
Put/Announce: gas×retention + kib×blob_base_fee_per_kib
              + (Forever ? kib×blob_forever_pin_fee_per_kib : 0)
Renew:        gas_base + kib×extend_blocks×blob_pin_renew_fee_per_kib_block
Sample/Settle: gas_base + blob_sample_fee_per_chunk
Register:     gas_base only (ask_fee is offer metadata)
```

---

## Sampling / DAS

- Node-local offer book (Phase 6 RPCs) still available  
- **On-chain path (Phase 7):** `RegisterBlobSampleOffer` ingested into sidecar; `SettleBlobSampleMatch` pays sample fee and drops the offer  
- DAS light-client batch: height-bound merkle samples  
- **VRF-bound seed** helpers (`das_light_client_seed_from_vrf`) — caller must verify ECVRF; not auto-wired into consensus leader VRF yet  
- **Succinct commitment stub** (`das_succinct_commitment_stub`) — BLAKE3 placeholder, **not** a SNARK  

---

## Header `blobs_root`

Optional (`None` omitted before activation). Legacy decode keeps historical hashes (#66).

---

## Phases

| Phase | Status |
|-------|--------|
| 1–5 | **Merged** |
| 6 Sampling book / DAS LC / segments | **Merged** (#65 · `79b6995`) |
| 7 On-chain matcher / VRF DAS / gated cap | **This PR (draft)** |

### Still later

- Full on-chain discovery/matching market (order book in consensus state)  
- Real succinct DAS proofs + VRF-bound sampling in production LC clients  
- Coordinated public-testnet activation of the 256 MiB cap  

---

## Env

```bash
BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT=1
# BOING_BLOBS_ROOT_HEADER_ACTIVATION_HEIGHT=1
# Unset keeps 16 MiB (recommended for Fly / public testnet until coordinated):
# BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT=…
```
