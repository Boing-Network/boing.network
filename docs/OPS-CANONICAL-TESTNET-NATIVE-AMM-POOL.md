# OPS — Canonical public testnet native AMM pool (`AccountId`)

> 👋 **Everyday users:** this is a specialist document. Start at [README.md](README.md) for user / developer / operator paths.
> 🛠️ **Developers:** keep this aligned with shipped code.
> 🛰️ **Operators:** treat the sections below as the working spec for this topic.


**Hosted Fly testnet (verified 2026-11-01):** `https://testnet-rpc.boing.network/` publishes **`end_user.canonical_native_cp_pool`** (and related `canonical_native_*` ids) matching § Published. Live source of truth remains `boing_getNetworkInfo.end_user` — re-check after any chain reset or `BOING_CANONICAL_NATIVE_*` change.

**Goal (OPS-1):** When operations freeze a **long-lived** constant-product pool on **Boing testnet (chain id 6913)**, publish its **32-byte pool `AccountId`** once and mirror it everywhere integrators look. This doc is the **checklist**; it does **not** contain a placeholder fake address.

**Lost signing seed / chain reset:** see [§ Fresh bootstrap / address rotation](#fresh-bootstrap--address-rotation) below.

**Context:** See [TESTNET-RPC-INFRA.md](TESTNET-RPC-INFRA.md). Operators who run **`boing-node`** locally get JSON-RPC from their own node; the node does not store a pool id. Deploying a pool is a normal **contract deploy** + **`boing_submitTransaction`** flow ([NATIVE-AMM-CALLDATA.md](NATIVE-AMM-CALLDATA.md)).

## Published (canonical public testnet)

| Field | Value |
|-------|--------|
| **Pool `AccountId`** | `0x5b60d6f8514d1a8a40340fa354951d778b0821cd3ff7e534d0bea50f67d38976` |
| **Bytecode** | **v1** constant-product pool (full-stack deploy on public RPC) |
| **Deploy** | Operator-published id above — **use this hex** for RPC reads, dApp defaults, and **`npm run check-canonical-pool`**. |
| **Deployer (documented)** | `0x8079f30559c02c5fe087c9ad1bc3b751a29de35992f3f8431734243e825f696e` |
| **Date** | **2026-05-21** deploy; **re-advertised** on hosted Fly `end_user` (verified **2026-11-01**) |
| **Verification** | **`npm run check-canonical-pool`** with **`BOING_REQUIRE_NONZERO_RESERVE=1`**; reserves via **`npm run fetch-native-amm-reserves`** with **`BOING_POOL_HEX`**. Deploy record: [NATIVE-DEX-OPERATOR-DEPLOYMENT-RECORD.md](NATIVE-DEX-OPERATOR-DEPLOYMENT-RECORD.md) **Appendix B**. |

**Related live contracts (same deploy):** factory `0x7322ccec…`, multihop router `0xd8504729…`, ledger v2/v3, LP vault, LP share — see **`boing-sdk`** `canonicalTestnetDex.ts` and [`scripts/canonical-testnet-published.manifest.json`](../scripts/canonical-testnet-published.manifest.json).

**CREATE2 prediction (deployer `0xc063512f…`):** A **fresh** deploy using **`constant_product_pool_bytecode()`** + **`NATIVE_CP_POOL_CREATE2_SALT_V1`** lands at **`0xce4f819369630e89c4634112fdf01e1907f076bc30907f0402591abfca66518d`** in today's tree — that is **not** the live public pool id. New deploys should use **`predictedPoolHex`** from **`npm run deploy-native-amm-pool`**. Run:

```bash
cargo run -p boing-execution --example verify_canonical_cp_pool_create2_drift
```

**Downstream:** Set the same pool hex in **boing.finance** (`boingCanonicalTestnetPool.js` / **`REACT_APP_BOING_NATIVE_AMM_POOL`**) and **`website/src/config/testnet.ts`**.

### Superseded publication (2026-04-03)

| Field | Value |
|-------|--------|
| **Pool `AccountId`** | `0xce4f819369630e89c4634112fdf01e1907f076bc30907f0402591abfca66518d` |
| **Deployer** | `0xc063512f42868f1278c59a1f61ec0944785c304dbc48dec7e4c41f70f666733f` |

Replaced by the **2026-05-21** row above after **`deploy-native-dex-full-stack`** on public RPC. Do **not** point new integrators at the superseded hex.

---

## What maintainers need from you (when the pool is deployed)

Paste (in a PR, issue, or direct message to whoever updates docs/repos):

1. **Pool `AccountId`** — **64 hex characters** (with or without `0x`), lowercase preferred. This is the contract address after the deploy is **included** on the target chain.
2. **Deploy method** — **CREATE2** with salt **`NATIVE_CP_POOL_CREATE2_SALT_V1`** or **`NATIVE_CP_POOL_CREATE2_SALT_V2`** (from `boing_execution::native_amm`; **v2** = token-hook bytecode) **or** **nonce-derived** deploy. If CREATE2, only the **deployer `AccountId`** (64 hex) is needed to verify the address; if nonce-based, send **deployer + deploy transaction nonce** (decimal). **Which salt** must match **which bytecode line** from `dump_native_amm_pool` (see [NATIVE-AMM-CALLDATA.md](NATIVE-AMM-CALLDATA.md) § CREATE2).
3. **Network** — Confirm **testnet chain id `6913`** (or name the network if different).
4. **Optional proof** — Block height or **tx id** (32-byte hex) of the deploy, and/or a **`boing_getContractStorage`** screenshot/log showing reserve keys after first liquidity — helps auditors match the id to an on-chain deploy.

With (1)–(3), maintainers can update [RPC-API-SPEC.md](RPC-API-SPEC.md) § Native AMM, [TESTNET.md](TESTNET.md), website/boing.finance pool constants, and runbooks without guessing.

## Who must produce the pool id?

**You / ops on a live chain** — not something a third party can “assign.” The pool **`AccountId`** is **deterministic from the deploy**:

- **Recommended (canonical testnet):** **CREATE2** with **`NATIVE_CP_POOL_CREATE2_SALT_V1`** and **`constant_product_pool_bytecode()`** (ledger-only), **or** **`NATIVE_CP_POOL_CREATE2_SALT_V2`** and **`constant_product_pool_bytecode_v2()`** (reference-token hooks) — address is stable for a given deployer + bytecode pair. See [NATIVE-AMM-CALLDATA.md](NATIVE-AMM-CALLDATA.md) § CREATE2.
- **Legacy / simple:** **Nonce-derived** (no `create2_salt`) — `nonce_derived_contract_address(sender, deploy_tx_nonce)`; same as the original `native_amm_rpc_happy_path` test.

**Precompute CREATE2 pool id:**

```bash
cargo run -p boing-execution --example dump_native_amm_pool   # stderr: v1 then v2 lengths; stdout: two hex lines
# Save the line you deploy (v1 or v2) to pool.hex — one line only, no comments.
cargo run -p boing-execution --example print_native_cp_create2_salt   # prints SALT_V1= and SALT_V2=
cargo run -p boing-primitives --example create2_contract_address -- \
  0x<YOUR_DEPLOYER_64_HEX> 0x<SALT_MATCHING_BYTECODE> pool.hex
```

**boing-sdk:** `predictNativeCpPoolCreate2Address` + **`NATIVE_CP_POOL_CREATE2_SALT_V1`** for v1; **`predictNativeCpPoolV2Create2Address`** + **`NATIVE_CP_POOL_CREATE2_SALT_V2`** for v2.

**Precompute (nonce-based deploy only):**

```bash
cargo run -p boing-primitives --example nonce_derived_contract_address -- \
  0x<YOUR_DEPLOYER_64_HEX> <DEPLOY_TX_NONCE_DECIMAL>
```

**Deploy bytecode hex:**

```bash
cargo run -p boing-execution --example dump_native_amm_pool
```

Then **`boing_qaCheck`** (purpose e.g. `dapp`) and **`boing_submitTransaction`** with **`create2_salt: Some(NATIVE_CP_POOL_CREATE2_SALT_V1)`** or **`…_V2`** matching the bytecode (or nonce-only) per [NATIVE-AMM-CALLDATA.md](NATIVE-AMM-CALLDATA.md). After inclusion, confirm reserves with **`boing_getContractStorage`** (§1 below).

**Prerequisites:** Pool contract is deployed from **`boing_execution::native_amm`** bytecode, passes **`boing_qaCheck`** with an allowed purpose (e.g. `dapp`), and has been smoke-tested (reserves via **`boing_getContractStorage`**, at least one successful **`swap`** / **`add_liquidity`**). See [NATIVE-AMM-CALLDATA.md](NATIVE-AMM-CALLDATA.md) and [NATIVE-AMM-INTEGRATION-CHECKLIST.md](NATIVE-AMM-INTEGRATION-CHECKLIST.md).

---

## 1. Record the canonical hex (internal)

Store the value as **`0x` + 64 lowercase hex characters** (32 bytes). Example shape only:

```text
CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX=0x<64_hex_chars>
```

Verify on public RPC (reserve A key = `0x` + 62×`0` + `01`; same as **`NATIVE_CONSTANT_PRODUCT_RESERVE_A_KEY_HEX`** in `boing-sdk`):

```bash
RPC="https://testnet-rpc.boing.network/"
POOL="0x<64_hex_chars>"
KEY_RESERVE_A="0x0000000000000000000000000000000000000000000000000000000000000001"
curl -s -X POST "$RPC" -H "Content-Type: application/json" \
  -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"boing_getContractStorage\",\"params\":[\"$POOL\",\"$KEY_RESERVE_A\"]}"
```

---

## 2. Update this repository (`boing.network`)

**Current publish (2026-05-21):** rows **1–5** below are **Done**. For a **future** canonical pool replacement, repeat in one PR.

| # | Location | Action |
|---|----------|--------|
| 1 | [RPC-API-SPEC.md](RPC-API-SPEC.md) — § **Native constant-product AMM** | **Done** — canonical hex in table (§ Published above). |
| 2 | [TESTNET.md](TESTNET.md) — § **5.3** | **Done** — same hex as RPC spec. |
| 3 | [NATIVE-AMM-INTEGRATION-CHECKLIST.md](NATIVE-AMM-INTEGRATION-CHECKLIST.md) | **Done** — **A1.5** / **A5.3** / **A6.4** updated for published hex (**2026-05-21**). |
| 4 | [NEXT-STEPS-FUTURE-WORK.md](NEXT-STEPS-FUTURE-WORK.md) | **Done** — **OPS-1** marked complete in scoped passes + backlog text. |
| 5 | This file + **`boing-sdk`** | **Done** — § Published below; **`CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX`** in [`boing-sdk/src/canonicalTestnet.ts`](../boing-sdk/src/canonicalTestnet.ts) (mirror of spec — bump when the on-chain canonical pool changes). |

**Tutorial / scripts** use **`BOING_POOL_HEX`** or `import { CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX } from '@boing-network/boing-sdk'` after **`npm run build`** in **`boing-sdk`**.

---

## 3. Update repositories outside this monorepo

| Area | Action |
|------|--------|
| **boing.finance** | Set **`CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX`** in **`frontend/src/config/boingCanonicalTestnetPool.js`** (or override via **`REACT_APP_BOING_NATIVE_AMM_POOL`** on Pages/build). Keep **`nativeConstantProductPool`** in `contracts.js` (or equivalent) for chain **6913** in sync. Redeploy. |
| **Website / portal** | **Done** — [boing.network/testnet/join](https://boing.network/testnet/join#native-amm-pool) shows copyable pool id (`website/src/config/testnet.ts` **`CANONICAL_NATIVE_CP_POOL_ACCOUNT_ID_HEX`**). |
| **Announcements** | Short post (Discord, X, blog): hex + link to [RPC-API-SPEC.md](RPC-API-SPEC.md) or this doc. |

---

## 4. Regression after publish

- **`cargo test -p boing-node --test native_amm_rpc_happy_path`** (still deploys a fresh pool in-test; unchanged).
- Optional: [examples/native-boing-playwright](../examples/native-boing-playwright/) headed smoke against **boing.finance** with Boing Express.
- Optional: `BOING_POOL_HEX=<canonical> npm run fetch-native-amm-reserves --prefix examples/native-boing-tutorial` against **`https://testnet-rpc.boing.network`**.

---

## Aux CREATE2 predictions (factory / routers / vault / share)

Mirror the same predictability story as the CP pool: fixed **deployer** + **CREATE2** + **frozen bytecode** ⇒ known `AccountId`s before deploy txs. Track what you actually deployed in [NATIVE-DEX-OPERATOR-DEPLOYMENT-RECORD.md](NATIVE-DEX-OPERATOR-DEPLOYMENT-RECORD.md).

**Important:** Until matching **`ContractDeploy`** transactions land on chain **6913**, predicted accounts may be **empty** — addresses are targets, not a guarantee of live code.

### Why “gaps” showed up historically

1. **Separate deploy steps** — pool was an early publish; factory/routers/vault/share need additional deploy txs.
2. **RPC surface** — public edge may omit `end_user.canonical_native_dex_*` on older binaries/proxies.
3. **Code-at-address reads** — prefer reserves + log scan when `boing_getContractCode` is unavailable.

### Published predictions (6913)

**Deployer** (CREATE2 ops key; may differ from the live pool’s historical deployer in § Published):

| Field | Value |
|-------|--------|
| **Deployer `AccountId`** | `0xc063512f42868f1278c59a1f61ec0944785c304dbc48dec7e4c41f70f666733f` |

| Contract | Predicted `AccountId` |
|----------|---------------------|
| Native DEX factory | `0x12dff97625620a1f10c05cd66cd72878288e8fea70d4150e9815bd38983b2890` |
| Ledger router v1 | `0x371b4cd7e3b88e06e6b89bdc86214918a7e7ec73b62deb7f9975e4166736d54d` |
| Ledger router v2 | `0x60a232b91d6f86a61d037ea6ea0fb769897f983c8e0d399e3df5189d00868992` |
| Ledger router v3 | `0xfb552619b27dacacba52b62d97cd171eabe4a74dac262ecb0e8735284d7555ba` |
| Multihop swap router | `0x8f8b2ecb6fd5dc7682e41ebe443d6116e0f4ae8247f67b4bfafec4dea2d861a3` |
| AMM LP vault | `0x2b195b93a57b632ca3c1cf58cb7578542a6d58998116cddb8a6a50f1bd652f48` |
| LP share token | `0x0618b4a6a30bc31822a0cdcf253ed2bcf642a6cecf26346ba655b63fccbde03c` |

JSON mirror: [`scripts/canonical-testnet-dex-predicted.json`](../scripts/canonical-testnet-dex-predicted.json). SDK: `canonicalTestnetDex.js` / package re-exports.

```bash
cargo run -p boing-execution --example print_canonical_testnet_dex_create2_addresses -- --json
```

When aux contracts are live at predicted addresses, set **`BOING_CANONICAL_NATIVE_DEX_FACTORY`**, **`BOING_CANONICAL_NATIVE_DEX_MULTIHOP_SWAP_ROUTER`**, ledger v2/v3, LP vault/share env on RPC nodes ([RPC-API-SPEC.md](RPC-API-SPEC.md) `end_user`). Typical order: pool + directory → `npm run deploy-native-dex-aux-contracts` → optional `npm run deploy-native-dex-lp-aux-contracts`. Verify: `npm run audit-native-dex-testnet`.

## Fresh bootstrap / address rotation

Use when **no one depends on the current published pool/deployer** and you have **lost** `BOING_SECRET_HEX` or are **wiping** chain state for one coherent CREATE2 set.

1. **Generate key (local only):** `npm run generate-testnet-operator-key` (or `cd boing-sdk && npm run generate-operator-key`). Save **`BOING_SECRET_HEX`** offline — never commit it.
2. **Fund** `BOING_EXPECT_SENDER_HEX` (faucet / genesis).
3. **Print manifest:** `cargo run -p boing-execution --example print_native_create2_manifest -- 0x<DEPLOYER> 2>/dev/null > scripts/canonical-testnet-published.manifest.json`
4. **Sync repo:** `node scripts/sync-canonical-testnet-from-manifest.mjs scripts/canonical-testnet-published.manifest.json` (second rotations: set `CANONICAL_SYNC_PREVIOUS_POOL` / `CANONICAL_SYNC_PREVIOUS_DEPLOYER`).
5. **Rebuild / check:** `cd boing-sdk && npm run build && npx vitest run tests/canonicalTestnet.test.ts`; `cargo run -p boing-execution --example verify_canonical_cp_pool_create2_drift` (want `create2_matches_published: true`).
6. **Deploy on-chain** with the same secret (pool, factory, routers, vault/share) — see [DEVNET-OPERATOR-NATIVE-AMM.md](DEVNET-OPERATOR-NATIVE-AMM.md) and tutorial README.
7. **Optional RPC hints:** `BOING_CANONICAL_NATIVE_CP_POOL`, `BOING_CANONICAL_NATIVE_DEX_FACTORY`, …
8. **Verify:** `npm run check-canonical-pool`, `npm run audit-native-dex-testnet`.

Fill-in deploy tables + historical snapshots: [NATIVE-DEX-OPERATOR-DEPLOYMENT-RECORD.md](NATIVE-DEX-OPERATOR-DEPLOYMENT-RECORD.md). Self-hosted stack: [DEVNET-OPERATOR-NATIVE-AMM.md](DEVNET-OPERATOR-NATIVE-AMM.md).

## References

| Doc | Role |
|-----|------|
| [NATIVE-AMM-CALLDATA.md](NATIVE-AMM-CALLDATA.md) | Calldata, storage keys, logs |
| [NATIVE-AMM-INTEGRATION-CHECKLIST.md](NATIVE-AMM-INTEGRATION-CHECKLIST.md) | Manual wallet + dApp smoke |
| [BOING-DAPP-INTEGRATION.md](BOING-DAPP-INTEGRATION.md) | Native CP swap integration |
