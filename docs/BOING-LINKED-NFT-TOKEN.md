# Linked NFT collection ↔ fungible token (enforced on-chain)

> 👋 **Everyday users:** this is a specialist document. Start at [README.md](README.md).
> 🛠️ **Developers:** use `boing-sdk` registry helpers; keep this aligned with shipped code.
> 🛰️ **Operators:** one registry is live on public testnet (below); redeploy only with the same CREATE2 salt + bytecode if you need a fresh chain.

## Verdict (Nico 2026-10-09, updated)

| Topic | Decision |
|-------|----------|
| **Enforcement** | **On-chain registry** (not display-only) |
| **Cardinality** | **Many-to-many** |
| **Mutability** | **Add / remove** via registry calls |
| **Metadata schema** | Optional **cache only** — not source of truth |

Protocol merge: [PR #42](https://github.com/Boing-Network/boing.network/pull/42) @ `main` **`924c0ba`**.

## Live registry (public testnet)

| Field | Value |
|-------|--------|
| **RPC** | `https://testnet-rpc.boing.network/` |
| **Registry `AccountId`** | `0xebf9f0190f415852f90d0e60343126201248ab96273fdbf8acc5fe5fa03c3dd8` |
| **CREATE2 salt** | `BOING_NFT_TOKEN_LINK_REG_V1` (`LINKED_NFT_TOKEN_REGISTRY_CREATE2_SALT_V1` / SDK `LINKED_NFT_TOKEN_REGISTRY_CREATE2_SALT_V1_HEX`) |
| **Selectors** | `0xE0`–`0xE6` |
| **Auth** | Dual **asset claimer** (claim both sides, then register / unlink) |
| **Cardinality / mutability** | Many-to-many; mutable after create |
| **QA purpose** | `dapp` |
| **Mainnet** | Not deployed |

**SDK constant:** `CANONICAL_BOING_TESTNET_LINKED_NFT_TOKEN_REGISTRY_HEX` (`linkedNftTokenRegistry.ts`).

**App env (wire the same hex):** `NEXT_PUBLIC_BOING_LINKED_NFT_TOKEN_REGISTRY` / `REACT_APP_BOING_LINKED_NFT_TOKEN_REGISTRY` / `BOING_LINKED_NFT_TOKEN_REGISTRY`.

**Verify (public):**

```bash
curl -fsS -A boing-sdk/json-rpc -X POST https://testnet-rpc.boing.network/ \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"boing_simulateContractCall","params":["0xebf9f0190f415852f90d0e60343126201248ab96273fdbf8acc5fe5fa03c3dd8","0x00000000000000000000000000000000000000000000000000000000000000e3"]}'
```

Expect `success: true` and a zero count word when no links are registered. Optional node hint `end_user.canonical_linked_nft_token_registry` is a follow-up; until then, apps hardcode or env-pin the AccountId above.

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
| **`buildLinkedNftTokenProjectPack`** | **dApp entry:** joint NFT+token CREATE2 deploys **+** claim×2 + `register_link` (thin composition) |
| `buildLinkedNftTokenPairDeploys` | Joint NFT+token CREATE2 deploys only |
| `buildLinkedNftTokenRegisterFlowTxs` | claim×2 + register Express txs (after addresses known) |
| `buildLinkedNftTokenRegistryDeployMetaTx` | Deploy registry |
| `encodeLinkedNftTokenClaimAsset*` / `RegisterLink*` / `UnlinkAt*` / … | Calldata |
| `buildLinkedNftTokenRegistryContractCallTx` | Access-listed `contract_call` |
| `linkedNftToken.ts` schema helpers | **Optional cache** (`description_hash` / off-chain JSON) |

### Recommended dApp sequence (project pack)

Prefer the thin pack helper when a wizard should mint a linked collection+token in one flow:

```ts
import {
  buildLinkedNftTokenProjectPack,
  CANONICAL_BOING_TESTNET_LINKED_NFT_TOKEN_REGISTRY_HEX,
} from 'boing-sdk';

const pack = buildLinkedNftTokenProjectPack({
  deployerHex: walletAccountId, // CREATE2 deployer + default claimer
  collectionName: 'My Collection',
  collectionSymbol: 'MYC',
  tokenName: 'My Token',
  tokenSymbol: 'MYT',
  // registryHex32 defaults to CANONICAL_BOING_TESTNET_LINKED_NFT_TOKEN_REGISTRY_HEX
  registryHex32: CANONICAL_BOING_TESTNET_LINKED_NFT_TOKEN_REGISTRY_HEX,
});

// 1) Submit pack.collectionDeployTx then pack.tokenDeployTx (either order OK).
// 2) After both land, submit pack.registerFlowTxs in order:
//    claim(collection) → claim(token) → register_link.
//    Claim in the same session as deploy so nobody front-runs claim_asset.
// pack.submitOrder === ['collectionDeploy','tokenDeploy','claimCollection','claimToken','registerLink']
```

**Lower-level composition** (same result) if you already split deploy vs register UI:

1. `buildLinkedNftTokenPairDeploys({ deployerHex, collectionName, … })` → two `contract_deploy_meta` txs + predicted addresses.
2. Submit both deploys; handle partial success (one of two may land alone).
3. `buildLinkedNftTokenRegisterFlowTxs({ senderHex32, registryHex32, collectionHex32, tokenHex32 })` with the predicted (or confirmed) AccountIds.
4. Submit the three `contract_call` txs in order.

No new contracts are required — the pack only wires existing reference NFT/fungible templates and the live registry.

## Optional metadata cache

Schema `boing.linked_nft_token.v1` and keys `linked_nft_token` / `companion_tokens` / `companion_collections` remain for UI hydration. **Do not** treat them as authoritative; resolve peers from the registry.

## Related

- Rust: `crates/boing-execution/src/linked_nft_token_registry.rs`
- [NATIVE-DEX-FACTORY.md](NATIVE-DEX-FACTORY.md) (same register-directory pattern)
- [BOING-REFERENCE-NFT.md](BOING-REFERENCE-NFT.md) / [BOING-REFERENCE-TOKEN.md](BOING-REFERENCE-TOKEN.md)
