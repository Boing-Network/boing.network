# Handoff: Phase 34 deployer-owned media blob download ACL

**Status:** implemented (capability tickets deferred to Phase 34.1)  
**Related:** [BOING-L1-MEDIA-BLOBS.md](./BOING-L1-MEDIA-BLOBS.md) · store design `docs/blob-download-acl-phase34.md`

## What shipped

| Piece | Detail |
|-------|--------|
| Tx **34** `GrantBlobDownload` | `{ blob_id, grantee }` — publisher-only |
| Tx **35** `RevokeBlobDownload` | `{ blob_id, grantee }` — publisher-only; bumps ACL `epoch` |
| Sidecar | `data/blobs/access_index.bin` — `{ grantees[], epoch }` per blob |
| RPC read | `boing_getBlobAccess` → publisher, grantees, epoch, activation flags |
| HTTP download | `GET /v1/media/blob?id=` accepts operator/download secrets **or** Phase 34 proof headers |
| Activation | `BOING_BLOB_ACL_ACTIVATION_HEIGHT` — tip ≥ H ⇒ private-by-default HTTP download |
| SDK | `grantBlobDownloadPayload` / `revokeBlobDownloadPayload` / `blobDownloadProofPreimage` / `signBlobDownloadProof` |

## Download proof headers

Domain: `BOING_BLOB_DOWNLOAD_V1` ‖ `chain_id` LE ‖ `blob_id` ‖ `expires_at_unix` LE ‖ `nonce` (32) → BLAKE3 → Ed25519 sign.

| Header | Value |
|--------|--------|
| `X-Boing-Blob-Downloader` | `0x` AccountId (32) |
| `X-Boing-Blob-Expires` | unix seconds |
| `X-Boing-Blob-Nonce` | `0x` 32 random bytes |
| `X-Boing-Blob-Signature` | `0x` 64-byte Ed25519 over the preimage |

Wallet path: compute preimage → `boing_signMessage(0x<preimage hex>)` → attach headers.

Allow if signer is `meta.publisher` or ACL grantee. Operator / `BOING_BLOB_DOWNLOAD_TOKENS` remain break-glass.

## Operator notes

1. Set `BOING_BLOB_ACL_ACTIVATION_HEIGHT` on Fly / public RPC when ready for private-by-default.
2. Keep `BOING_OPERATOR_RPC_TOKEN` for observer proxy and ops break-glass.
3. ACL is node-local sidecar (same class as blob bytes) — not in the account SMT / state root.
4. Capability tickets (`BOING_BLOB_CAP_V1`) are coded in primitives but not yet wired on HTTP (Phase 34.1).

## Acceptance

- [ ] PutBlob → publisher downloads with signed proof (no shared secret)
- [ ] Grant → grantee downloads; Revoke → grantee denied
- [ ] Operator token still downloads when secrets configured
- [ ] With ACL activation height reached and no proof/secret → HTTP 401
- [ ] `boing_getBlobAccess` lists publisher + grantees
