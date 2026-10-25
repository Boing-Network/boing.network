# Boing L1 media blobs

Content-addressed media on L1: sidecar bytes + consensus metadata via PutBlob / BlobAnnounce.

> 🛰️ **Operators:** `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT`. Optional header: `BOING_BLOBS_ROOT_HEADER_ACTIVATION_HEIGHT`. Optional **gated** 256 MiB raise: `BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT` (**unset = stay 16 MiB — Fly-safe**). Optional BLS multi-pin committee flip: `BOING_MEDIA_BLOBS_BLS_COMMITTEE_FLIP_HEIGHT` (**unset = keep Ed25519 tx 32 — Fly-safe**).

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
| ResolveSlashAppealAttested | 32 | Ed25519 ballot + same frozen-W ⌈2W/3⌉ (**required** when `|pinned|>1` until BLS flip); same resolve window; **V2** binds pin-set |
| ResolveSlashAppealBlsAggregate | 33 | Self-contained BLS aggregate resolve; optional until flip; **required** for multi-pin when flip active |

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
- Phase 22: new **32** casts sign ballot domain **V2** (`BOING_SLASH_APPEAL_RESOLVE_BALLOT_V2`) binding Submit-frozen `pin_set_commitment`; cast + audit dual-verify historical **V1**. Read-only RPC **`boing_reverifySlashAppealBallots`** batch-checks stored ballots (ok / invalid / unattested), pin-set match, and registry commitment — **no state mutation**.  
- Phase 23: block housekeeping **`prune_expired_appeals`** drops **Expired** rows when `tip > resolve_deadline + 4096` (mirrors settle-receipt TTL); slash records stay; Pending/Approved/Rejected untouched. **`boing_listSlashRecords`** gains `appealStatus` / `includeExpired` (default true); re-verify gains `includeExpired` (default **false**).  
- Phase 24: block housekeeping **`prune_terminal_challenges`** drops **Responded** / settled **FailedServe** when `tip > deadline + 4096` (frees soft-cap 256); Open and unsettled FailedServe stay. **`boing_listBlobSampleServeChallenges`** gains `status` / `includeTerminal`; **`boing_getBlobSampleServeChallenge`** looks up by key.  
- Phase 25: block housekeeping **`prune_resolved_appeals`** drops **Approved** / **Rejected** when `tip > resolve_deadline + 4096`; slash records stay; Pending untouched. **`boing_listSlashRecords`** / **`boing_reverifySlashAppealBallots`** gain `includeResolved` (list default **true**; re-verify default **false**).  
- Phase 26: **BLS aggregate scaffolding** — dual-key types (`AccountId` + BLS12-381 G1 pubkey 48 B; G2 sig/aggregate 96 B), domain-separated aggregate ballot preimage, `phase26_validate_dual_key_set`, and aggregate/verify APIs that return **`NotWired`** (`PHASE26_BLS_AGGREGATE_WIRED=false`). Live **32** remains Ed25519 verify-each.  
- Phase 27: **Wire `blst` (`min_pk`)** — real sign / aggregate / `fast_aggregate_verify`; host-local **`bls_dual_key_registry.bin`** (BDKR v1; **not** slash V5 / not state-root); RPC `boing_listBlsDualKeys` / `boing_registerBlsDualKey` / `boing_verifyBlsAggregate`; `PHASE27_BLS_AGGREGATE_WIRED=true` for library+RPC. **Live tx 32 stays Ed25519 verify-each** (no committee flip yet).  
- Phase 28: **BLS dual-key proof-of-possession** — chain-bound PoP required on `boing_registerBlsDualKey` (`blsPop` 96 B + `chainId`); `PHASE28_BLS_POP_REQUIRED=true`. PoP checked at register (BDKR v1 unchanged; signature not persisted). **Live tx 32 stays Ed25519 verify-each**.  
- Phase 29: **BLS aggregate resolve payload + pin-set signer bitset** — library encode/decode/verify + dry-run RPC `boing_verifyBlsAggregateResolve`; reserved tx index **33**; `PHASE29_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED=false` (dual-keys still host-local). **Live tx 32 stays Ed25519 verify-each**.  
- Phase 30: **Self-contained BLS G1 pubkey list** on aggregate-resolve + `TransactionPayload::ResolveSlashAppealBlsAggregate` (**33**) wired; library/RPC verify without host registry; `PHASE30_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED=false` (mempool/VM reject). **Live tx 32 stays Ed25519 verify-each**.  
- Phase 31: **Consensus-admit / execute tx 33** — self-contained verify → stake-weighted cast for each bitset signer; `PHASE31_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED=true`. **Multi-pin committee default stays Ed25519 32** (`PHASE31_BLS_LIVE_TX32_STILL_ED25519=true`).  
- Phase 32: **Opt-in BLS multi-pin committee flip** — `BOING_MEDIA_BLOBS_BLS_COMMITTEE_FLIP_HEIGHT` / `--media-blobs-bls-committee-flip-height`. **Unset** ⇒ identical to Phase 31. When tip ≥ flip height and `|pinned|>1`, reject **31**/**32** and require **33**; solo (`|pinned|≤1`) keeps Ed25519. Historical tx **32** before flip height still replays. `PHASE32_BLS_LIVE_TX32_STILL_ED25519=true` compile-time default; RPC `blsLiveTx32StillEd25519` is **dynamic** (`!blsCommitteeFlipActiveAtTip`).  
- Phase 33: **ReservedRealV1 IPA/KZG/SNARK scaffolding** — `DasRealVerifierBackend` (`ipaV1` / `kzgV1` / `snarkV1`); `PHASE33_RESERVED_REAL_VERIFIER_WIRED=false`; stub verify APIs envelope-check then return honest **`NotWired`**. **`DasSuccinctProofKind::ReservedRealV1.is_implemented()` stays false**. Builds on Phase 32; no arkworks/halo2 deps.
- Versioned `slash_appeals_registry.bin` (magic + u32 + bincode **V5**); V4 / V3 / V2 / legacy Phase 14–16 loads without required clear. Pre-V5 Pending derives `resolve_deadline` from `slash.appeal_deadline`. **Phase 22–32 do not bump slash to V6** (dual-keys use a separate host-local file).  
- Submit/Resolve/Attested/BlsAggregate (**30**/**31**/**32**/**33**) are **always admitted** when consensus-wired (not gated on media-blobs activation); mempool may reject **31** early when `|pinned|>1` (and **32** when flip active)  
- Operator RPC `boing_submitSlashAppeal` / `boing_resolveSlashAppeal` is **deprecated** for FailedServe / equivocation / liveness when the consensus book owns the slash (prefer txs 30/32/33); refuses double credit after consensus reverse  
- Resolve is **ops/validator stake-weighted voting** (+ Ed25519 ballot when required), not SNARK verification of evidence  
- Succinct DAS: StubBlake3V0; ReservedRealV1 envelope checks; **verify rejects** until a real system is wired  
- Leader-VRF LC: `boing_getLeaderVrfOutput` / `useLeaderVrf`  

