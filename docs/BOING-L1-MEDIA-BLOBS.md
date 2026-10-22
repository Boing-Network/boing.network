# Boing L1 media blobs

> 👋 **Everyday users:** images / GIFs / short clips can live **on Boing**. Default live cap **16 MiB**.  
> 🛠️ **Developers:** PutBlob **21** … Cancel **27**, ChallengeServe **28**, RespondServe **29**; DAS RPCs; SDK `mediaBlob.ts`.  
> 🛰️ **Operators:** `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`. Optional header: `BOING_BLOBS_ROOT_HEADER_ACTIVATION_HEIGHT`. Optional **gated** 256 MiB raise: `BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT` (**unset = stay 16 MiB — Fly-safe**).

Project design: Cursor store `docs/boing-l1-media-blobs.md`. Merges: #53–#57 · #65 · #66 · #67 · #68 · #69 · #71 · #72 (`d4debf3`).

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
| SettleBlobSampleMatch | 26 | Protocol fee + release ask escrow + **settle receipt** |
| CancelBlobSampleOffer | 27 | Refund ask escrow |
| ChallengeBlobSampleServe | 28 | Open serve challenge (**requires prior Settle**; escrows bond+penalty) |
| RespondBlobSampleServe | 29 | Provider DAS merkle response (**bond+penalty → provider**) |

---

## Sampling / DAS / challenges

- SMT offer book + ask escrow (Phase 9)  
- Serve challenges (Phase 10–12): settle-before-challenge; bond/penalty escrow; FailedServe → bond refund + penalty burn + consensus `slashId` mirrored to `boing_listSlashRecords` (`blobFailedServe`)  
- Succinct DAS: StubBlake3V0 implemented; ReservedRealV1 opaque envelope + structure checks; **verify rejects** until a real system is wired  
- Leader-VRF LC: `boing_getLeaderVrfOutput` / `useLeaderVrf`  

---

## Phases

| Phase | Status |
|-------|--------|
| 1–11 | **Merged** (#72 · `d4debf3`) |
| 12 Settle-before-challenge / FailedServe SlashRegistry audit | **This PR (draft)** |

### Still later

- Real IPA/KZG/SNARK verify for ReservedRealV1 (flip `is_implemented`)  
- Coordinated public-testnet 256 MiB activation  
- FailedServe appeal → native unburn (if ever desired)  

---

## Env

```bash
# Admission (Put/Announce/sample txs) — set identical H on every validator:
BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT=<tip+N>

# Unset keeps 16 MiB (Fly / public testnet until coordinated):
# BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT=…
```

### Fly admission activation (operators)

Secret/env name: **`BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`**. Set the **same** height on **boing-testnet-1** and **boing-testnet-2**. Choose `H = tip + N` (e.g. tip+50) so both apps restart before tip reaches H. Leave **`BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT` unset**. See store roadmap Activation checklist for flyctl commands.
