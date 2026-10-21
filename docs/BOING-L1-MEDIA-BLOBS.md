# Boing L1 media blobs

> 👋 **Everyday users:** images / GIFs / short clips can live **on Boing**. Default live cap **16 MiB**.  
> 🛠️ **Developers:** PutBlob **21**, BlobAnnounce **22**, RenewMediaBlobPin **23**, PayBlobSample **24**, RegisterBlobSampleOffer **25**, SettleBlobSampleMatch **26**, CancelBlobSampleOffer **27**; DAS RPCs; SDK `mediaBlob.ts`.  
> 🛰️ **Operators:** `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`. Optional header: `BOING_BLOBS_ROOT_HEADER_ACTIVATION_HEIGHT`. Optional **gated** 256 MiB raise: `BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT` (**unset = stay 16 MiB — Fly-safe**). Fee JSON: `blob_*` fields as below.

Project design: Cursor store `docs/boing-l1-media-blobs.md`. Merges: #53–#57 · #65 (`79b6995`) · #66 · #67 (`16bd244`) · #68 (`d7efb37`).

---

## Caps

| Knob | Value |
|------|------:|
| Live max (default) | **16 MiB** |
| Prefer Announce above | **1 MiB** |
| DAS chunk | 64 KiB |
| RS parity shards | **2** |
| Phase 6–9 design / activated max | **256 MiB** — only if Phase 7 cap activation is set |

**Do not** set `BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT` on public testnet until all validators agree — a mismatched cap breaks admission.

### Coordinated public-testnet 256 MiB activation (ops notes)

Default remains **unset → 16 MiB** (Fly-safe). Checklist (also `boing_getMediaBlobParams.capActivationChecklist`):

1. Pre-flight: Fly/host disk quotas, RPC gateway body limits, explorer/indexer timeouts tolerate 256 MiB.
2. Every validator / RPC node runs the same binary that honors `media_blob_max_bytes_at`.
3. Roll order: deploy binary → confirm all on that build → set identical `H` → restart all → verify params → only then load-test large Puts.
4. After tip ≥ `H`: `phase7CapActiveAtTip=true`, `effectiveMaxBytesAtTip=268435456`.
5. Rollback: never partially unset after tip ≥ H on some nodes — requires coordinated freeze / emergency procedure.
6. Monitor admission rejects for size > 16 MiB before `H`; after `H`, watch OOM / RPC timeouts.
7. Record chosen `H` in `tools/boing-node-public-testnet.env.example` once activated.

---

## Tx types

| Variant | Index | Role |
|---------|------:|------|
| PutBlob | 21 | Inline publish |
| BlobAnnounce | 22 | Commit id+size; bytes via P2P/RPC |
| RenewMediaBlobPin | 23 | Extend `pin_until_height` (sidecar) |
| PayBlobSample | 24 | Pay protocol sample fee (scaffold) |
| RegisterBlobSampleOffer | 25 | Register offer; **escrow `ask_fee`** into system account |
| SettleBlobSampleMatch | 26 | Settle vs provider; protocol sample fee + **release escrowed ask** |
| CancelBlobSampleOffer | 27 | Provider cancel; **refund escrow** |

---

## Fees

```
Put/Announce: gas×retention + kib×blob_base_fee_per_kib
              + (Forever ? kib×blob_forever_pin_fee_per_kib : 0)
Renew:        gas_base + kib×extend_blocks×blob_pin_renew_fee_per_kib_block
Sample/Settle: gas_base + blob_sample_fee_per_chunk
Register/Cancel: gas_base only (ask_fee escrowed / refunded in SMT path)
```

Ask-fee escrow account: `BLAKE3("BOING_BLOB_SAMPLE_ASK_ESCROW_V1")` (`blob_sample_ask_escrow_account` / SDK `blobSampleAskEscrowAccount`).

---

## Sampling / DAS

- **SMT-resident book (Phase 9):** `SampleOfferRegistry` leaf in state root; Register/Settle/Cancel mutate it; persisted as `sample_offers_registry.bin`  
- Sidecar replay + local gossip books remain for discovery UX (`onChain` / `local`); `boing_listSmtBlobSampleOffers` reads consensus state  
- DAS light-client batch: height-bound merkle samples; optional `vrfOutput` or `useLeaderVrf` (`boing_getLeaderVrfOutput`)  
- RoundRobin leader election → no leader VRF (height seed); VRF mode uses ECVRF winner or `dummy_vrf_output`  
- **Succinct proof envelope:** StubBlake3V0 only (**not** a SNARK); ReservedRealV1 rejects  

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
| 8 Consensus-replay market / succinct envelope | **Merged** (#68 · `d7efb37`) |
| 9 SMT escrow + leader-VRF LC path | **This PR (draft)** |

### Still later

- Richer matching / serving fraud proofs if sample not delivered  
- Real succinct DAS proofs (replace StubBlake3V0)  
- Coordinated public-testnet activation of the 256 MiB cap (checklist above; default stays unset)  

---

## Env

```bash
BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT=1
# BOING_BLOBS_ROOT_HEADER_ACTIVATION_HEIGHT=1
# Unset keeps 16 MiB (recommended for Fly / public testnet until coordinated):
# BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT=…
```