---

## Phases

| Phase | Status |
|-------|--------|
| 1–31 | **Merged** (#97) |
| 32 Opt-in BLS multi-pin committee flip (32→33) | **This PR** |

### Remaining after Phase 33

- Optional Ed25519 account-side binding on a consensus dual-key / identity tx  
- Wire a real IPA/KZG/SNARK verifier behind `ReservedRealV1` and flip `is_implemented` only when production-ready  
- Coordinated **256 MiB** activation (`BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT`) — do not set on Fly until operators agree  

---

## Ops / migration (Phase 32–33)

| Item | Guidance |
|------|----------|
| `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT` | **Leave at 1200** (both Fly apps). Do not change. |
| `BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT` | **Leave unset** (16 MiB Fly-safe). |
| `BOING_MEDIA_BLOBS_BLS_COMMITTEE_FLIP_HEIGHT` | **Leave unset** by default (Phase 31 Ed25519 **32** multi-pin default). |
| `slash_appeals_registry.bin` | **Do not clear.** Stay **V5**. |
| `bls_dual_key_registry.bin` | Host-local BDKR v1; PoP still required on register. |
| `PHASE32_BLS_AGGREGATE_SUBMIT_CONSENSUS_WIRED` | **true** — tx **33** admitted/executed. |
| `PHASE32_BLS_LIVE_TX32_STILL_ED25519` | **true** compile-time default until flip; RPC field is dynamic. |
| Clients | SDK **0.5.35**; flip helpers + encode/submit tx 33. |
| Operators | Prefer **32** until coordinated flip; set identical flip height on all validators when ready. |
| Account-side binding / SNARK / 256 MiB | **Not this phase** — listed under remaining. |

---

## Env

```bash
# Admission — already coordinated on Fly at 1200; do not change unless operators re-agree:
# BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT=1200

# Unset keeps 16 MiB (Fly / public testnet until coordinated):
# BOING_MEDIA_BLOBS_PHASE7_CAP_ACTIVATION_HEIGHT=…

# Unset keeps multi-pin Ed25519 tx 32 (Phase 31); set only after coordinated BLS readiness:
# BOING_MEDIA_BLOBS_BLS_COMMITTEE_FLIP_HEIGHT=…

# Permission-gated raw download (recommended for public RPC / explorer operators):
# GET /v1/media/blob?id=<blobId> returns Content-Type bytes (not JSON hex).
# Auth when BOING_OPERATOR_RPC_TOKEN and/or BOING_BLOB_DOWNLOAD_TOKENS are set:
#   X-Boing-Operator | X-Boing-Blob-Download | Authorization: Bearer <token>
# BOING_BLOB_DOWNLOAD_TOKENS=…
# Optional: BOING_BLOB_GET_REQUIRE_AUTH=1 also gates JSON-RPC boing_getBlob (default off).
```

## Permission-gated download

Large `PutBlob` bodies are stubbed in explorer block JSON (`include_code=false`) so public RPC stays under the ~6 MiB gateway cap. Operators who need the raw bytes use:

| Path | Role |
|------|------|
| `GET /v1/media/blob?id={blobId}` | Raw attachment; auth when download/operator tokens are configured |
| `boing_getBlobMeta` | Public metadata (size, contentType, publisher, …) |
| `boing_getBlob` | `{ meta, dataHex }` — optional auth via `BOING_BLOB_GET_REQUIRE_AUTH` |

Explorer (boing.observer) exposes a same-origin download UI that checks a user-presented download token against server `BOING_BLOB_DOWNLOAD_TOKENS` / `BOING_OPERATOR_RPC_TOKEN`, then streams from the node over Fly-direct (not the public gateway).

**Today those tokens are network-operator secrets** (Fly / Worker), not per-blob deployer credentials. Deployer-owned grant/revoke ACL is designed as **Phase 34** — see [HANDOFF_BLOB_DOWNLOAD_ACL.md](./HANDOFF_BLOB_DOWNLOAD_ACL.md).
