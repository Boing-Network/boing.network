# Linked NFT collection ↔ fungible token (enforced on-chain)

> 👋 **Everyday users:** this is a specialist document. Start at [README.md](README.md).
> 🛠️ **Developers:** use `boing-sdk` registry helpers; keep this aligned with shipped code.
> 🛰️ **Operators:** the v2 registry is live on public testnet (below); on a fresh chain deploy the empty v2 runtime with the v2 salt.

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
| **Registry `AccountId` (v2, current)** | `0xf704485ad29f56bb6516ac822fcf9ca8fcb6c4266e22c9ed7f9c6908668655dd` |
| **Deployed** | block **1174**, tx `0xd7dbd2e155784372fc872de67f1a993f46dd0845dfde2c24f4297b33950ee398` (deployer `0x8079f305…f696e`) |
| **CREATE2 salt** | `BOING_NFT_TOKEN_LINK_REG_V2` (`LINKED_NFT_TOKEN_REGISTRY_CREATE2_SALT_V2` / SDK `LINKED_NFT_TOKEN_REGISTRY_CREATE2_SALT_V2_HEX`) |
| **Selectors** | `0xE0`–`0xE8` |
| **Auth** | Only an asset's **creator** can claim it (below); dual claimer for register / unlink |
| **Cardinality / mutability** | Many-to-many; mutable after create |
| **QA purpose** | `dapp` |
| **Mainnet** | Not deployed |
| **v1 (retired)** | `0xebf9f0190f415852f90d0e60343126201248ab96273fdbf8acc5fe5fa03c3dd8` — first caller could claim any AccountId. Its 6 claims and 3 links (all claimants were the real deployers) were copied into v2 at height 1173. Do not send new calls there. |

**SDK constants:** `CANONICAL_BOING_TESTNET_LINKED_NFT_TOKEN_REGISTRY_HEX` (v2) and `CANONICAL_BOING_TESTNET_LINKED_NFT_TOKEN_REGISTRY_V1_HEX` (`linkedNftTokenRegistry.ts`).

**App env (wire the same hex):** `NEXT_PUBLIC_BOING_LINKED_NFT_TOKEN_REGISTRY` / `REACT_APP_BOING_LINKED_NFT_TOKEN_REGISTRY` / `BOING_LINKED_NFT_TOKEN_REGISTRY`.

**Verify (public):**

```bash
curl -fsS -A boing-sdk/json-rpc -X POST https://testnet-rpc.boing.network/ \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"boing_simulateContractCall","params":["0xf704485ad29f56bb6516ac822fcf9ca8fcb6c4266e22c9ed7f9c6908668655dd","0x00000000000000000000000000000000000000000000000000000000000000e3"]}'
```

Expect `success: true` and the link count (3 copied from v1, plus new links).

## Auth model (v2: creator-only claims)

The registry does **not** call into NFT/token templates (lazy-admin `CALL` would hijack admin), and the VM has no
opcode to read another contract's storage or its deployer. v2 therefore takes proof of creation from the asset itself:

1. **`claim_self(claimer)`** (`0xE7`) — `CALLER` **is the asset**. Only the asset's own code can make that call, so an
   asset deployed with `0xFD` init code calls it from the init frame with `claimer = CALLER of the init` (the deploy tx
   sender). The creator is the claimer in the same transaction that creates the asset; nobody can claim earlier
   (the asset has no code yet) or later (already claimed). SDK: `buildLinkedNftTokenClaimSelfInitSnippetHex(registry)`;
   Rust: `linked_nft_token_registry_v2_claim_self_init_snippet`. If the registry call fails, the whole deploy fails.
2. **`claim_asset(asset)`** (`0xE0`) — only `CALLER == deployer record of asset`. Deployer records are written once by
   the registry's own deploy (seed: the sender of the deploy tx of every contract that existed at height 1173), so
   assets created before v2 can still be claimed by whoever deployed them. Idempotent for the current claimer.
3. **`transfer_asset_claimer(asset, new)`** — only current claimer (unchanged).
4. **`register_link(collection, token)`** / **`unlink_at(index)`** — `CALLER` must be claimer of **both** sides (unchanged).

Failed writes (not authorised, already claimed, bad index, unknown selector) **fail the transaction** (v1 returned
success and did nothing). Reads keep the v1 behaviour (out-of-range `get_link_at` returns empty).

