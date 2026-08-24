# OPS — Canonical public testnet native AMM pool (`AccountId`)

**Hosted Fly testnet (2026-08-24):** `https://testnet-rpc.boing.network/` currently returns **`end_user.canonical_native_cp_pool: null`**. The hex in § Published is the **previous tunnel ledger** — do not treat it as live on the Fly cluster until ops re-runs `npm run deploy-native-dex-full-stack` and sets `BOING_CANONICAL_NATIVE_*` on the nodes. Live source of truth: `boing_getNetworkInfo.end_user`.

**Goal (OPS-1):** When operations freeze a **long-lived** constant-product pool on **Boing testnet (chain id 6913)**, publish its **32-byte pool `AccountId`** once and mirror it everywhere integrators look. This doc is the **checklist**; it does **not** contain a placeholder fake address.

**Lost signing seed / chain reset:** [OPS-FRESH-TESTNET-BOOTSTRAP.md](OPS-FRESH-TESTNET-BOOTSTRAP.md) — new `BOING_SECRET_HEX`, CREATE2 manifest, repo sync, redeploy.

**Context:** See [TESTNET-RPC-INFRA.md](TESTNET-RPC-INFRA.md). **VibeMiner** users run **`boing-node`** locally for JSON-RPC but do not set the pool id in the app — see [VIBEMINER-INTEGRATION.md](VIBEMINER-INTEGRATION.md) §5.2.

## Published (canonical public testnet)

| Field | Value |
|-------|--------|
| **Pool `AccountId`** | `0x7247ddc3180fdc4d3fd1e716229bfa16bad334a07d28aa9fda9ad1bfa7bdacc3` |
| **Bytecode** | **v1** constant-product pool (full-stack deploy on public RPC) |
| **Deploy** | Operator-published id above — **use this hex** for RPC reads, dApp defaults, and **`npm run check-canonical-pool`**. |
| **Deployer (documented)** | `0x3b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| **Date** | **2026-05-21** (previous public tunnel ledger; **not** currently advertised on hosted Fly `end_user`) |
| **Verification** | **`npm run check-canonical-pool`** with **`BOING_REQUIRE_NONZERO_RESERVE=1`**; reserves via **`npm run fetch-native-amm-reserves`** with **`BOING_POOL_HEX`**. Deploy record: [NATIVE-DEX-OPERATOR-DEPLOYMENT-RECORD.md](NATIVE-DEX-OPERATOR-DEPLOYMENT-RECORD.md) **Appendix B**. |

**Related live contracts (same deploy):** factory `0x58112627…`, multihop router `0xf801cd1a…`, ledger v2/v3, LP vault, LP share — see **`boing-sdk`** `canonicalTestnetDex.ts` and [`scripts/canonical-testnet-published.manifest.json`](../scripts/canonical-testnet-published.manifest.json).

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

**Tutorial / scripts** use **`BOING_POOL_HEX`** or `import { CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX } from 'boing-sdk'` after **`npm run build`** in **`boing-sdk`**.

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

## References

| Doc | Role |
|-----|------|
| [NATIVE-AMM-CALLDATA.md](NATIVE-AMM-CALLDATA.md) | Calldata, storage keys, logs |
| [NATIVE-AMM-INTEGRATION-CHECKLIST.md](NATIVE-AMM-INTEGRATION-CHECKLIST.md) | Manual wallet + dApp smoke |
| [BOING-DAPP-INTEGRATION.md](BOING-DAPP-INTEGRATION.md) | Native CP swap integration |
