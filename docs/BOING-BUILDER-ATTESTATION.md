# Builder attestation registry (on-chain)

> 👋 **Everyday users:** product apps say **Builder attested** — not “registry.”
> 🛠️ **Developers:** use `boing-sdk` helpers; keep this aligned with shipped code.
> 🛰️ **Operators:** deploy once with CREATE2 salt below; pin the AccountId in app env.

## Verdict (Nico 2026-10-11)

| Topic | Decision |
|-------|----------|
| **Enforcement** | **On-chain registry** (not soft wallet proofs) |
| **Revoke** | Recorded **builder** **or** current **dual claimer** |
| **Companions** | **App-only** preference to link first (no cross-contract) |
| **Duplicates** | **Allowed**; clients show latest active per builder |
| **Note** | **Hash on-chain**; finance chrome may omit note for now |
| **Project pack** | Optional **Attest as builder** CTA (not auto-prompt) |
| **Claimer** | **Separate** from companions registry claimer state |
| **Soft finance #23** | **Do not merge** (superseded) |

## Live registry (public testnet)

| Field | Value |
|-------|--------|
| **RPC** | `https://testnet-rpc.boing.network/` |
| **Registry `AccountId`** | *TBD — deploy with steps below* |
| **CREATE2 salt** | `BOING_BUILDER_ATTEST_REG_V1` (`BUILDER_ATTESTATION_REGISTRY_CREATE2_SALT_V1` / SDK `BUILDER_ATTESTATION_REGISTRY_CREATE2_SALT_V1_HEX`) |
| **Selectors** | `0xE0`–`0xE6` |
| **Auth** | Dual **asset claimer**; revoke also allows recorded builder |
| **Cardinality / mutability** | Many attestations; append + tombstone revoke |
| **QA purpose** | `dapp` |
| **Mainnet** | Not deployed |

**SDK:** `DEFAULT_BUILDER_ATTESTATION_REGISTRY_BYTECODE_HEX`, `buildBuilderAttestationRegistryDeployMetaTx`, `buildBuilderAttestationAttestFlowTxs`.

**App env (after deploy):** `REACT_APP_BOING_BUILDER_ATTESTATION_REGISTRY` / `NEXT_PUBLIC_BOING_BUILDER_ATTESTATION_REGISTRY` / `BOING_BUILDER_ATTESTATION_REGISTRY`.

## Auth model

1. **`claim_asset(asset)`** — first successful caller for that AccountId becomes its **claimer** on **this** registry (independent of companions).
2. **`transfer_asset_claimer(asset, new)`** — only current claimer.
3. **`attest(collection, token, note_hash)`** — require `CALLER` claimer of **both** sides; stores builder = `CALLER`.
4. **`revoke_at(index)`** — require `CALLER` == recorded builder **or** current dual claimer of both sides; tombstones the slot.

Operational note: claim assets in the same session as NFT/token deploy so a third party cannot front-run `claim_asset`.

## Selectors

| Selector | Byte | Calldata | Purpose |
|----------|------|----------|---------|
| `claim_asset` | `0xE0` | **64** | Word1 = asset AccountId |
| `attest` | `0xE1` | **128** | Word1 = collection, word2 = token, word3 = note_hash (zeros = empty) |
| `revoke_at` | `0xE2` | **64** | Word1 = index (u64 low 8 bytes); tombstones the slot |
| `attestations_count` | `0xE3` | **32** | Returns count word (includes tombstones) |
| `get_attestation_at` | `0xE4` | **64** | Returns **128** bytes: collection, token, builder, note_hash (zeros collection = tombstone) |
| `get_asset_claimer` | `0xE5` | **64** | Returns claimer AccountId |
| `transfer_asset_claimer` | `0xE6` | **96** | Word1 = asset, word2 = new claimer |

Max slots: **4096** (`BUILDER_ATTESTATION_REGISTRY_MAX_SLOTS`). No O(1) presence map — clients scan `get_attestation_at` / logs.