Contracts created after v2 without the init snippet cannot be claimed. A contract whose runtime forwards arbitrary
calls would let anyone call `claim_self` as it; the reference NFT/token templates do not.

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
| `claim_self` (v2) | `0xE7` | **64** | Word1 = claimer; `CALLER` is the asset |
| `get_asset_deployer` (v2) | `0xE8` | **64** | Returns the seeded deployer record (zero if none) |

Max slots: **4096** (`LINKED_NFT_TOKEN_REGISTRY_MAX_LINKS`). No O(1) presence map (no hash opcode; XOR maps collide) — clients scan `get_link_at` / logs and skip zeros / duplicates.

## Logs

| Event | topic0 | topic1 | topic2 | data |
|-------|--------|--------|--------|------|
| register | `BOING_NFT_TOKEN_LINK_REG1…` | collection | token | CALLER |
| unlink | `BOING_NFT_TOKEN_LINK_UNL1…` | collection | token | CALLER |
| claim (v2, `Log2`) | `BOING_NFT_TOKEN_LINK_CLM2…` | asset | — | claimer |

## Storage

- **Count** — `linked_nft_token_registry_count_key()`
- **Pair *i*** — `BASE + (i * 2 + f)` for `f ∈ {0,1}` (collection, token)
- **Claimer** — `asset ^ LINKED_NFT_TOKEN_CLAIMER_XOR`
- **Deployer record (v2)** — `asset ^ LINKED_NFT_TOKEN_DEPLOYER_XOR_V2` (`BOING_LNK_DEPLOYER_V2`)

## CREATE2

v2 salt: `LINKED_NFT_TOKEN_REGISTRY_CREATE2_SALT_V2` (`BOING_NFT_TOKEN_LINK_REG_V2`).
Runtime: `linked_nft_token_registry_v2_runtime_bytecode()` / SDK `DEFAULT_LINKED_NFT_TOKEN_REGISTRY_V2_RUNTIME_BYTECODE_HEX`
(a plain deploy of it is an empty v2 registry for a new network: `buildLinkedNftTokenRegistryV2DeployMetaTx`).
Seeded deploy (testnet): `linked_nft_token_registry_v2_deploy_bytecode(seed)`; reproduce with

```bash
cargo run -p boing-execution --example dump_linked_nft_token_registry_v2 -- \
  --seed scripts/linked-nft-token-registry-v2-testnet.seed.txt > /tmp/reg_v2_deploy.hex
node scripts/deploy-linked-nft-token-registry-v2.mjs --bytecode-file /tmp/reg_v2_deploy.hex   # verifies the live one
```

v1 salt `BOING_NFT_TOKEN_LINK_REG_V1`, bytecode `linked_nft_token_registry_bytecode()` (kept for history).
QA purpose: **`dapp`**.

## SDK (`boing-sdk`)

| Surface | Role |
|---------|------|
| **`buildLinkedNftTokenProjectPack`** | **dApp entry:** joint NFT+token CREATE2 deploys **+** `register_link` (`claimAtDeploy: true` with bytecode overrides that include the claim snippet; otherwise claim×2 + register) |
| `buildLinkedNftTokenClaimSelfInitSnippetHex` | v2: init-code snippet that claims the asset for the deploy tx sender |
| `buildLinkedNftTokenPairDeploys` | Joint NFT+token CREATE2 deploys only |
| `buildLinkedNftTokenRegisterFlowTxs` | claim×2 + register Express txs (after addresses known); `includeClaims: false` → register only |
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
} from '@boing-network/boing-sdk';

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
// 2) After both land, submit pack.registerFlowTxs in order.
// On v2, give both deploys 0xFD init code that runs buildLinkedNftTokenClaimSelfInitSnippetHex(registry)
// (collectionBytecodeHexOverride / tokenBytecodeHexOverride) and pass claimAtDeploy: true:
// pack.submitOrder === ['collectionDeploy','tokenDeploy','registerLink'].
// Without it the flow is claim(collection) → claim(token) → register_link, which on v2 only works
// for assets with a deployer record.
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

- Builder attestation (separate registry; builder signal, not companions): [BOING-BUILDER-ATTESTATION.md](BOING-BUILDER-ATTESTATION.md)
- Rust: `crates/boing-execution/src/linked_nft_token_registry_v2.rs` (v1: `linked_nft_token_registry.rs`)
- [NATIVE-DEX-FACTORY.md](NATIVE-DEX-FACTORY.md) (same register-directory pattern)
- [BOING-REFERENCE-NFT.md](BOING-REFERENCE-NFT.md) / [BOING-REFERENCE-TOKEN.md](BOING-REFERENCE-TOKEN.md)
