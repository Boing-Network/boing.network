# Boing reference NFT layout (off-chain standard)

> 👋 **Everyday users:** this is a specialist document. Start at [README.md](README.md) for user / developer / operator paths.
> 🛠️ **Developers:** keep this aligned with shipped code.
> 🛰️ **Operators:** treat the sections below as the working spec for this topic.


This document defines a **recommended** calldata layout for NFT-style contracts on the **Boing VM**. It is **not** a consensus-enforced transaction type: deployers use ordinary `ContractDeploy` / `ContractCall` with bytecode that may implement this ABI. All deploys still pass **protocol QA** (`boing-qa`). Use purpose category **`NFT`** / **`nft`** when declaring deploys (see `QUALITY-ASSURANCE-NETWORK.md`).

## Principles

- **Boing VM only.** Opcodes and semantics are Boing-defined (`TECHNICAL-SPECIFICATION.md` §7, [BOING-VM-INDEPENDENCE.md](BOING-VM-INDEPENDENCE.md)).
- **Storage layout** is contract-defined. A common pattern is **owner mapping**: `SLOAD` / `SSTORE` with key derived from `token_id` (32-byte word) and value = owner `AccountId` or zero if burned.
- **Authorization** uses **`CALLER`** (`0x33`) to verify transfers (owner or approved operator).

## Calldata (reference)

Each call uses **96 bytes** (three 32-byte words), matching the fungible reference width for wallet consistency:

| Offset | Length | Content |
|--------|--------|---------|
| 0 | 32 | Selector word: 31 zero bytes + one-byte selector in the **last** byte. |
| 32 | 32 | First argument (see below). |
| 64 | 32 | Second argument (or zero padding for single-arg reads). |

### Selectors (reference)

| Selector (low byte) | Name | Intended meaning |
|---------------------|------|------------------|
| `0x03` | `owner_of` | Return current holder of `token_id` (layout: word1 = `token_id`, word2 = 0). |
| `0x04` | `transfer_nft` | Transfer `token_id` to `to` if authorized. Word1 = `to`, word2 = `token_id`. |
| `0x05` | `set_metadata_hash` | Optional: bind `metadata_hash` to `token_id`. Word1 = `token_id`, word2 = `metadata_hash`. |

### Token id

The reference treats **`token_id` as a full 32-byte opaque word**. Contracts may internally use only part of it (e.g. sequential ids in the low 8 bytes).

## Rust / SDK helpers

- Rust: `boing_execution::encode_owner_of_calldata`, `encode_transfer_nft_calldata`, `encode_set_metadata_hash_calldata`, and `SELECTOR_*` constants in `reference_nft`.
- TypeScript: `boing-sdk` — `encodeReferenceOwnerOfCalldata`, `encodeReferenceTransferNftCalldata`, `encodeReferenceSetMetadataHashCalldata`.

## Marketplace, royalties, and metadata (F2)

**Roadmap:** [BOING-VM-CAPABILITY-PARITY-ROADMAP.md](BOING-VM-CAPABILITY-PARITY-ROADMAP.md) track **F2**.

These are **optional conventions** on top of `owner_of`, `transfer_nft`, and `set_metadata_hash`. They do **not** change consensus; new selectors can be implemented in contract bytecode and documented per collection.

### Optional metadata keys (off-chain / URI)

When `metadata_hash` points to JSON (IPFS, HTTPS), recommended keys for marketplaces:

| Key | Use |
|-----|-----|
| `name` | Display name |
| `description` | Human-readable description |
| `image` | Thumbnail / media URI |
| `attributes` | Array of `{ trait_type, value }` for rarity UIs |
| `seller_fee_basis_points` | Optional royalty hint (0–10000); **enforce in contract** if royalties are binding |
| `fee_recipient` | Optional `AccountId` hex for royalty receiver (off-chain hint) |

**Binding royalties** require **contract logic** (e.g. on `transfer_nft`, query a stored **royalty bps + recipient** per `token_id` or collection-wide slot)—not JSON alone.

### Example call sequences

1. **List (off-chain index):** Indexer reads `owner_of` + metadata URI from `set_metadata_hash` / collection policy; listing state may live **only** in indexer DB (common pattern for off-chain order books).
2. **Sale (on-chain escrow pattern):** Buyer `ContractCall`s **escrow contract** with `token_id` + seller + price; escrow `ContractCall`s collection contract `transfer_nft` after payment leg—each tx declares **full access list** (buyer, seller, escrow, collection, token ledger).
3. **Offer / bid:** Same idea with **escrow** holding BOING or a **reference-token** balance ([BOING-REFERENCE-TOKEN.md](BOING-REFERENCE-TOKEN.md)).

---

## Canonical collection deploy template (pinned bytecode)

**Implementation:** `boing_execution::reference_nft_collection_template_bytecode()` — minimal collection with **lazy admin** (first caller becomes admin when the admin slot is unset), reference **`owner_of`**, **`transfer_nft`**, **`set_metadata_hash`**, XOR-derived storage keys `REF_NFT_OWNER_STORAGE_XOR` / `REF_NFT_METADATA_STORAGE_XOR` in `crates/boing-execution/src/reference_nft.rs`. **Mint:** when a `token_id` has no owner, only the **admin** may set the owner via `transfer_nft` (lazy mint to `to`).

Integration: [BOING-CANONICAL-DEPLOY-ARTIFACTS.md](BOING-CANONICAL-DEPLOY-ARTIFACTS.md). **`boing-sdk`:** `resolveReferenceNftCollectionTemplateBytecodeHex`, **`REFERENCE_NFT_COLLECTION_TEMPLATE_VERSION`** = **`1`**. Hex: `cargo run -p boing-execution --example dump_reference_token_artifacts` (second line).

---

## QA

NFT deploys should declare a valid **purpose** (`NFT`, `nft`, …). Bytecode must satisfy the Boing VM **opcode whitelist** and well-formedness rules. This document does not add new automated bytecode checks beyond existing QA; it standardizes **calldata** for interoperability.

For **marketplace / escrow** contracts, purpose **`dApp`** may apply. Avoid evasive **proxy** patterns that hide reviewed code ([BOING-PATTERN-UPGRADE-PROXY.md](BOING-PATTERN-UPGRADE-PROXY.md)).
