# Native AMM LP vault (Boing VM)

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

## Storage keys (vault contract)

- **`NATIVE_AMM_LP_VAULT_KEY_CONFIGURED`** — non-zero after successful configure.
- **`NATIVE_AMM_LP_VAULT_KEY_POOL`** / **`NATIVE_AMM_LP_VAULT_KEY_SHARE_TOKEN`** — configured addresses.
