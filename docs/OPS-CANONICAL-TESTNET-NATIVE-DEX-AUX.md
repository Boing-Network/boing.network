# OPS — Canonical public testnet native DEX aux contracts (predicted CREATE2)

**Goal:** Mirror the same **predictability story** as the canonical CP pool ([OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md](OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md)): fixed **deployer** + **CREATE2** + **frozen bytecode** ⇒ **known `AccountId`s** before anyone submits deploy txs. This doc lists those predictions and how to verify them on RPC.

**Track what you actually deployed** (nonce vs CREATE2, your pool id, etc.): [NATIVE-DEX-OPERATOR-DEPLOYMENT-RECORD.md](NATIVE-DEX-OPERATOR-DEPLOYMENT-RECORD.md).

**Lost the deployer seed or resetting testnet?** Use [OPS-FRESH-TESTNET-BOOTSTRAP.md](OPS-FRESH-TESTNET-BOOTSTRAP.md) (`generate-testnet-operator-key`, `print-native-create2-manifest`, `sync-canonical-testnet-manifest`).

**Important:** Until ops includes matching **`ContractDeploy`** transactions on chain **6913**, these accounts are **empty** — the addresses are still correct as **targets**, not a guarantee of live code.

## Why “gaps” showed up (pool documented, factory/routers/vault/share not)

1. **Separate deploy steps** — The canonical pool was an early, explicit ops publish. Factory, ledger routers, multihop router, LP vault, and LP share token are **additional** native programs. Nothing invents their on-chain presence without **deploy txs** (tutorial scripts, operator automation, etc.).
2. **RPC surface** — `boing_getNetworkInfo` may carry `end_user.canonical_native_dex_factory` (and related hints) on a **full node**, but **public edge RPC** sometimes **omits** that method or field (allowlist, proxy, older binary). Apps then saw **no** factory hint even though the chain could support DEX reads.
3. **No universal “code at address” read** — Without a portable **`boing_getContractCode`**, “is this contract deployed?” is weaker than **storage** (pool reserves) or **logs** (factory `register_pair`). The audit script uses **reserves + optional log scan** as practical signals.

## Published predictions (6913, deployer below)

**Deployer** (same as canonical testnet CP pool ops doc):

| Field | Value |
|-------|--------|
| **Deployer `AccountId`** | `0xc063512f42868f1278c59a1f61ec0944785c304dbc48dec7e4c41f70f666733f` |

**Predicted `AccountId`s** (from current `boing-execution` bytecode; JSON mirror: [`scripts/canonical-testnet-dex-predicted.json`](../scripts/canonical-testnet-dex-predicted.json)):

| Contract | Predicted `AccountId` |
|----------|---------------------|
| Native DEX factory | `0x12dff97625620a1f10c05cd66cd72878288e8fea70d4150e9815bd38983b2890` |
| Ledger router v1 | `0x371b4cd7e3b88e06e6b89bdc86214918a7e7ec73b62deb7f9975e4166736d54d` |
| Ledger router v2 | `0x60a232b91d6f86a61d037ea6ea0fb769897f983c8e0d399e3df5189d00868992` |
| Ledger router v3 | `0xfb552619b27dacacba52b62d97cd171eabe4a74dac262ecb0e8735284d7555ba` |
| Multihop swap router | `0x43a6410510e7d742db8366347a343af6f7d2d1aec39b8281677d5643a7fc110b` |
| AMM LP vault | `0x2b195b93a57b632ca3c1cf58cb7578542a6d58998116cddb8a6a50f1bd652f48` |
| LP share token | `0x0618b4a6a30bc31822a0cdcf253ed2bcf642a6cecf26346ba655b63fccbde03c` |

**boing-sdk:** import from `canonicalTestnetDex.js` or the package root re-exports (`CANONICAL_BOING_TESTNET_NATIVE_DEX_FACTORY_HEX`, …).

**Regenerate** after bytecode changes:

```bash
cargo run -p boing-execution --example print_canonical_testnet_dex_create2_addresses -- --json
# paste stdout into scripts/canonical-testnet-dex-predicted.json and sync boing-sdk/src/canonicalTestnetDex.ts
```

## Node configuration (optional but nice)

When the factory is deployed at the predicted address, set on **RPC nodes** that should advertise it (see [RPC-API-SPEC.md](RPC-API-SPEC.md)):

- **`BOING_CANONICAL_NATIVE_DEX_FACTORY`** — factory `AccountId` hex (should match table above, e.g. `0x12dff97625620a1f10c05cd66cd72878288e8fea70d4150e9815bd38983b2890` for the canonical deployer + CREATE2 path).

**Operator deploy order (typical):** pool + directory → `npm run deploy-native-dex-aux-contracts` (routers) → optional `npm run deploy-native-dex-lp-aux-contracts` (LP vault + share token at predicted CREATE2 ids). After factory is live at the predicted id, restart or configure nodes with **`BOING_CANONICAL_NATIVE_DEX_FACTORY`** so `boing_getNetworkInfo` can surface **`canonical_native_dex_factory`** to wallets and indexers.

Tutorial: [examples/native-boing-tutorial/README.md](../examples/native-boing-tutorial/README.md) §7c2c.

## Verification

- **Automated:** from repo root, `npm run audit-native-dex-testnet` (uses `BOING_RPC_URL`, optional `BOING_AUDIT_MAX_BLOCKS` = max block index inclusive for log scan, `BOING_AUDIT_STRICT_POOL`).
- **Manual:** pool reserves via tutorial `fetch-native-amm-reserves`; factory activity via `boing_getLogs` on the predicted factory address and the `register_pair` topic (see `boing-sdk` / `nativeDexFactory.ts`).

## Downstream

After ops confirms deploys, align **website**, **tutorial env**, and **partner apps** with the same hex values so chain **6913** defaults match what is actually live.