## Logs

| Event | topic0 | topic1 | topic2 | data |
|-------|--------|--------|--------|------|
| attest | `BOING_BLDR_ATTEST_REG1…` | collection | token | builder (`CALLER`) |
| revoke | `BOING_BLDR_ATTEST_UNL1…` | collection | token | recorded builder |

## Storage

- **Count** — `builder_attestation_registry_count_key()`
- **Slot *i*** — `BASE + (i * 4 + f)` for `f ∈ {0..3}` (collection, token, builder, note_hash)
- **Claimer** — `asset ^ BUILDER_ATTEST_CLAIMER_XOR`

## CREATE2

Salt: `BUILDER_ATTESTATION_REGISTRY_CREATE2_SALT_V1` (`BOING_BUILDER_ATTEST_REG_V1`).  
Bytecode: `builder_attestation_registry_bytecode()` / SDK `DEFAULT_BUILDER_ATTESTATION_REGISTRY_BYTECODE_HEX`.  
QA purpose: **`dapp`**.

## Deploy (public testnet)

Requires a funded Ed25519 seed (`BOING_SECRET_HEX`). Ephemeral faucet-funded deployer is fine (seed need not be retained after pin).

```bash
cd boing-sdk && npm run build && cd ..
cd examples/native-boing-tutorial
export BOING_RPC_URL=https://testnet-rpc.boing.network
export BOING_SECRET_HEX=0x…   # 32-byte Ed25519 seed
# optional: fund first
# npm run fund-deployer-from-env
export BOING_CREATE2_SALT_HEX=0x424f494e475f4255494c4445525f4154544553545f5245475f56310000000000
export BOING_NATIVE_BYTECODE_HEX="$(node -e "import('boing-sdk').then(m=>console.log(m.DEFAULT_BUILDER_ATTESTATION_REGISTRY_BYTECODE_HEX))")"
export BOING_PURPOSE=dapp
npm run deploy-native-purpose-contract
```

Predict AccountId before submit:

```bash
node -e "
import {
  DEFAULT_BUILDER_ATTESTATION_REGISTRY_BYTECODE_HEX,
  predictBuilderAttestationRegistryCreate2Address,
  accountIdFromSecretHex, // if available; else pass deployer AccountId
} from 'boing-sdk';
"
```

Verify after deploy (`attestations_count` → zero word):

```bash
# Replace REG with live AccountId
curl -fsS -A boing-sdk/json-rpc -X POST https://testnet-rpc.boing.network/ \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"boing_simulateContractCall","params":["REG","0x00000000000000000000000000000000000000000000000000000000000000e3"]}'
```

## SDK (`boing-sdk`)

| Surface | Role |
|---------|------|
| `buildBuilderAttestationRegistryDeployMetaTx` | Deploy registry |
| `buildBuilderAttestationAttestFlowTxs` | claim×2 + `attest` |
| `encodeBuilderAttest*` / `decodeBuilderAttest*` | Calldata + return decode |
| `buildBuilderAttestationRegistryContractCallTx` | Access-listed `contract_call` |
| `predictBuilderAttestationRegistryCreate2Address` | CREATE2 preview |

### Recommended dApp sequence

1. Prefer companions already linked (app check against companions registry).
2. Claim collection + token on **this** registry (or run `buildBuilderAttestationAttestFlowTxs`).
3. `attest` (note_hash optional zeros).
4. UIs scan `get_attestation_at` / logs; show latest active per builder.

## Related

- Companions authority: [BOING-LINKED-NFT-TOKEN.md](BOING-LINKED-NFT-TOKEN.md)
- Rust: `crates/boing-execution/src/builder_attestation_registry.rs`
- SDK: `boing-sdk/src/builderAttestationRegistry.ts`
- Dump: `cargo run -p boing-execution --example dump_builder_attestation_registry`
