# OPS — Fresh testnet operator keys + canonical address rotation

> 👋 **Everyday users:** this is a specialist document. Start at [README.md](README.md) for user / developer / operator paths.
> 🛠️ **Developers:** keep this aligned with shipped code.
> 🛰️ **Operators:** treat the sections below as the working spec for this topic.


Use this when **no one depends on the current published pool/deployer** and you have **lost** the old `BOING_SECRET_HEX` (32-byte Ed25519 seed) or you are **wiping** chain state and want one coherent set of CREATE2 predictions in git + on-chain.

## What you are replacing

- **`BOING_SECRET_HEX`** — **private** signing seed (`0x` + 64 hex). It is **not** the pool id (`BOING_POOL_HEX`). It is **never** stored in this repository.
- **`BOING_EXPECT_SENDER_HEX` / deployer** — **public** `AccountId` derived from that seed. All native CREATE2 addresses in this repo are keyed off **one** deployer account (pool + DEX factory + routers + vault + share token).

## 1. Generate a new operator key (local only)

From the repo:

```bash
cd boing-sdk && npm install && npm run generate-operator-key
```

Or from the monorepo root:

```bash
npm run generate-testnet-operator-key
```

The JSON prints **`BOING_SECRET_HEX`** and **`BOING_EXPECT_SENDER_HEX`**. **Save the secret** in a password manager or offline store. **Do not** commit it or paste it into tickets.

## 2. Fund the new deployer on chain

Use the faucet (public testnet) or genesis allocation (devnet) so **`BOING_EXPECT_SENDER_HEX`** has enough **native BOING** to pay deploy fees and later liquidity actions.

## 3. Print the CREATE2 manifest

Pass the **public** deployer hex:

```bash
# Send **only** JSON to the file (Cargo may print build lines to stderr).
cargo run -p boing-execution --example print_native_create2_manifest -- 0x<YOUR_64_HEX_DEPLOYER> 2>/dev/null > scripts/canonical-testnet-published.manifest.json
```

On Windows **Git Bash**, `2>/dev/null` works; in **cmd**, run the binary from `target\release\examples\` or redirect stderr with `2>nul`.

Save stdout to a file, e.g. `canonical-testnet-published.manifest.json` (public addresses only).

## 4. Sync this repository

Applies the manifest to **`boing-sdk`**, **`scripts/`**, **`website/`**, **`crates/boing-execution` examples**, **`tools/*.env.example`**, and selected **`docs/`** (string-replace of the **previous** published pool + deployer).

```bash
node scripts/sync-canonical-testnet-from-manifest.mjs canonical-testnet-published.manifest.json
```

For a **second** rotation later, point replacement at the last published values:

```bash
CANONICAL_SYNC_PREVIOUS_POOL=0x... CANONICAL_SYNC_PREVIOUS_DEPLOYER=0x... \
  node scripts/sync-canonical-testnet-from-manifest.mjs next-manifest.json
```

Then rebuild and sanity-check:

```bash
cd boing-sdk && npm run build && npx vitest run tests/canonicalTestnet.test.ts tests/dexIntegration.test.ts
cargo run -p boing-execution --example verify_canonical_cp_pool_create2_drift
```

You want **`create2_matches_published: true`** after sync (published row = current bytecode CREATE2).

## 5. Deploy on-chain (same secret)

Use **`BOING_SECRET_HEX`** and **`BOING_EXPECT_SENDER_HEX`** with the tutorial scripts (CREATE2 on):

1. **CP pool (v1)** — `examples/native-boing-tutorial`: `npm run deploy-native-amm-pool` (bytecode from `dump_native_amm_pool`; see [DEVNET-OPERATOR-NATIVE-AMM.md](DEVNET-OPERATOR-NATIVE-AMM.md)).
2. **DEX factory** — `npm run deploy-native-dex-directory` (and optional `register_pair` envs).
3. **Routers / vault / share** — same pattern with the corresponding dump examples and CREATE2 salts (predicted addresses are already in the manifest).

The deployed **`AccountId`s** must match the manifest (same deployer + salts + bytecode as this `main`).

## 6. Optional RPC hints

Set on public RPC nodes (see [RPC-API-SPEC.md](RPC-API-SPEC.md), [tools/boing-node-public-testnet.env.example](../tools/boing-node-public-testnet.env.example)):

- **`BOING_CANONICAL_NATIVE_CP_POOL`** — `native_cp_pool_v1` from the manifest  
- **`BOING_CANONICAL_NATIVE_DEX_FACTORY`** — `native_dex_factory`

## 7. Verify

```bash
npm run check-canonical-pool
npm run audit-native-dex-testnet
```

Adjust **`BOING_RPC_URL`** if you are not hitting public testnet.

## Related

- [OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md](OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md) — published pool checklist (after you freeze a new id)  
- [OPS-CANONICAL-TESTNET-NATIVE-DEX-AUX.md](OPS-CANONICAL-TESTNET-NATIVE-DEX-AUX.md) — DEX aux predictions  
- [PRE-VIBEMINER-NODE-COMMANDS.md](PRE-VIBEMINER-NODE-COMMANDS.md) — script index
