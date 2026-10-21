# Boing L1 media blobs

> 👋 **Everyday users:** images / GIFs / short clips can live **on Boing**. Default live cap **16 MiB**.  
> 🛠️ **Developers:** PutBlob **21** … Cancel **27**, ChallengeServe **28**, RespondServe **29**; DAS RPCs; SDK `mediaBlob.ts`.  
> 🛰️ **Operators:** `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`. Optional header: `BOING_BLOBS_ROOT_HEADER_ACTIVATION_HEIGHT`. Optional **gated** 256 MiB raise: `BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT` (**unset = stay 16 MiB — Fly-safe**).

Project design: Cursor store `docs/boing-l1-media-blobs.md`. Merges: #53–#57 · #65 · #66 · #67 · #68 · #69 (`0747903`).

---

## Caps

| Knob | Value |
|------|------:|
| Live max (default) | **16 MiB** |
| Prefer Announce above | **1 MiB** |
| DAS chunk | 64 KiB |
| RS parity shards | **2** |
| Design / activated max | **256 MiB** — only if Phase 7 cap activation is set |

**Do not** set `BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT` on public testnet until all validators agree.

### Coordinated public-testnet 256 MiB activation

Default **unset → 16 MiB** (Fly-safe). See `boing_getMediaBlobParams.capActivationChecklist` for pre-flight, roll order, rollback, and monitoring. Do not activate on Fly until coordinated.

---

## Tx types

| Variant | Index | Role |
|---------|------:|------|
| PutBlob | 21 | Inline publish |
| BlobAnnounce | 22 | Commit id+size; bytes via P2P/RPC |
| RenewMediaBlobPin | 23 | Extend pin horizon |
| PayBlobSample | 24 | Protocol sample fee |
| RegisterBlobSampleOffer | 25 | Escrow `ask_fee` + SMT offer |
| SettleBlobSampleMatch | 26 | Protocol fee + release ask escrow |
| CancelBlobSampleOffer | 27 | Refund ask escrow |
| ChallengeBlobSampleServe | 28 | Open serve-availability challenge |
| RespondBlobSampleServe | 29 | Provider DAS merkle response |

---

## Sampling / DAS / challenges

- SMT offer book + ask escrow (Phase 9)  
- Serve challenges (Phase 10): window **256** blocks; timeout → `failedServe` signal (**no automatic slash yet**)  
- Succinct DAS: StubBlake3V0 implemented; ReservedRealV1 opaque envelope **verify rejects** until a real system is wired (`boing_getBlobDasSuccinctProofPlan`)  
- Leader-VRF LC: `boing_getLeaderVrfOutput` / `useLeaderVrf`  

---

## Phases

| Phase | Status |
|-------|--------|
| 1–9 | **Merged** (#69 · `0747903`) |
| 10 Serve challenge / reserved DAS path | **This PR (draft)** |

### Still later

- Automatic slash / bond on failed serve  
- Real IPA/KZG/SNARK verify for ReservedRealV1  
- Coordinated public-testnet 256 MiB activation  

---

## Env

```bash
BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT=1
# Unset keeps 16 MiB (Fly / public testnet until coordinated):
# BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT=…
```
