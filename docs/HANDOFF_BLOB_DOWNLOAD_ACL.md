# Handoff: deployer-owned blob download ACL (Phase 34)

**Status:** design draft — not wired  
**Depends on:** permission-gated raw download ([#103](https://github.com/Boing-Network/boing.network/pull/103)) · explorer UI ([boing.observer#36](https://github.com/Boing-Network/boing.observer/pull/36))  
**L1 context:** [BOING-L1-MEDIA-BLOBS.md](./BOING-L1-MEDIA-BLOBS.md) · sidecar `BlobMetaRecord.publisher`

---

## Today (ops shared secret)

| Secret | Where |
|--------|--------|
| `BOING_OPERATOR_RPC_TOKEN` | Fly node + observer Worker |
| `BOING_BLOB_DOWNLOAD_TOKENS` | Optional allowlist (comma-separated) |

Users do **not** discover this in-product. Operators paste the Fly/Worker secret into the explorer download box. That is a temporary gate so large `PutBlob` bodies stay off the public 6 MiB JSON path.

`PutBlob` already stores **`publisher`** (tx sender) on the sidecar — unused for download auth yet.

---

## Goal

Publisher of a media blob controls who may download raw bytes: **automatic self-access**, plus **grant/revoke** for other accounts (and optional shareable capability tickets). Network operator token remains **break-glass only**.

---

## Why not “chain mints a unique token per blob”?

Consensus cannot privately hand the deployer a secret (PutBlob / receipts are public). The deployer already held the bytes at upload time. Authorization should be **AccountId-based** (wallet proof), not a password embedded in L1.

---

## Design

### Who may download

Allow `GET /v1/media/blob?id=` when the caller proves an AccountId that is:

1. `meta.publisher` (automatic), or  
2. on the blob ACL allowlist, or  
3. holder of a publisher-signed capability ticket (optional), or  
4. operator/download shared secret (break-glass)

### Proof

Domain-separated Ed25519 (`boing_signMessage`), same family as slash ballots / name commits:

```text
BOING_BLOB_DOWNLOAD_V1 || chain_id_le64 || blob_id32 || expires_unix_le64 || nonce32
```

### Grant / revoke (proposed payload indices **34** / **35**)

| Variant | Index | Fields | Auth |
|---------|------:|--------|------|
| `GrantBlobDownload` | 34 | `blob_id`, `grantee` | tx sender == publisher |
| `RevokeBlobDownload` | 35 | `blob_id`, `grantee` | tx sender == publisher |

Sidecar ACL (not SMT): `data/blobs/access/{id}.bin` → `{ grantees, epoch }`.  
Revoke bumps `epoch` so old capability tickets die.

Read RPC: `boing_getBlobAccess` → `{ publisher, grantees, epoch }` (public; not the bytes).

### Optional capability tickets

Publisher signs `BOING_BLOB_CAP_V1 || chain_id || blob_id || epoch || expires || …` for share links. Revocation = revoke grantee and/or bump epoch.

### Activation

`BOING_BLOB_ACL_ACTIVATION_HEIGHT` (unset = keep today’s operator-gated behavior when secrets are set). After activation: signature/ACL path is primary; operator token remains override.

### Default privacy

Recommendation: **HTTP raw download private-by-default** (publisher + grantees). Keep `boing_getBlobMeta` public. Leave JSON-RPC `boing_getBlob` as today unless `BOING_BLOB_GET_REQUIRE_AUTH` / ACL mode is enabled.

---

## Build order

1. Publisher-only signed download (no new txs) — proves wallet path  
2. Wire signature auth on `GET /v1/media/blob` (keep operator break-glass)  
3. Tx 34/35 + ACL sidecar + SDK helpers  
4. Observer: replace token box with wallet Download + Manage access  
5. Capability tickets (optional)  
6. Docs + Fly activation checklist  

---

## Explicit non-goals

- Encrypting bytes at rest against the hosting node  
- Putting ACL in the account SMT / state root  
- Removing operator break-glass on public RPC  

---

## Open decision

Confirm **on-chain Grant/Revoke txs (34/35)** vs off-chain capabilities only before implementation starts.
