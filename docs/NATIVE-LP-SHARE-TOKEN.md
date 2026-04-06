# Native LP share token (Boing VM)

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

## Access list + Express tx (`boing-sdk`)

Declare **signer** + **share token contract** on both **read** and **write**: `buildLpShareTokenAccessList`, `buildLpShareTokenContractCallTx`, `mergeLpShareTokenAccessListWithSimulation` (`nativeLpShareToken.ts`).

## Tutorial (Node CLI)

From [examples/native-boing-tutorial](../examples/native-boing-tutorial/README.md):

- **`npm run native-lp-share-print-contract-call-tx`** — read-only **`contract_call`** JSON (**§7h**).
- **`npm run native-lp-share-submit-contract-call`** — simulate → merge → submit (**§7i**).

End-to-end: deploy share token → **`set_minter_once`** the vault as minter → deploy/configure [NATIVE-AMM-LP-VAULT.md](./NATIVE-AMM-LP-VAULT.md) → users call **`deposit_add`** on the vault to receive share **`mint`**s.
