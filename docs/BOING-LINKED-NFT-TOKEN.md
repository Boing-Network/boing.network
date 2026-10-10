# Linked NFT collection ↔ fungible token (enforced on-chain)

> 👋 **Everyday users:** this is a specialist document. Start at [README.md](README.md).
> 🛠️ **Developers:** use `boing-sdk` registry helpers; keep this aligned with shipped code.
> 🛰️ **Operators:** deploy one registry contract (purpose `dapp`); separate `nft` + `token` deploys unchanged.

## Verdict (Nico 2026-10-09, updated)

| Topic | Decision |
|-------|----------|
| **Enforcement** | **On-chain registry** (not display-only) |
| **Cardinality** | **Many-to-many** |
| **Mutability** | **Add / remove** via registry calls |
| **Metadata schema** | Optional **cache only** — not source of truth |

## Auth model (dual asset claimer)

The registry does **not** call into NFT/token templates (lazy-admin `CALL` would hijack admin). Instead:

1. **`claim_asset(asset)`** — first successful caller for that AccountId becomes its **claimer**. Idempotent if already claimer; others abort.
2. **`transfer_asset_claimer(asset, new)`** — only current claimer.
3. **`register_link(collection, token)`** / **`unlink_at(index)`** — require `CALLER` to be claimer of **both** sides.

Operational note: claim assets in the same session as deploy (CREATE2-predictable addresses) so a third party cannot front-run `claim_asset` on your ids.

## Selectors

| Selector | Byte | Calldata | Purpose |
|----------|------|----------|---------|
| `claim_asset` | `0xE0` | **64** | Word1 = asset AccountId |
| `register_link` | `0xE1` | **96** | Word1 = collection, word2 = token |
| `unlink_at` | `0xE2` | **64** | Word1 = index (u64 low 8 bytes); tombstones the slot |
| `links_count` | `0xE3` | **32** | Returns count word (includes tombstones) |
| `get_link_at` | `0xE4` | **64** | Returns **64** bytes: collection, token (zeros = tombstone) |
| `get_asset_claimer` | `0xE5` | **64** | Returns claimer AccountId |
| `transfer_asset_claimer` | `0xE6` | **96** | Word1 = asset, word2 = new claimer |

Max slots: **4096** (`LINKED_NFT_TOKEN_REGISTRY_MAX_LINKS`). No O(1) presence map (no hash opcode; XOR maps collide) — clients scan `get_link_at` / logs and skip zeros / duplicates.

## Logs

| Event | topic0 | topic1 | topic2 | data |
|-------|--------|--------|--------|------|
| register | `BOING_NFT_TOKEN_LINK_REG1…` | collection | token | CALLER |
| unlink | `BOING_NFT_TOKEN_LINK_UNL1…` | collection | token | CALLER |

## Storage

- **Count** — `linked_nft_token_registry_count_key()`
- **Pair *i*** — `BASE + (i * 2 + f)` for `f ∈ {0,1}` (collection, token)
- **Claimer** — `asset ^ LINKED_NFT_TOKEN_CLAIMER_XOR`

## CREATE2

Salt: `LINKED_NFT_TOKEN_REGISTRY_CREATE2_SALT_V1` (`BOING_NFT_TOKEN_LINK_REG_V1`).  
Bytecode: `linked_nft_token_registry_bytecode()` / SDK `DEFAULT_LINKED_NFT_TOKEN_REGISTRY_BYTECODE_HEX`.  
QA purpose: **`dapp`**.

## SDK (`boing-sdk`)

| Surface | Role |
|---------|------|
| `buildLinkedNftTokenRegistryDeployMetaTx` | Deploy registry |
| `encodeLinkedNftTokenClaimAsset*` / `RegisterLink*` / `UnlinkAt*` / … | Calldata |
| `buildLinkedNftTokenRegisterFlowTxs` | claim×2 + register Express txs |
| `buildLinkedNftTokenRegistryContractCallTx` | Access-listed `contract_call` |
| `buildLinkedNftTokenPairDeploys` | Joint NFT+token CREATE2 deploys (then register on registry) |
| `linkedNftToken.ts` schema helpers | **Optional cache** (`description_hash` / off-chain JSON) |

## Optional metadata cache

Schema `boing.linked_nft_token.v1` and keys `linked_nft_token` / `companion_tokens` / `companion_collections` remain for UI hydration. **Do not** treat them as authoritative; resolve peers from the registry.

## Related

- Rust: `crates/boing-execution/src/linked_nft_token_registry.rs`
- [NATIVE-DEX-FACTORY.md](NATIVE-DEX-FACTORY.md) (same register-directory pattern)
- [BOING-REFERENCE-NFT.md](BOING-REFERENCE-NFT.md) / [BOING-REFERENCE-TOKEN.md](BOING-REFERENCE-TOKEN.md)
