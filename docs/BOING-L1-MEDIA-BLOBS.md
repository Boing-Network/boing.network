# Boing L1 media blobs

> 👋 **Everyday users:** images / GIFs / short clips can live **on Boing**. Default live cap **16 MiB**.  
> 🛠️ **Developers:** PutBlob **21**, BlobAnnounce **22**, RenewMediaBlobPin **23**, PayBlobSample **24**, RegisterBlobSampleOffer **25**, SettleBlobSampleMatch **26**; DAS RPCs; SDK `mediaBlob.ts`.  
> 🛰️ **Operators:** `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`. Optional header: `BOING_BLOBS_ROOT_HEADER_ACTIVATION_HEIGHT`. Optional **gated** 256 MiB raise: `BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT` (**unset = stay 16 MiB — Fly-safe**). Fee JSON: `blob_*` fields as below.

Project design: Cursor store `docs/boing-l1-media-blobs.md`. Merges: #53–#57 · #65 (`79b6995`) · header boot-compat #66 · #67 (`16bd244`).

---

## Caps

| Knob | Value |
|------|------:|
| Live max (default) | **16 MiB** |
| Prefer Announce above | **1 MiB** |
| DAS chunk | 64 KiB |
| RS parity shards | **2** |
| Phase 6–8 design / activated max | **256 MiB** — only if Phase 7 cap activation is set |

**Do not** set `BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT` on public testnet until all validators agree — a mismatched cap breaks admission.

### Coordinated public-testnet 256 MiB activation (Phase 8 notes)

Default remains **unset → 16 MiB** (Fly-safe). Before enabling on public testnet:

1. Every validator / RPC node runs a binary that honors `media_blob_max_bytes_at`.
2. Agree on identical height `H`; set `BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT=H` (or `--media-blobs-phase7-cap-activation-height`) on **all** nodes; restart.
3. After tip ≥ `H`, confirm `boing_getMediaBlobParams`: `phase7CapActiveAtTip=true`, `effectiveMaxBytesAtTip=268435456`.
4. Disk/RAM must absorb 256 MiB Put/Announce + 64 KiB DAS chunks + RS parity=2.
5. Record the chosen `H` in `tools/boing-node-public-testnet.env.example` once activated.
6. Partial rolls fork admission — never enable on a subset of validators.

`boing_getMediaBlobParams.capActivationChecklist` returns these lines for operators.

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

Settle quote helpers (`boing_quoteBlobSampleSettle` / SDK) report `protocolSampleFee + askFee = total`. Ask fee is **not** SMT-escrowed yet; wallets may tip/transfer separately.

---

## Sampling / DAS

- **Consensus-replay book (Phase 8):** Register/Settle rebuild an on-chain offer book (`sample_offers_onchain.bin`) identically on honest full nodes — **not** SMT-resident yet  
- **Local gossip book (Phase 6):** `boing_putBlobSampleOffer` still available (`source: local`)  
- Match / list prefer on-chain, then local (`boing_matchBlobSampleOffer`, `boing_listBlobSampleOffers`)  
- DAS light-client batch: height-bound merkle samples; optional `vrfOutput` for VRF-bound indices (caller verifies ECVRF)  
- **Succinct proof envelope:** `StubBlake3V0` (BLAKE3 commitment — **not** a SNARK); `ReservedRealV1` rejects until wired. RPCs: batch embeds `succinctProof`; `boing_verifyBlobDasSuccinctProof`  

---

## Header `blobs_root`

Optional (`None` omitted before activation). Legacy decode keeps historical hashes (#66).

---

## Phases

| Phase | Status |
|-------|--------|
| 1–5 | **Merged** |
| 6 Sampling book / DAS LC / segments | **Merged** (#65 · `79b6995`) |
| 7 On-chain matcher / VRF DAS / gated cap | **Merged** (#67 · `16bd244`) |
| 8 Consensus-replay market / succinct envelope / activation notes | **This PR (draft)** |

### Still later

- Full SMT-resident order book + ask_fee escrow in consensus state  
- Real succinct DAS proofs (replace StubBlake3V0) + production VRF-bound LC clients wired to leader ECVRF  
- Coordinated public-testnet activation of the 256 MiB cap (checklist above; default stays unset)  

---

## Env

```bash
BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT=1
# BOING_BLOBS_ROOT_HEADER_ACTIVATION_HEIGHT=1
# Unset keeps 16 MiB (recommended for Fly / public testnet until coordinated):
# BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT=…
```
