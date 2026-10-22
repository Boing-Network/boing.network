# Boing L1 media blobs

> 👋 **Everyday users:** images / GIFs / short clips can live **on Boing**. Default live cap **16 MiB**.  
> 🛠️ **Developers:** PutBlob **21** … Cancel **27**, ChallengeServe **28**, RespondServe **29**, SubmitSlashAppeal **30**, ResolveSlashAppeal **31**; DAS RPCs; SDK `mediaBlob.ts`.  
> 🛰️ **Operators:** `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`. Optional header: `BOING_BLOBS_ROOT_HEADER_ACTIVATION_HEIGHT`. Optional **gated** 256 MiB raise: `BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT` (**unset = stay 16 MiB — Fly-safe**).

Project design: Cursor store `docs/boing-l1-media-blobs.md`. Merges: #53–#57 · #65 · #66 · #67 · #68 · #69 · #71 · #72 · #74 · #75.

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

Default **unset → 16 MiB** (Fly-safe). See `boing_getMediaBlobParams.capActivationChecklist`. Do not activate on Fly until coordinated.

---

## Tx types

| Variant | Index | Role |
|---------|------:|------|
| PutBlob | 21 | Inline publish |
| BlobAnnounce | 22 | Commit id+size; bytes via P2P/RPC |
| RenewMediaBlobPin | 23 | Extend pin horizon |
| PayBlobSample | 24 | Protocol sample fee |
| RegisterBlobSampleOffer | 25 | Escrow `ask_fee` + SMT offer |
| SettleBlobSampleMatch | 26 | Protocol fee + release ask escrow + settle receipt |
| CancelBlobSampleOffer | 27 | Refund ask escrow |
| ChallengeBlobSampleServe | 28 | Open serve challenge (requires prior Settle; escrows bond+penalty) |
| RespondBlobSampleServe | 29 | Provider DAS merkle response (bond+penalty → provider) |
| SubmitSlashAppeal | 30 | Consensus appeal against a slash (evidence ≤ 4096 B) |
| ResolveSlashAppeal | 31 | N-of-M committee vote (⌈2M/3⌉) over pinned resolvers; FailedServe **balance** or equivocation/liveness **stake** restore only when approve threshold met |

---

## Sampling / DAS / challenges

- SMT offer book + ask escrow (Phase 9)  
- Serve challenges (Phase 10–16): settle-before-challenge; bond/penalty; FailedServe burn + shared consensus `slashId`; **Submit/Resolve txs** restore provider balance (FailedServe) or stake (equivocation/liveness) on multi-validator nets; settle-receipt TTL prune  
- Equivocation/liveness **detection** remains node-local (evidence gossip + apply); Phase 15 seeds the consensus appeal book at slash time so 30/31 replay restore  
- Phase 16: **ResolveSlashAppeal (31)** is an N-of-M vote (⌈2M/3⌉ approve **or** reject) over host-pinned `slash_appeal_resolvers`; one vote per voter; finalize/restore only at threshold. When no pinned set, M=1 (solo-dev one-shot). Payload index unchanged.  
- Submit/Resolve (**30**/**31**) are **always admitted** (not gated on media-blobs activation)  
- Operator RPC `boing_submitSlashAppeal` / `boing_resolveSlashAppeal` is **deprecated** for FailedServe / equivocation / liveness when the consensus book owns the slash (prefer txs 30/31); refuses double credit after consensus reverse  
- Resolve is **ops/validator committee voting**, not SNARK verification of evidence  
- Succinct DAS: StubBlake3V0; ReservedRealV1 envelope checks; **verify rejects** until a real system is wired  
- Leader-VRF LC: `boing_getLeaderVrfOutput` / `useLeaderVrf`  

---

## Phases

| Phase | Status |
|-------|--------|
| 1–15 | **Merged** (#77) |
| 16 Committee threshold slash-appeal resolve | **This PR** |

### Still later

- Real IPA/KZG/SNARK verify for ReservedRealV1 (flip `is_implemented`)  
- Coordinated public-testnet 256 MiB activation  

---

## Env

```bash
# Admission — already coordinated on Fly at 1200; do not change unless operators re-agree:
# BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT=1200

# Unset keeps 16 MiB (Fly / public testnet until coordinated):
# BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT=…
```
