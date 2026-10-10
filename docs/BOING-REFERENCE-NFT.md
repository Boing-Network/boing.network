# Boing reference NFT layout (off-chain standard)

> 👋 **Everyday users:** this is a specialist document. Start at [README.md](README.md) for user / developer / operator paths.
> 🛠️ **Developers:** keep this aligned with shipped code.
> 🛰️ **Operators:** treat the sections below as the working spec for this topic.


This document defines a **recommended** calldata layout for NFT-style contracts on the **Boing VM**. It is **not** a consensus-enforced transaction type: deployers use ordinary `ContractDeploy` / `ContractCall` with bytecode that may implement this ABI. All deploys still pass **protocol QA** (`boing-qa`). Use purpose category **`NFT`** / **`nft`** when declaring deploys (see `QUALITY-ASSURANCE-NETWORK.md`).

> **Ops / testnet (6913):** Template **v3** raises `mint_batch` to **n ≤ 500** and needs **`GAS_PER_CONTRACT_CALL = 40_000_000`** on the node. After merge to **`boing.network` `main`**, public Fly apps **`boing-testnet-1` / `boing-testnet-2`** must be **redeployed** before n≈500 works on `https://testnet-rpc.boing.network/`. Older 3M-budget nodes `OutOfGas` on full v3 batches; v2 collections stay capped at **n = 50**. This is **not** JSON-RPC HTTP batching (`BOING_RPC_MAX_BATCH`).

## Principles

- **Boing VM only.** Opcodes and semantics are Boing-defined (`TECHNICAL-SPECIFICATION.md` §7, [BOING-VM-INDEPENDENCE.md](BOING-VM-INDEPENDENCE.md)).
- **Storage layout** is contract-defined. A common pattern is **owner mapping**: `SLOAD` / `SSTORE` with key derived from `token_id` (32-byte word) and value = owner `AccountId` or zero if burned.
- **Authorization** uses **`CALLER`** (`0x33`) to verify transfers (owner or approved operator).

## Calldata (reference)

Single-token calls (`owner_of`, `transfer_nft`, `set_metadata_hash`) use **96 bytes** (three 32-byte words):

| Offset | Length | Content |
|--------|--------|---------|
| 0 | 32 | Selector word: 31 zero bytes + one-byte selector in the **last** byte. |
| 32 | 32 | First argument (see below). |
| 64 | 32 | Second argument (or zero padding for single-arg reads). |

`mint_batch` is **variable length**: **`96 + 64n` bytes** (see below). Extra bytes after a 96-byte v1 call are ignored.

### Selectors (reference)

| Selector (low byte) | Name | Intended meaning |
|---------------------|------|------------------|
| `0x03` | `owner_of` | Return current holder of `token_id` (layout: word1 = `token_id`, word2 = 0). |
| `0x04` | `transfer_nft` | Transfer `token_id` to `to` if authorized. Word1 = `to`, word2 = `token_id`. |
| `0x05` | `set_metadata_hash` | Optional: bind `metadata_hash` to `token_id`. Word1 = `token_id`, word2 = `metadata_hash`. |
| `0x06` | `mint_batch` | Admin-only atomic multi-mint (template **v2+**). See layout below. |

### Token id

The reference treats **`token_id` as a full 32-byte opaque word**. Contracts may internally use only part of it (e.g. sequential ids in the low 8 bytes).

### `mint_batch` layout (template v2 / v3)

| Offset | Content |
|--------|---------|
| 0..31 | Selector word (low byte **`0x06`**) |
| 32..63 | `to` (32-byte `AccountId`; every token in this call) |
| 64..95 | `n` as big-endian **u64 in the low 8 bytes** (`1 ≤ n ≤ MAX`; high 24 bytes of the word must be zero) |
| 96 .. 96+32n−1 | `tokenIds[i]` words |
| 96+32n .. 96+64n−1 | `metadataHashes[i]` words |

Total size: **`96 + 64n`**. Bytecode hard cap:

| Template | `MAX_REFERENCE_NFT_MINT_BATCH` |
|----------|--------------------------------|
| **v2** (already deployed collections) | **50** |
| **v3** (new deploys) | **500** |

**Semantics (check-all-then-write):**

