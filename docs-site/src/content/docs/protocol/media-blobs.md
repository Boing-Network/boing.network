---
title: Media blobs
description: L1 PutBlob / announce path, PHASE7 256 MiB cap, and download ACL notes for public testnet.
---

Boing stores content-addressed **media blobs** on L1 (NFT media and related workloads). Admission and caps are height-gated; clients should call **`boing_getMediaBlobParams`** before large puts.

## Live snapshot (public testnet)

From a probe of `boing_getMediaBlobParams`:

| Field | Value |
|-------|-------|
| `activationHeight` | **1200** |
| `active` | `true` |
| `phase7CapActivationHeight` | **1306** |
| `phase7CapActiveAtTip` | `true` |
| `effectiveMaxBytesAtTip` | **268435456** (256 MiB) |
| `maxBytes` (baseline field) | Still reports the smaller baseline; **effective** tip cap is what matters for admission |

Operators must keep **identical** activation / PHASE7 env across validators; partial rolls fork admission. Fly public testnet coordinates admission at height **1200**; PHASE7 cap at **1306**.

## RPC surface (selection)

| Method | Use |
|--------|-----|
| `boing_getMediaBlobParams` | Caps, activation, gas / checklist knobs |
| `boing_getBlobMeta` / `boing_getBlob` | Metadata / bytes |
| `boing_getBlobAccess` | Access / ACL-related metadata |
| DAS / sample / serve-challenge methods | Availability sampling and challenges |

Wallet **publish sessions** for PutBlob (`boing_requestPutBlobSession`) are an **Express-side** flow, not a `boing-node` JSON-RPC method.

## Download gating

Phase **34** introduces deployer-owned download ACL behavior (wallet proof for publisher / grantee; operator token override paths on explorer). Product details: monorepo `docs/BOING-L1-MEDIA-BLOBS.md` and `docs/HANDOFF_BLOB_DOWNLOAD_ACL.md`. Observer streams bytes via its own `/api/blob/download` path when authorized.

## Honest limits

Several PHASE checklist items remain scaffolded (for example ReservedRealV1 verifier, optional BLS committee flip). Treat opaque DAS proofs as **not** full availability evidence until verify is implemented. Re-read `capActivationChecklist` on the live params object before ops changes.
