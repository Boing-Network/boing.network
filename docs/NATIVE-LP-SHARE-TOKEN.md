# Native LP share token (Boing VM)

> 👋 **Everyday users:** this is a specialist document. Start at [README.md](README.md) for user / developer / operator paths.
> 🛠️ **Developers:** keep this aligned with shipped code.
> 🛰️ **Operators:** treat the sections below as the working spec for this topic.


Bytecode: `lp_share_token_bytecode()` in `crates/boing-execution/src/native_lp_share_token.rs`.

## Purpose

**Transferable** bookkeeping for vault or protocol-defined “shares”: **`set_minter_once`**, **`mint`** (minter only), **`transfer`**. Balances use the same XOR storage layout style as the reference fungible template (`LP_SHARE_BALANCE_XOR`).

## Selectors

| Low byte | Name | Calldata size |
|----------|------|----------------|
| **`0x07`** | `set_minter_once(minter)` | **64** bytes |
| **`0x06`** | `mint(to, amount)` | **96** bytes |
| **`0x01`** | `transfer(to, amount)` | **96** bytes |

## CREATE2

Salt: **`NATIVE_LP_SHARE_TOKEN_CREATE2_SALT_V1`** (`BOING_LP_SHARE_TOKEN_V1`, zero-padded).

```bash
cargo run -p boing-execution --example dump_native_lp_share_token
```

## Canonical public testnet (chain **6913**)

For deployer **`0xc063512f42868f1278c59a1f61ec0944785c304dbc48dec7e4c41f70f666733f`** and current `boing-execution` bytecode, the predicted **`AccountId`** is:

**`0x0618b4a6a30bc31822a0cdcf253ed2bcf642a6cecf26346ba655b63fccbde03c`**

- JSON mirror: [`scripts/canonical-testnet-dex-predicted.json`](../scripts/canonical-testnet-dex-predicted.json) key **`native_lp_share_token`**
- **boing-sdk:** **`CANONICAL_BOING_TESTNET_NATIVE_LP_SHARE_TOKEN_HEX`**
- Batch deploy (CREATE2): **`npm run deploy-native-dex-lp-aux-contracts`** — see [examples/native-boing-tutorial README §7c2c](../examples/native-boing-tutorial/README.md)

End-to-end order: deploy share + vault → **`set_minter_once`** (minter = vault) → vault **`configure`** → users **`deposit_add`** on the vault.

## Access list + Express tx (`boing-sdk`)

Declare **signer** + **share token contract** on both **read** and **write**: `buildLpShareTokenAccessList`, `buildLpShareTokenContractCallTx`, `mergeLpShareTokenAccessListWithSimulation` (`nativeLpShareToken.ts`).

## Tutorial (Node CLI)

From [examples/native-boing-tutorial](../examples/native-boing-tutorial/README.md):

- **`npm run native-lp-share-print-contract-call-tx`** — read-only **`contract_call`** JSON (**§7h**).
- **`npm run native-lp-share-submit-contract-call`** — simulate → merge → submit (**§7i**).

End-to-end: deploy share token → **`set_minter_once`** the vault as minter → deploy/configure [NATIVE-AMM-LP-VAULT.md](./NATIVE-AMM-LP-VAULT.md) → users call **`deposit_add`** on the vault to receive share **`mint`**s.
