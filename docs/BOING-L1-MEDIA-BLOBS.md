# Boing L1 media blobs

Content-addressed media on L1: sidecar bytes + consensus metadata via PutBlob / BlobAnnounce.

> 🛰️ **Operators:** `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`. Optional header: `BOING_BLOBS_ROOT_HEADER_ACTIVATION_HEIGHT`. Optional **gated** 256 MiB raise: `BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT` (**unset = stay 16 MiB — Fly-safe**).

## Caps

| | |
|--|--|
| Live default | **16 MiB** (`MAX_MEDIA_BLOB_BYTES`) |
| Gated raise | **256 MiB** when tip ≥ `BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT` |

**Do not** set `BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT` on public testnet until all validators agree.

## Payload indices (media-related)

| Variant | Index | Role |
|---------|------:|------|
| PutBlob | 21 | Inline publish (≤ live cap) |
| BlobAnnounce | 22 | Id + size; bytes via gossip/sidecar |
| RenewMediaBlobPin | 23 | Extend pin horizon |
| PayBlobSample | 24 | Pay for one DAS chunk sample |
| RegisterBlobSampleOffer | 25 | On-chain sample offer |
| SettleBlobSampleMatch | 26 | Settle sample match |
| CancelBlobSampleOffer | 27 | Cancel + refund ask escrow |
| ChallengeBlobSampleServe | 28 | Open serve challenge (requires prior Settle; escrows bond+penalty) |
| RespondBlobSampleServe | 29 | Provider DAS merkle response (bond+penalty → provider) |
| SubmitSlashAppeal | 30 | Consensus appeal against a slash (evidence ≤ 4096 B); **snapshots electorate W** + **resolve_deadline** |
| ResolveSlashAppeal | 31 | Unattested stake-weighted vote (⌈2W/3⌉ vs **frozen W**); solo / unpinned / `|pinned|≤1` only; rejects after resolve_deadline |
| ResolveSlashAppealAttested | 32 | Ed25519 ballot + same frozen-W ⌈2W/3⌉ (**required** when `|pinned|>1`); same resolve window |

---

## Sampling / DAS / challenges

- SMT offer book + ask escrow (Phase 9)  
- Serve challenges (Phase 10–18): settle-before-challenge; bond/penalty; FailedServe burn + shared consensus `slashId`; **Submit/Resolve txs** restore provider balance (FailedServe) or stake (equivocation/liveness) on multi-validator nets; settle-receipt TTL prune  
- Equivocation/liveness **detection** remains node-local (evidence gossip + apply); Phase 15 seeds the consensus appeal book at slash time so 30/31/32 replay restore  
- Phase 16: equal-weight N-of-M (⌈2M/3⌉) over host-pinned `slash_appeal_resolvers`  
- Phase 17: **ResolveSlashAppeal (31)** is **stake-weighted** ⌈2W/3⌉ (weight = bonded stake at cast) over the same pinned set; one vote per voter; finalize/restore only at threshold. Unpinned / empty pin → effective W=1 (solo-dev one-shot). W=0 with a non-empty pin never finalizes.  
- Phase 18: **ResolveSlashAppealAttested (32)** carries a domain-separated Ed25519 ballot (`BOING_SLASH_APPEAL_RESOLVE_BALLOT_V1` over `chain_id|appeal_id|slash_id|approved|voter`); verified against `tx.sender`; ballots accumulate and are **verified individually** (thin attest — **not** BLS).  
- Phase 19: when host-pinned `|slash_appeal_resolvers| > 1`, **32 is required**; unattested **31** is rejected. Solo / unpinned / `|pinned|≤1` keep **31**. Historical unattested votes mid-tally **keep their weight**; new multi-pin casts must use **32**.  
- Phase 20: **SubmitSlashAppeal (30)** snapshots `electorate_w_snapshot` (Σ bonded stake over pinned resolvers at Submit) and a pin-set commitment; **Resolve (31/32)** uses that frozen W for ⌈2W/3⌉ (voter weight still at cast). Pin-set change vs snapshot rejects Resolve. Unpinned → snapshot W=1. W=0 at Submit stays Pending (no rewrite). Pre-V4 Pending rows use live-W fallback (sentinel).  
- Phase 21: **Submit** stores `resolve_deadline = tip + 1000`. **Resolve (31/32)** after deadline expires Pending → **Expired** (no vote; no restore) and rejects `ResolveWindowClosed`. Block housekeeping `expire_pending_appeals` cleans stuck Pending (W=0 / incomplete tallies).  
- Versioned `slash_appeals_registry.bin` (magic + u32 + bincode **V5**); V4 / V3 / V2 / legacy Phase 14–16 loads without required clear. Pre-V5 Pending derives `resolve_deadline` from `slash.appeal_deadline`.  
- Submit/Resolve/Attested (**30**/**31**/**32**) are **always admitted** (not gated on media-blobs activation); mempool may reject **31** early when `|pinned|>1`  
- Operator RPC `boing_submitSlashAppeal` / `boing_resolveSlashAppeal` is **deprecated** for FailedServe / equivocation / liveness when the consensus book owns the slash (prefer txs 30/32); refuses double credit after consensus reverse  
- Resolve is **ops/validator stake-weighted voting** (+ Ed25519 ballot when required), not SNARK verification of evidence  
- Succinct DAS: StubBlake3V0; ReservedRealV1 envelope checks; **verify rejects** until a real system is wired  
- Leader-VRF LC: `boing_getLeaderVrfOutput` / `useLeaderVrf`  

---

## Phases

| Phase | Status |
|-------|--------|
| 1–20 | **Merged** (#82) |
| 21 Resolve expiry + Pending cleanup | **This PR** |

### Still later

- True **BLS aggregate** of resolve ballots (dual-key registration + `blst`)  
- Real IPA/KZG/SNARK verify for ReservedRealV1 (flip `is_implemented`)  
- Coordinated public-testnet 256 MiB activation  

---

## Ops / migration (Phase 21)

| Item | Guidance |
|------|----------|
| `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT` | **Leave at 1200** (both Fly apps). Do not change. |
| `BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT` | **Leave unset** (16 MiB Fly-safe). |
| `slash_appeals_registry.bin` | **Do not clear.** Save V5; load V5 or migrate V4 / V3 / V2 / Phase 14–16 in memory. |
| Mid-tally at upgrade | Pre-V5 Pending appeals: derive `resolve_deadline` from `slash.appeal_deadline` (may be shorter than submit+1000). **New** Submits always write `tip + 1000`. |
| Clients | SDK **0.5.24**; no new encode API for 30/31/32 payloads (behavior is node-side). RPC may show `resolveDeadline` / status `Expired`. |
| Operators | Prefer Resolve before deadline; W=0 / idle committees will **Expired** instead of hanging forever. |
| Binary upgrade | Roll validators together (new deadline field + housekeeping expire). |
| BLS / SNARK / 256 MiB | **Not this phase** — still later. |

---

## Env

```bash
# Admission — already coordinated on Fly at 1200; do not change unless operators re-agree:
# BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT=1200

# Unset keeps 16 MiB (Fly / public testnet until coordinated):
# BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT=…
```
