# Native DEX pair directory (Boing VM)

This document specifies the **pair directory** bytecode produced by `native_dex_factory_bytecode()` in `crates/boing-execution/src/native_dex_factory.rs`. It complements:

- [NATIVE-AMM-CALLDATA.md](./NATIVE-AMM-CALLDATA.md) — constant-product **pool** contract
- [BOING-L1-DEX-ENGINEERING.md](./BOING-L1-DEX-ENGINEERING.md) — product / topology notes

## Why this exists

Boing VM contracts **cannot deploy child contracts** from bytecode (`ContractDeploy` is applied only from transactions; there is no `CREATE` opcode). A Uniswap-style **factory that deploys pairs in-contract** is therefore **not implementable** as pure VM bytecode without a protocol change.

The supported pattern is:

1. Deploy each **native CP pool** with `ContractDeploy` (+ optional Create2 salt per pool template — see native AMM docs).
2. Deploy **one** pair-directory contract (this spec) with `ContractDeploy` (+ `NATIVE_DEX_FACTORY_CREATE2_SALT_V1` for a canonical address).
3. Call **`register_pair`** on the directory to publish `(token_a, token_b, pool_account_id)` for indexers and dApps.

Lookup by token pair is **off-chain** in the protocol: scan **`Log3`** events (`nativeDexFactoryLogs` in **boing-sdk**), or index `get_pair_at` / storage locally. On-chain **O(1) map (token_a, token_b) → pool** would require a hash opcode or unbounded search in bytecode; neither is assumed here.

**SDK helper:** `findNativeDexFactoryPoolByTokens` reads `pairs_count` from storage, then runs **`boing_simulateTransaction`** for each `get_pair_at(i)` until a matching `(token_a, token_b)` pair is found (either order). Use a **low** `maxPairsToScan` on busy RPCs. **Tutorial:** `examples/native-boing-tutorial/scripts/deploy-native-dex-directory.mjs` deploys the directory (and optionally registers a pool).

## Limits

- At most **4096** entries (`NATIVE_DEX_FACTORY_MAX_PAIRS`). Further `register_pair` calls **stop** without state change (same silent failure style as invalid native AMM calls).

## Calldata

All layouts use **32-byte words** (selector in the **low byte** of word0), like reference token / native AMM.

| Selector | Byte | Calldata length | Purpose |
|----------|------|-----------------|--------|
| `register_pair` | `0xD0` | **128** | Word0 selector; word1 `token_a`; word2 `token_b`; word3 `pool` |
| `pairs_count` | `0xD1` | **32** | Returns **32** bytes: count as a word (low **8** bytes) |
| `get_pair_at` | `0xD2` | **64** | Word0 selector; word1 index (**u64** in low **8** bytes). Returns **96** bytes: `token_a`, `token_b`, `pool` |

## Storage layout

- **Count** — `native_dex_factory_count_key()`: one word; number of registered pairs.
- **Triplet** *i* (0-based) — keys `native_dex_factory_triplet_base_word() + (i * 4 + f)` for `f ∈ {0,1,2}` (token_a, token_b, pool). Arithmetic is **256-bit** unsigned.

## Logs

Successful **`register_pair`** emits **`Log3`**:

- **topic0** — `NATIVE_DEX_FACTORY_TOPIC_REGISTER`
- **topic1** — `token_a` (32-byte word)
- **topic2** — `token_b` (32-byte word)
- **data** — `pool` id (32 bytes)

## Tooling

- **Rust:** `cargo run -p boing-execution --example dump_native_dex_factory` — bytecode hex
- **TypeScript:** `boing-sdk` — `nativeDexFactory.ts` (calldata), `nativeDexFactoryPool.ts` (access lists, `fetchNativeDexFactoryPairsCount`, return-data decoders), `nativeDexFactoryLogs.ts` (`Log3` parsing), `create2.ts` (`predictNativeDexFactoryCreate2Address`)
- **Ledger router (optional):** [NATIVE-DEX-LEDGER-ROUTER.md](./NATIVE-DEX-LEDGER-ROUTER.md) — forward pool calls through one contract (**v1–v3** widths).
- **Two-hop swap router (optional):** [NATIVE-DEX-SWAP2-ROUTER.md](./NATIVE-DEX-SWAP2-ROUTER.md) — two sequential pool **`Call`**s in one tx (`nativeDexSwap2Router.ts`).

## Routers and liquidity lockers

A **router** that **`Call`s** a pool forwards **`Caller` = router**, which breaks **v2/v4** native pools that **`transfer` output to `Caller`**. Users should **call the pool directly** from the signer for token-hook pools, or use **v1/v3** ledger-style pools when experimenting with nested calls.

**Liquidity locker** semantics (time locks, LP NFTs, etc.) are **not** part of this bytecode; they require additional programs and/or pool surface changes.
