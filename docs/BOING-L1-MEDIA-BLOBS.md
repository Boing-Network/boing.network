# Boing L1 media blobs

> 👋 **Everyday users:** this is how images / GIFs / short clips can live **on Boing itself** instead of only on HTTPS or IPFS. v1 is a small, honest cap — not “50 GB movies on L1.”  
> 🛠️ **Developers:** `TransactionPayload::PutBlob` (index **21**), RPC `boing_getBlob` / `boing_getBlobMeta` / `boing_getMediaBlobParams`, SDK `mediaBlob.ts`.  
> 🛰️ **Operators:** enable with `BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT` or `--media-blobs-activation-height` (all nodes must agree). Sidecar path: `{data_dir}/blobs/`.

**Contrast:** the [interface blob store](INTERFACE-REGISTRY.md) packs small JSON into **contract SSTORE** (~24 KiB). Media blobs use a **separate size-scaled fee schedule** and a **node sidecar** — do not stuff art into SSTORE.

Project design / roadmap (Nico): store doc `docs/boing-l1-media-blobs.md` in the Boing Network Cursor project.

---

## MVP caps (v1)

| Knob | Value |
|------|------:|
| Max payload | **256 KiB** (`MAX_MEDIA_BLOB_BYTES`) |
| Max `content_type` | 64 UTF-8 bytes |
| Retention | `Prunable { ttl_blocks }` or `Forever` |
| Max prunable TTL | 1_000_000 blocks |
| Blob id | `BLAKE3(data)` (32 bytes) |
| URI | `boing://blob/0x{64 hex}` |

Raise caps in later phases; video-scale needs a data plane (announce + gossip / DAS), not bigger PutBlob txs alone.

---

## Fee model (sketch)

```
blob_gas = 21_000 + ceil(size / 32) * 16
effective_gas = blob_gas * (Forever ? 10 : 1)
fee = network_fee_amount_in(effective_gas, PutBlob, NetworkFeePolicy)
```

Same BOING fee path as other txs (`ceil(gas / 21_000)` whole-BOING era, or base-unit price). **Not** SSTORE (20_000 gas/word).

| Retention | Who pays ongoing storage (MVP) |
|-----------|--------------------------------|
| **Prunable** | Upfront size×1 fee; nodes may delete after `included_height + ttl_blocks` |
| **Forever** | Upfront size×10 fee = prepaid best-effort on full nodes — **not** a cryptographic eternity guarantee until pin markets (Phase 3) |

### What makes gas fees lessen on Boing?

1. **Governance / fee policy** — lower schedule or `NetworkFeePolicy` extras (testnet operator RPC / governance).  
2. **Efficiency** — avoid SSTORE for media; keep contracts lean.  
3. **Separate blob fee market** — data gas must not bid against swap / transfer gas (this feature’s point).  
4. **Congestion** — later: independent blob base fee (EIP-4844-style) + execution base fee.

---

## NFT binding

Reference NFT `metadata_hash` is an opaque 32-byte word ([BOING-REFERENCE-NFT.md](BOING-REFERENCE-NFT.md)):

1. **Direct:** `metadata_hash = BlobId` when the token *is* the media file.  
2. **Indirect (recommended):** JSON with `"image": "boing://blob/0x…"`, `metadata_hash = BLAKE3(JSON)`. Resolve via `boing_getBlob`.

Does **not** block Finance marketplace `mint_batch` work — hashes stay the same; media fetch gains an on-chain URI when blobs are enabled.

---

## RPC

| Method | Role |
|--------|------|
| `boing_getMediaBlobParams` | Caps, activation, gas knobs |
| `boing_getBlobMeta` | Metadata or `null` |
| `boing_getBlob` | `{ meta, dataHex }` or `null` |

Publish: sign a `PutBlob` tx and `boing_submitTransaction` (same as any payload). Feature off until activation height.

---

## Phases

| Phase | Status |
|-------|--------|
| 0 Design | This doc + project store design |
| 1 MVP PutBlob + sidecar + RPC + SDK | Scaffolding in this PR (256 KiB) |
| 2 Announce + P2P data plane, optional `blobs_root`, 1 MiB+ | Next |
| 3 Video-scale DA, pin rents, independent blob base fee | Later |

---

## Env / CLI

```bash
BOING_MEDIA_BLOBS_ACTIVATION_HEIGHT=1
# or
boing-node --media-blobs-activation-height 1 --data-dir ./data
```

Unset = PutBlob rejected (safe default for existing testnet until operators coordinate).
