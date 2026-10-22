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
| SubmitSlashAppeal | 30 | Consensus appeal against a slash (evidence ≤ 4096 B) |
| ResolveSlashAppeal | 31 | Unattested stake-weighted vote (⌈2W/3⌉); historical / solo |
| ResolveSlashAppealAttested | 32 | Ed25519 ballot + same stake-weighted ⌈2W/3⌉ (prefer for committee) |

---

## Sampling / DAS / challenges

- SMT offer book + ask escrow (Phase 9)  
- Serve challenges (Phase 10–18): settle-before-challenge; bond/penalty; FailedServe burn + shared consensus `slashId`; **Submit/Resolve txs** restore provider balance (FailedServe) or stake (equivocation/liveness) on multi-validator nets; settle-receipt TTL prune  
- Equivocation/liveness **detection** remains node-local (evidence gossip + apply); Phase 15 seeds the consensus appeal book at slash time so 30/31/32 replay restore  
- Phase 16: equal-weight N-of-M (⌈2M/3⌉) over host-pinned `slash_appeal_resolvers`  
- Phase 17: **ResolveSlashAppeal (31)** is **stake-weighted** ⌈2W/3⌉ (weight = bonded stake at cast) over the same pinned set; one vote per voter; finalize/restore only at threshold. Unpinned / empty pin → effective W=1 (solo-dev one-shot). W=0 with a non-empty pin never finalizes.  
- Phase 18: **ResolveSlashAppealAttested (32)** carries a domain-separated Ed25519 ballot (`BOING_SLASH_APPEAL_RESOLVE_BALLOT_V1` over `chain_id|appeal_id|slash_id|approved|voter`); verified against `tx.sender`; ballots accumulate and are **verified individually** (thin attest — **not** BLS). Keep **31** for historical replay and unattested solo casts. Prefer **32** for committee nets.  
- Versioned `slash_appeals_registry.bin` (magic + u32 + bincode **V3**); V2 / legacy Phase 14–16 loads without required clear. Mid-tally V2 votes keep weight with `ballot_signature=None`.  
- Submit/Resolve/Attested (**30**/**31**/**32**) are **always admitted** (not gated on media-blobs activation)  
- Operator RPC `boing_submitSlashAppeal` / `boing_resolveSlashAppeal` is **deprecated** for FailedServe / equivocation / liveness when the consensus book owns the slash (prefer txs 30/32); refuses double credit after consensus reverse  
- Resolve is **ops/validator stake-weighted voting** (+ optional Ed25519 ballot), not SNARK verification of evidence  
- Succinct DAS: StubBlake3V0; ReservedRealV1 envelope checks; **verify rejects** until a real system is wired  
- Leader-VRF LC: `boing_getLeaderVrfOutput` / `useLeaderVrf`  

---

## Phases

| Phase | Status |
|-------|--------|
| 1–17 | **Merged** (#79) |
| 18 Ed25519 attested slash-appeal resolve + registry V3 | **This PR** |

### Still later

- True **BLS aggregate** of resolve ballots  
- Real IPA/KZG/SNARK verify for ReservedRealV1 (flip `is_implemented`)  
- Coordinated public-testnet 256 MiB activation  

---

## Ops / migration (Phase 18)

| Item | Guidance |
|------|----------|
| `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT` | **Leave at 1200** (both Fly apps). Do not change. |
| `BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT` | **Leave unset** (16 MiB Fly-safe). |
| `slash_appeals_registry.bin` | **Do not clear.** Node loads V3 or migrates V2 / legacy bare bincode and rewrites V3 on next save. |
| Mid-tally at upgrade | Survives; pre-upgrade votes keep weight with `ballot_signature=None`. New casts via **32** attach sigs. |
| Clients | Prefer **32** + SDK **0.5.21** `encodeResolveSlashAppealAttested` for committee nets; **31** still works. |
| Pinned resolvers | Prefer accounts with **non-zero bonded stake** (W=0 never finalizes). Ballot key = resolver AccountId. |
| Binary upgrade | Roll all validators together (commitment domain V3 + payload variant 32). |
| BLS | **Not this phase** — still later after attest MVP is live. |

---

## Env

```bash
# Admission — already coordinated on Fly at 1200; do not change unless operators re-agree:
# BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT=1200

# Unset keeps 16 MiB (Fly / public testnet until coordinated):
# BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT=…
```
