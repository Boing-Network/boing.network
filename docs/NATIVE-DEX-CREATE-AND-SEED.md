# Native DEX create-and-seed factory

> 👋 **Everyday users:** Create Pool on boing.finance asks for **one** Express approval when this factory is published.
> 🛠️ **Developers:** CREATE2 + `set_tokens` + optional `register_pair` in one `contract_call`; seed liquidity in the same Batch as the user.
> 🛰️ **Operators:** Deploy this factory once (CREATE2 salt below); publish its AccountId for Finance / SDK.

Complements the register-only pair directory ([NATIVE-DEX-FACTORY.md](./NATIVE-DEX-FACTORY.md)).

## Why

Custody pool create used to need a top-level **`ContractDeploy*`** (QA path) plus bind / seed / register. Express / protocol **Batch** steps are `transfer` | `contract_call` only — deploy cannot join a batch — so true one-sig create required either a multi-tx wallet intent or an in-contract **CREATE2** factory.

This factory is that CREATE2 product:

1. Caller supplies **pool runtime bytecode** in calldata (v8–v11 templates from `boing-execution` / boing-sdk). Embedding the ~16 KiB pool in the factory would blow the 32 KiB QA cap (no `CODECOPY`).
2. Factory **`CREATE2`**s the pool, **`CALL`s `set_tokens`**, **`CALL`s `set_fee_admin(tx sender)`** (so pre-LP fee control is not stuck on the factory), optionally **`CALL`s directory `register_pair`**, emits **`Log3`**, returns the pool id. When a directory is set, the factory reads **`pairs_count` before and after** `register_pair` and **faults** if the count did not increase — so a directory soft-`STOP` abort (full / duplicate / zero) is not treated as Call “success”. Pool runtime still comes from calldata — apply the canonical QA **v1–v7 hash blocklist** so CREATE2 rejects ledger-only templates.
3. **Liquidity seed** stays a separate step with **`CALLER` = tx sender** (custody pools pull with `transfer_from(CALLER,…)` and mint LP to `CALLER`). Nesting `add_liquidity` inside the factory would mint LP to the factory.

## One user approval (Finance / SDK)

Compose **one** Batch (or equivalent Express confirmation):

| Step | Action |
|------|--------|
| (optional) | `wBOING.deposit` with native value when one side is BOING |
| 1–2 | `approve` each token **to the predicted pool** (`create2(factory, salt, poolBytecode)`) |
| 3 | `create_and_seed` on this factory |
| 4 | `add_liquidity` on the new pool |

Predict the pool id before signing so approvals target the right AccountId.

## Calldata (`create_and_seed`, selector `0xC0`)

| Offset | Content |
|--------|---------|
| 0 | selector word (`0xC0` in low byte) |
| 32 | CREATE2 salt (32 bytes) |
| 64 | `token_a` AccountId |
| 96 | `token_b` AccountId |
| 128 | directory AccountId (**zero** = skip `register_pair`) |
| 160 | `bytecode_len` (u128 word; must be `1..=32768`) |
| 192 | pool **runtime** bytes (`bytecode_len` long) |

## CREATE2 / logs

| Item | Value |
|------|--------|
| Factory deploy salt | `NATIVE_DEX_CREATE_AND_SEED_CREATE2_SALT_V1` (`BOING_NATIVEDEX_CREATESEED_V1`) — **bytecode hash changed** when pairs_count verify landed; redeploy for the soft-STOP guard (existing CREATE2 addresses keep prior runtime) |
| Child address | `create2(factory, salt, pool_bytecode)` — same as top-level CREATE2 |
| Success `Log3` topic0 | `NATIVE_DEX_CREATE_AND_SEED_TOPIC` |
| topic1 / topic2 / data | `token_a` / `token_b` / pool id |

In-tx CREATE2 uses the existing QA path (`purpose` treated as `dapp`). Child templates that are `Unsure` under the live registry must still use a top-level deploy.

## Rust / SDK

- **Bytecode:** `native_dex_create_and_seed_bytecode()` in `crates/boing-execution/src/native_dex_create_and_seed.rs`
- **Encode:** `encode_create_and_seed_calldata(salt, token_a, token_b, directory, pool_bytecode)`
- **Dump:** `cargo run -p boing-execution --example dump_native_dex_create_and_seed`
- **TypeScript:** `nativeDexCreateAndSeed.ts` in boing-sdk (`encodeNativeDexCreateAndSeedCalldata`, `buildCreateAndSeedBatchCalls`, predictors)

## Relation to the pair directory

Keep using the existing directory for discovery (`register_pair` / `boing_listDexPools`). Pass its AccountId as `directory` so create-and-seed both deploys and lists the pair. The directory bytecode stays register-only.