- Caller must equal the collection **admin** (same lazy-admin slot as v1).
- `to` must be **nonzero** (v2+; v1 `transfer_nft` does not forbid minting to the zero account).
- Every `token_id` must be **unowned** and **unique** in the list (uniqueness is an **O(n²)** scan in the reference bytecode).
- Then write owner slots, then metadata slots.
- **Zero-hash policy (option B):** an all-zero `metadataHashes[i]` **skips** that metadata `SSTORE` (saves 20 000 gas). Non-zero hashes store `token_id ^ REF_NFT_METADATA_STORAGE_XOR`.
- On any check failure the program **`JUMP`s to an invalid PC** (`VmError::InvalidJump`). Receipt `success` is **false** and **no** owner/metadata stores from this call persist. Do **not** treat `STOP` as revert — `STOP` commits.
- The VM has **no `CALLDATASIZE`**: a trailing **nonzero** word at `96+64n` faults; extra **zero** padding cannot be distinguished from exact length. Encoders must emit **exactly** `96+64n`.
- **Lazy admin:** if the admin slot is empty, the **first** successful opcode path still `SSTORE`s `CALLER` as admin **before** dispatch. First-touch with `owner_of` (or any call) as the creator **before** exposing `mint_batch`. A first `mint_batch` from a stranger would steal admin (same grief as v1, worse in a batch).

### Gas scaling (production meters)

`SSTORE` = 20 000, `SLOAD` = 100. Production call budget for **v3** is **`GAS_PER_CONTRACT_CALL = 40_000_000`**. Fees are `ceil(gas_used / 21_000)` BOING. Measured owner+metadata `mint_batch` on the v3 template (release interpreter, `--release`):

| n | gas (owner+meta) | fee (BOING) | calldata (`96+64n`) | ~interpreter ms |
|---|------------------|-------------|---------------------|-----------------|
| 1 | 41 073 | 2 | 160 | &lt;1 |
| 50 | 2 151 307 | 103 | 3 296 | ~0.5 |
| 100 | 4 564 482 | 218 | 6 496 | ~2 |
| 200 | 10 178 332 | 485 | 12 896 | ~7 |
| **500** | **33 319 882** | **1 587** | **32 096** | **~40** |
| 1000* | 92 889 132 | 4 424 | 64 096 | ~158 |
| 10 000* | ≈5.65×10⁹ | ≈269 000 | 640 096 | seconds+ |

\*n = 1000 / 10 000 exceed the **v3 bytecode cap (500)**; rows are **extrapolated / measured with a temporary higher cap** to size budgets. They are **not** supported on shipped v3.

**Budget table (owner+metadata, same meters):**

| Call gas budget | Approx max N (owner+meta) |
|-----------------|---------------------------|
| 3 000 000 | ~65 (v2 shipped 50) |
| 10 000 000 | ~195 |
| 40 000 000 | **500** (v3 shipped) |
| 50 000 000 | ~620 |
| 100 000 000 | ~1040 |

**Why not 10 000 in one tx:** gas is dominated by **2×`SSTORE` per token** plus **O(n²) in-batch uniqueness**. A 10k owner+metadata mint would need on the order of **billions** of gas and multi-second interpreter time — unrealistic for a ~2s target block. OpenSea-scale “upload 10k” is often **media/API bulk**, not one consensus transaction. On Boing, mint **10 000** as **20 × 500** (or smaller chunks) with separate `ContractCall`s and receipt polls.

**Access lists:** `read`/`write` **AccountIds** are **sender + collection**. NFT storage keys live **inside** the collection account. Recipients are not listed unless the call `CALL`s them. Calldata hex for n=500 is ~64 KiB; default RPC body limit (8 MiB) is fine. `boing_simulateContractCall` caps calldata at **256 KiB** (≈ n ≤ 4094).

**Failure observation:** poll `boing_getTransactionReceipt(tx_id)` where **`tx_id = Transaction::id()`** (BLAKE3 of the unsigned tx body). Mempool `{ tx_hash: "ok" }` is only an ack. Then `owner_of` / storage for each id.

## Rust / SDK helpers

- Rust: `encode_owner_of_calldata`, `encode_transfer_nft_calldata`, `encode_set_metadata_hash_calldata`, **`encode_mint_batch_calldata`**, `SELECTOR_MINT_BATCH`, `MAX_REFERENCE_NFT_MINT_BATCH` in `reference_nft`.
- TypeScript: `encodeReferenceOwnerOfCalldata`, `encodeReferenceTransferNftCalldata`, `encodeReferenceSetMetadataHashCalldata`, **`encodeReferenceMintBatchCalldata`** / **`encodeReferenceMintBatchCalldataHex`**. `BoingReferenceCallDescriptors` stays 96-byte-only; do not encode `mint_batch` through that helper.
- Re-measure: `cargo run -p boing-execution --example measure_mint_batch_gas --release`.

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
| `linked_nft_token` | Optional cache of **`boing.linked_nft_token.v1`** (not authoritative; prefer on-chain registry) |
| `companion_tokens` | Optional AccountId hex list cache of linked fungible tokens |

