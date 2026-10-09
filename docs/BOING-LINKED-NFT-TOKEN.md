# Linked NFT collection ↔ fungible token (display-only MVP)

> 👋 **Everyday users:** this is a specialist document. Start at [README.md](README.md).
> 🛠️ **Developers:** use `boing-sdk` helpers; keep this aligned with shipped code.
> 🛰️ **Operators:** no new node binary or QA purpose category — separate `nft` + `token` deploys.

## Verdict

Boing L1 has **no** on-chain collection↔token registry yet. The MVP is a **metadata convention + SDK**:

| Decision (2026-10-09) | Choice |
|----------------------|--------|
| Cardinality | **Many-to-many** (no strict 1:1 limit) |
| Mutability | **Mutable after create** (off-chain JSON / republished metadata) |
| Enforcement | **Display-only** (not mint-gating / not registry) |
| Spoof policy | **Soft-gate:** prefer same deployer / `attester`; document spoof risk |

## Schema `boing.linked_nft_token.v1`

Canonical JSON (fixed key order via SDK normalize):

```json
{
  "schema": "boing.linked_nft_token.v1",
  "role": "nft_collection",
  "self": "0x…32-byte AccountId…",
  "peers": ["0x…", "0x…"],
  "attester": "0x…",
  "revision": 0,
  "note": ""
}
```

- **`role`:** `nft_collection` | `fungible_token`
- **`peers`:** AccountIds of the other side (deduped, lowercased, sorted before hash)
- **`attester`:** preferred same-deployer hint for indexers / UIs
- **`revision`:** bump when attaching / updating / unlinking peers off-chain

**On-chain commit:** Blake3-256 of UTF-8 canonical JSON → **`description_hash`** on `contract_deploy_meta` (same pattern as `boing.native_token_security.v1`).

**Off-chain JSON keys** (mutable surface after deploy):

| Key | Meaning |
|-----|---------|
| `linked_nft_token` | Full schema object |
| `companion_tokens` | Convenience peer list on NFT / project metadata |
| `companion_collections` | Convenience peer list on fungible / project metadata |

Deploy-time `description_hash` is the **initial** commitment. Later peer changes live in republished off-chain metadata (templates do not rewrite deploy meta).

## CREATE2 salt convention

For joint “project” deploys:

1. Choose a 32-byte **collection** salt (or let the SDK randomize).
2. **Token** salt = `BLAKE3("boing.nft_token_pair.v1" ‖ collectionSalt)`.
3. Predict both addresses with `predictCreate2ContractAddress`, then put mutual peers into both link documents before submit.

Two txs remain (mempool / QA). Handle partial success in the UI.

## SDK surface (`boing-sdk`)

| Helper | Purpose |
|--------|---------|
| `normalizeLinkedNftToken` / `encodeLinkedNftTokenJson` / `decodeLinkedNftTokenJson` | Schema encode/decode |
| `descriptionHashHexFromLinkedNftToken` | Blake3 → `description_hash` |
| `attachLinkedNftTokenPeer` / `updateLinkedNftTokenPeers` / `unlinkLinkedNftTokenPeer` | Mutate peer lists (+ `revision`) |
| `applyLinkedNftTokenOffchainKeys` / `readLinkedNftTokenFromOffchainMetadata` | Off-chain JSON keys |
| `linkedNftTokenPairSalts` / `deriveLinkedNftTokenFungibleSaltHex` | Salt convention |
| `buildLinkedNftTokenPairDeploys` | Two `contract_deploy_meta` txs + predicted addresses |
| `softGateLinkedNftTokenPeers` | Prefer same deployer / attester; flag spoof risk |

See `boing-sdk/src/linkedNftToken.ts` and `boing-sdk/tests/linkedNftToken.test.ts`.

## Spoof risk

Anyone can put arbitrary AccountIds in `peers` or off-chain keys. **Do not** treat a link as authoritative without checks. Prefer:

1. Same deploy AccountId on both contracts, and/or
2. `attester` matching that deployer (`softGateLinkedNftTokenPeers`).

A future on-chain registry (DEX `register_pair` precedent) can harden discovery later.

## Related

- [BOING-REFERENCE-NFT.md](BOING-REFERENCE-NFT.md)
- [BOING-REFERENCE-TOKEN.md](BOING-REFERENCE-TOKEN.md)
- [BOING-CANONICAL-DEPLOY-ARTIFACTS.md](BOING-CANONICAL-DEPLOY-ARTIFACTS.md)
