# Native AMM LP vault (Boing VM)

> 👋 **Everyday users:** this is a specialist document. Start at [README.md](README.md) for user / developer / operator paths.
> 🛠️ **Developers:** keep this aligned with shipped code.
> 🛰️ **Operators:** treat the sections below as the working spec for this topic.


Bytecode: `native_amm_lp_vault_bytecode()` in `crates/boing-execution/src/native_amm_lp_vault.rs`.

## Purpose

1. **`configure(pool, share_token)`** (once): stores the native CP **pool** and [NATIVE-LP-SHARE-TOKEN.md](./NATIVE-LP-SHARE-TOKEN.md) contract.
2. **`deposit_add(inner, min_lp)`**: copies **128-byte** pool **`add_liquidity`** calldata, **`Call`s** the pool with **32-byte** return buffer, requires **non-zero** return and **`returned_lp ≥ min_lp`**, then **`Call`s** share token **`mint(Caller, returned_lp)`** so the **transaction signer** receives shares.

Pool **LP** from that add accrues to **`Caller` = vault** (see [NATIVE-AMM-CALLDATA.md](./NATIVE-AMM-CALLDATA.md) `add_liquidity` return data).

## Selectors

| Low byte | Name | Outer calldata |
|----------|------|----------------|
| **`0xC0`** | `configure` | **96** bytes: word0 + **pool** + **share_token** |
| **`0xC1`** | `deposit_add` | **192** bytes: word0 + **128-byte** inner `add_liquidity` + **`min_lp`** word |

## Atomicity caveat

The Boing VM does **not** roll back nested **`Call`s**. If the pool call succeeds but the vault aborts (e.g. **`min_lp`** vs return word), **pool state may already be updated** while **share `mint`** does not run. Align **`min_lp`** with the inner **`min_liquidity`** and treat outer **`min_lp`** as an extra guard, not a cross-call revert.

## Access list (SDK)

For **`deposit_add`**, declare **signer**, **vault**, **pool**, and **share token** on both **read** and **write** (nested `Call`s). TypeScript: `buildNativeAmmLpVaultDepositAddAccessList` / `buildNativeAmmLpVaultDepositAddContractCallTx` and `mergeNativeAmmLpVaultDepositAddAccessListWithSimulation` in `boing-sdk` `nativeAmmLpVault.ts`.

For **`configure`**, **signer** + **vault** suffice: `buildNativeAmmLpVaultConfigureAccessList`.

## dApp readiness probe (**boing-sdk**)

**`fetchNativeAmmLpVaultProductReadiness(client, { vaultHex32, shareHex32, expectedPoolHex32? })`** uses **`boing_getContractStorage`** on the vault (**configured** / **pool** / **share** slots) and the share token **minter** slot. When **`depositAddReady`** is **`true`**, vault **`configure`** has run, the stored share id matches **`shareHex32`**, the minter is the vault, and the stored pool matches **`expectedPoolHex32`** when you pass it (e.g. merged default pool from **`fetchNativeDexIntegrationDefaults`**). Use this in UIs to gate **`deposit_add`** until ops finish **`set_minter_once`** + **`configure`**. Lower-level: **`fetchNativeAmmLpVaultStorageSnapshot`**, **`fetchLpShareTokenMinterAccountHex`**, storage key exports **`NATIVE_AMM_LP_VAULT_KEY_*_HEX`**, **`LP_SHARE_MINTER_KEY_HEX`**.

## Tutorial (Node CLI)

From [examples/native-boing-tutorial](../examples/native-boing-tutorial/README.md) (after `npm install` in that package and a built **`boing-sdk`**):

- **`npm run native-amm-lp-vault-print-contract-call-tx`** — read-only JSON for Boing Express / `contract_call` (**§7f** env table).
- **`npm run native-amm-lp-vault-submit-contract-call`** — **`submitContractCallWithSimulationRetry`** against the vault (**§7g**).

Pair with [NATIVE-LP-SHARE-TOKEN.md](./NATIVE-LP-SHARE-TOKEN.md) (**`set_minter_once`** + share **`npm` scripts**, **§7h–§7i**).

## CREATE2

Salt: **`NATIVE_AMM_LP_VAULT_CREATE2_SALT_V1`** (`BOING_AMM_LP_VAULT_V1`, zero-padded).

Dump bytecode:

```bash
cargo run -p boing-execution --example dump_native_amm_lp_vault
```

## Canonical public testnet (chain **6913**)

For deployer **`0xc063512f42868f1278c59a1f61ec0944785c304dbc48dec7e4c41f70f666733f`** and current `boing-execution` bytecode, the predicted **`AccountId`** is:

**`0x2b195b93a57b632ca3c1cf58cb7578542a6d58998116cddb8a6a50f1bd652f48`**

- JSON mirror: [`scripts/canonical-testnet-dex-predicted.json`](../scripts/canonical-testnet-dex-predicted.json) key **`native_amm_lp_vault`**
- **boing-sdk:** **`CANONICAL_BOING_TESTNET_NATIVE_AMM_LP_VAULT_HEX`**
- Batch deploy (CREATE2): **`npm run deploy-native-dex-lp-aux-contracts`** from repo root or [examples/native-boing-tutorial](../examples/native-boing-tutorial/README.md) — see §7c2c

After deploy, **`configure(pool, share_token)`** must still be submitted against **your** live native CP pool and the paired [LP share token](./NATIVE-LP-SHARE-TOKEN.md) account ( **`set_minter_once`** the vault first on the share contract).

## Storage keys (vault contract)

- **`NATIVE_AMM_LP_VAULT_KEY_CONFIGURED`** — non-zero after successful configure.
- **`NATIVE_AMM_LP_VAULT_KEY_POOL`** / **`NATIVE_AMM_LP_VAULT_KEY_SHARE_TOKEN`** — configured addresses.