**Binding royalties** require **contract logic** (e.g. on `transfer_nft`, query a stored **royalty bps + recipient** per `token_id` or collection-wide slot)—not JSON alone.

**Linked project tokens:** enforced via the on-chain link registry (dual asset-claimer). See [BOING-LINKED-NFT-TOKEN.md](BOING-LINKED-NFT-TOKEN.md) and `boing-sdk` **`linkedNftTokenRegistry.ts`**.

### Example call sequences

1. **List (off-chain index):** Indexer reads `owner_of` + metadata URI from `set_metadata_hash` / collection policy; listing state may live **only** in indexer DB (common pattern for off-chain order books).
2. **Sale (on-chain escrow pattern):** Buyer `ContractCall`s **escrow contract** with `token_id` + seller + price; escrow `ContractCall`s collection contract `transfer_nft` after payment leg—each tx declares **full access list** (buyer, seller, escrow, collection, token ledger).
3. **Offer / bid:** Same idea with **escrow** holding BOING or a **reference-token** balance ([BOING-REFERENCE-TOKEN.md](BOING-REFERENCE-TOKEN.md)).

---

## Canonical collection deploy template (pinned bytecode)

**Implementation:** `boing_execution::reference_nft_collection_template_bytecode()` — lazy admin; `owner_of` / `transfer_nft` / `set_metadata_hash` / **`mint_batch`**; XOR keys `REF_NFT_OWNER_STORAGE_XOR` / `REF_NFT_METADATA_STORAGE_XOR`. **Single mint:** unowned `token_id` + admin `transfer_nft`. **Batch mint (v2/v3):** `mint_batch`.

Integration: [BOING-CANONICAL-DEPLOY-ARTIFACTS.md](BOING-CANONICAL-DEPLOY-ARTIFACTS.md). **`boing-sdk`:** `resolveReferenceNftCollectionTemplateBytecodeHex`, **`REFERENCE_NFT_COLLECTION_TEMPLATE_VERSION`** = **`3`**, artifact id **`boing.reference_nft_collection.v0`**. Hex: pinned [`artifacts/reference-nft-collection-template-v3.hex`](artifacts/reference-nft-collection-template-v3.hex), or regenerate with `cargo run -p boing-execution --example dump_reference_token_artifacts` (**third** `0x` line) / `node boing-sdk/scripts/embed-reference-nft-collection-template-hex.mjs`.

### v1 vs v2 vs v3 (no in-place upgrade)

Contract code is **immutable** after deploy ([BOING-PATTERN-UPGRADE-PROXY.md](BOING-PATTERN-UPGRADE-PROXY.md)). **Existing collections keep their baked-in cap.** Calling `mint_batch` on v1 **`STOP`s** with `success: true` and mints **nothing**. v2 stays at **n ≤ 50**. Only **new** deploys from template **v3** get **n ≤ 500**.

| Situation | What to do |
|-----------|------------|
| New drop / zero tokens minted | Deploy a **new** collection from template **v3**, first-touch admin, then `mint_batch` (chunks of **≤500**; for 10k use **20×500**). |
| Existing v2 collection | Keep using **n ≤ 50**, or deploy a **new** v3 collection for the higher cap. |
| Some tokens already minted on v1 | Keep one-tx-per-token `transfer_nft`, **or** deploy a **new** v3 collection and remint **unowned drafts only** (new AccountIds; marketplace must migrate). Do **not** replace bytecode on the old address. |

`owner_of` / `transfer_nft` / `set_metadata_hash` on v2/v3 match v1 for secondary sales.

---

## QA

NFT deploys should declare a valid **purpose** (`NFT`, `nft`, …). Default `RuleRegistry` checks opcode whitelist, well-formedness, and size (32 KiB). Tests expect **Allow or Unsure** — there is **no** default hash allowlist of official NFT bytecode. Governance may put `nft` in `always_review_categories` (testnet pool). This document does not add new QA rules.

For **marketplace / escrow** contracts, purpose **`dApp`** may apply. Avoid evasive **proxy** patterns that hide reviewed code ([BOING-PATTERN-UPGRADE-PROXY.md](BOING-PATTERN-UPGRADE-PROXY.md)).
