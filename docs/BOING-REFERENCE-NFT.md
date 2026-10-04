# Boing reference NFT layout (off-chain standard)

> 👋 **Everyday users:** this is a specialist document. Start at [README.md](README.md) for user / developer / operator paths.
> 🛠️ **Developers:** keep this aligned with shipped code.
> 🛰️ **Operators:** treat the sections below as the working spec for this topic.


This document defines a **recommended** calldata layout for NFT-style contracts on the **Boing VM**. It is **not** a consensus-enforced transaction type: deployers use ordinary `ContractDeploy` / `ContractCall` with bytecode that may implement this ABI. All deploys still pass **protocol QA** (`boing-qa`). Use purpose category **`NFT`** / **`nft`** when declaring deploys (see `QUALITY-ASSURANCE-NETWORK.md`).

> **Ops / testnet (6913):** Template **v2** + `GAS_PER_CONTRACT_CALL = 3_000_000` are on **`boing.network` `main`** (merge [`2bd5a33`](https://github.com/Boing-Network/boing.network/commit/2bd5a332d7535eaed107bde8945c6c4e1d77d511)). Public Fly apps **`boing-testnet-1` / `boing-testnet-2`** must still be **redeployed** before `mint_batch` at n≈50 works on `https://testnet-rpc.boing.network/`. Until then, do **not** claim batch mint is live on hosted testnet; older 100 000-gas nodes `OutOfGas`. This is **not** JSON-RPC HTTP batching (`BOING_RPC_MAX_BATCH`).

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
| `0x06` | `mint_batch` | Admin-only atomic multi-mint (template **v2** only). See layout below. |

### Token id

The reference treats **`token_id` as a full 32-byte opaque word**. Contracts may internally use only part of it (e.g. sequential ids in the low 8 bytes).

### `mint_batch` layout (template v2)

| Offset | Content |
|--------|---------|
| 0..31 | Selector word (low byte **`0x06`**) |
| 32..63 | `to` (32-byte `AccountId`; every token in this call) |
| 64..95 | `n` as big-endian **u64 in the low 8 bytes** (`1 ≤ n ≤ 50`; high 24 bytes of the word must be zero) |
| 96 .. 96+32n−1 | `tokenIds[i]` words |
| 96+32n .. 96+64n−1 | `metadataHashes[i]` words |

Total size: **`96 + 64n`**. Bytecode hard cap: **`MAX_REFERENCE_NFT_MINT_BATCH = 50`**.

**Semantics (check-all-then-write):**

- Caller must equal the collection **admin** (same lazy-admin slot as v1).
- `to` must be **nonzero** (v2-only; v1 `transfer_nft` does not forbid minting to the zero account).
- Every `token_id` must be **unowned** and **unique** in the list.
- Then write owner slots, then metadata slots.
- **Zero-hash policy (option B):** an all-zero `metadataHashes[i]` **skips** that metadata `SSTORE` (saves 20 000 gas). Non-zero hashes store `token_id ^ REF_NFT_METADATA_STORAGE_XOR`.
- On any check failure the program **`JUMP`s to an invalid PC** (`VmError::InvalidJump`). Receipt `success` is **false** and **no** owner/metadata stores from this call persist. Do **not** treat `STOP` as revert — `STOP` commits.
- The VM has **no `CALLDATASIZE`**: a trailing **nonzero** word at `96+64n` faults; extra **zero** padding cannot be distinguished from exact length. Encoders must emit **exactly** `96+64n`.
- **Lazy admin:** if the admin slot is empty, the **first** successful opcode path still `SSTORE`s `CALLER` as admin **before** dispatch. First-touch with `owner_of` (or any call) as the creator **before** exposing `mint_batch`. A first `mint_batch` from a stranger would steal admin (same grief as v1, worse in a batch).

**Gas (production):** `GAS_PER_CONTRACT_CALL = 3_000_000` (was 100 000). `SSTORE` is 20 000. Measured **n = 50** owner+metadata mint ≈ **2 151 307** gas on the v2 template (interpreter tests use this production budget, not 5e6). Fees are `ceil(gas_used / 21_000)` BOING, so a full 50-token metadata mint is on the order of **~103 BOING**. Chunking is **not** required for n ≤ 50 on a node running this budget; older 100k-budget nodes will `OutOfGas`.

**Access lists:** `read`/`write` **AccountIds** are **sender + collection**. NFT storage keys live **inside** the collection account. Recipients are not listed unless the call `CALL`s them.

**Failure observation:** poll `boing_getTransactionReceipt(tx_id)` where **`tx_id = Transaction::id()`** (BLAKE3 of the unsigned tx body). Mempool `{ tx_hash: "ok" }` is only an ack. Then `owner_of` / storage for each id.

## Rust / SDK helpers

- Rust: `encode_owner_of_calldata`, `encode_transfer_nft_calldata`, `encode_set_metadata_hash_calldata`, **`encode_mint_batch_calldata`**, `SELECTOR_MINT_BATCH`, `MAX_REFERENCE_NFT_MINT_BATCH` in `reference_nft`.
- TypeScript: `encodeReferenceOwnerOfCalldata`, `encodeReferenceTransferNftCalldata`, `encodeReferenceSetMetadataHashCalldata`, **`encodeReferenceMintBatchCalldata`** / **`encodeReferenceMintBatchCalldataHex`**. `BoingReferenceCallDescriptors` stays 96-byte-only; do not encode `mint_batch` through that helper.

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

**Implementation:** `boing_execution::reference_nft_collection_template_bytecode()` — lazy admin; `owner_of` / `transfer_nft` / `set_metadata_hash` / **`mint_batch`**; XOR keys `REF_NFT_OWNER_STORAGE_XOR` / `REF_NFT_METADATA_STORAGE_XOR`. **Single mint:** unowned `token_id` + admin `transfer_nft`. **Batch mint (v2):** `mint_batch`.

Integration: [BOING-CANONICAL-DEPLOY-ARTIFACTS.md](BOING-CANONICAL-DEPLOY-ARTIFACTS.md). **`boing-sdk`:** `resolveReferenceNftCollectionTemplateBytecodeHex`, **`REFERENCE_NFT_COLLECTION_TEMPLATE_VERSION`** = **`2`**, artifact id **`boing.reference_nft_collection.v0`**. Hex: `cargo run -p boing-execution --example dump_reference_token_artifacts` (**third** `0x` line) or `node boing-sdk/scripts/embed-reference-nft-collection-template-hex.mjs`.

### v1 vs v2 (no in-place upgrade)

Contract code is **immutable** after deploy ([BOING-PATTERN-UPGRADE-PROXY.md](BOING-PATTERN-UPGRADE-PROXY.md)). **Existing v1 collections cannot gain `0x06`.** Calling `mint_batch` on v1 **`STOP`s** with `success: true` and mints **nothing**.

| Situation | What to do |
|-----------|------------|
| New drop / zero tokens minted | Deploy a **new** collection from template **v2**, first-touch admin, then one `mint_batch` (or chunks of 50). |
| Some tokens already minted on v1 | Keep one-tx-per-token `transfer_nft`, **or** deploy a **new** v2 collection and remint **unowned drafts only** (new AccountIds; marketplace must migrate). Do **not** replace bytecode on the old address. |

`owner_of` / `transfer_nft` / `set_metadata_hash` on v2 match v1 for secondary sales.

---

## QA

NFT deploys should declare a valid **purpose** (`NFT`, `nft`, …). Default `RuleRegistry` checks opcode whitelist, well-formedness, and size (32 KiB). Tests expect **Allow or Unsure** — there is **no** default hash allowlist of official NFT bytecode. Governance may put `nft` in `always_review_categories` (testnet pool). This document does not add new QA rules.

For **marketplace / escrow** contracts, purpose **`dApp`** may apply. Avoid evasive **proxy** patterns that hide reviewed code ([BOING-PATTERN-UPGRADE-PROXY.md](BOING-PATTERN-UPGRADE-PROXY.md)).
