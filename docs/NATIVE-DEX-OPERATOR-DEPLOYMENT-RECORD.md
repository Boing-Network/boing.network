# Native DEX — operator deployment record (addresses & env)

Use this doc to **track what you actually deployed** on a given network. It complements:

- **Canonical CREATE2 predictions** (fixed deployer + bytecode): [OPS-CANONICAL-TESTNET-NATIVE-DEX-AUX.md](OPS-CANONICAL-TESTNET-NATIVE-DEX-AUX.md), JSON mirror [`scripts/canonical-testnet-dex-predicted.json`](../scripts/canonical-testnet-dex-predicted.json).
- **Published canonical CP pool** (separate ops freeze): [OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md](OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md).

**Why two kinds of addresses?**

- **CREATE2** deploys land at **predictable** ids (same deployer + salt + bytecode ⇒ same address everywhere). If the slot was **already taken**, tutorial scripts may fall back to **nonce-derived** ids — those differ per chain history and are **not** in the canonical JSON.
- Your **pair directory** and **pool** may be CREATE2 or nonce-derived depending on collisions and flags (`BOING_USE_CREATE2`, bootstrap auto-retry).

## 1. Copy the template locally (optional)

In the tutorial package:

```bash
cp examples/native-boing-tutorial/DEPLOYMENT-ADDRESSES.example.md DEPLOYMENT-ADDRESSES.local.md
```

`DEPLOYMENT-ADDRESSES.local.md` is **gitignored** — safe for secrets-adjacent notes (never paste **seeds** into git).

## 2. Record table (fill in)

| Role | `AccountId` (0x + 64 hex) | Deploy method | Notes |
|------|---------------------------|---------------|--------|
| **Deployer** (signer pubkey) | | Ed25519 from `BOING_SECRET_HEX` | Same hex as `senderHex` in script JSON |
| **RPC** | | URL | e.g. `https://testnet-rpc.boing.network` |
| **Native CP pool** | | CREATE2 / nonce | `BOING_POOL_HEX` |
| **Pair directory (factory)** | | CREATE2 / nonce | `BOING_DEX_FACTORY_HEX` for routes / `register_pair` |
| **Ledger router v1** | | CREATE2 / nonce | 128-byte inner calldata forward |
| **Ledger router v2** | | CREATE2 / nonce | 160-byte inner (e.g. v5 `swap_to`) |
| **Ledger router v3** | | CREATE2 / nonce | 192-byte inner (e.g. v5 `remove_liquidity_to`) |
| **Multihop / swap2 router** | | CREATE2 / nonce | 2–4 hops in one tx |
| **AMM LP vault** | | optional | [NATIVE-AMM-LP-VAULT.md](NATIVE-AMM-LP-VAULT.md) |
| **LP share token** | | optional | [NATIVE-LP-SHARE-TOKEN.md](NATIVE-LP-SHARE-TOKEN.md) |

**`register_pair` (directory):**

| Field | Value |
|-------|--------|
| `token_a` | |
| `token_b` | |
| `pool` | Must match your live pool id |

Use **real** reference-token ids for production; synthetic `0xaa…` / `0xbb…` is devnet-only.

## 3. Tutorial / dApp env cheat sheet

| Env / config | Typical value |
|--------------|----------------|
| `BOING_RPC_URL` | Your JSON-RPC |
| `BOING_POOL_HEX` | Native CP pool |
| `BOING_DEX_FACTORY_HEX` | Pair directory (merge override when RPC omits hint) |
| `TOKEN_IN` / `TOKEN_OUT` | For `print-native-dex-routes` |
| Node **`BOING_CANONICAL_NATIVE_DEX_FACTORY`** | Optional RPC advertisement — see [OPS-CANONICAL-TESTNET-NATIVE-DEX-AUX.md](OPS-CANONICAL-TESTNET-NATIVE-DEX-AUX.md) |

## 4. Verify on RPC

- Pool: `npm run fetch-native-amm-reserves` with `BOING_POOL_HEX`.
- Factory: `boing_getLogs` / SDK helpers for `register_pair` `Log3` ([NATIVE-DEX-FACTORY.md](NATIVE-DEX-FACTORY.md)).
- Audit: repo root `npm run audit-native-dex-testnet` ([OPS-CANONICAL-TESTNET-NATIVE-DEX-AUX.md](OPS-CANONICAL-TESTNET-NATIVE-DEX-AUX.md) § Verification).

## Appendix A — Example snapshot (public testnet, one operator)

**Not normative.** Illustrates a real path: **pool + directory** used **nonce** deploy (CREATE2 slots busy); **routers** matched **canonical CREATE2** predictions for deployer `0xc063512f…` (same as [`canonical-testnet-dex-predicted.json`](../scripts/canonical-testnet-dex-predicted.json)).

| Role | Account id |
|------|------------|
| RPC | `https://testnet-rpc.boing.network` |
| Deployer / sender | `0xc063512f42868f1278c59a1f61ec0944785c304dbc48dec7e4c41f70f666733f` |
| Native CP pool (v1, **nonce**) | `0x20a236ffa501f96204780e2b940b18f252d970a60400ce29531cc414cef60112` |
| Pair directory (**nonce**) | `0x5fffaea0269c6460a766e05bdd4584f87b3e0e39569b3a1a61231c1c2a506fc8` |
| Ledger router v1 (**CREATE2**, matches canonical) | `0x371b4cd7e3b88e06e6b89bdc86214918a7e7ec73b62deb7f9975e4166736d54d` |
| Multihop / swap2 router (**CREATE2**, matches canonical) | `0x43a6410510e7d742db8366347a343af6f7d2d1aec39b8281677d5643a7fc110b` |
| Ledger router v2 (**CREATE2**, matches canonical) | `0x60a232b91d6f86a61d037ea6ea0fb769897f983c8e0d399e3df5189d00868992` |
| Ledger router v3 (**CREATE2**, matches canonical) | `0xfb552619b27dacacba52b62d97cd171eabe4a74dac262ecb0e8735284d7555ba` |

**Canonical JSON differs for pool/factory** (those rows assume CREATE2 with no collision): pool `0xce4f8193…`, factory `0x12dff976…` in `canonical-testnet-dex-predicted.json`.

**LP product path (optional):** deploy vault + share at predicted CREATE2 ids with `npm run deploy-native-dex-lp-aux-contracts` (from repo root or `examples/native-boing-tutorial`). Matches JSON keys **`native_amm_lp_vault`** (`0x2b195b93…`) and **`native_lp_share_token`** (`0x0618b4a6…`) when using the same deployer and bytecode as [`scripts/canonical-testnet-dex-predicted.json`](../scripts/canonical-testnet-dex-predicted.json).

**Demo `register_pair` token placeholders:** `0x` + 64× `aa` and 64× `bb` (not production assets).

**Example reserves (ledger units):** reserve A `1000`, reserve B `2000`, total LP `1000` after seeding.
