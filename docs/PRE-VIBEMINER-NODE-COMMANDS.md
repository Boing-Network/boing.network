# Commands and scripts — before VibeMiner node rollout

**Purpose:** One place to copy commands after **bootnodes / validators / public RPC** are up, so you can finish verification and optional “future work” flows before upgrading **VibeMiner** to rely on live nodes.

**Maintenance:** When you add or rename a **repo-root** or **tutorial** `npm run` script, a new **`scripts/*.mjs`**, change **`.github/workflows/boing-sdk-rpc-integration.yml`**, or change **`website/functions/api/networks.js`** (**`BOING_TESTNET_DOWNLOAD_TAG`**, **`buildNetworksMeta`**, bootnode defaults), update this document and [VIBEMINER-INTEGRATION.md](VIBEMINER-INTEGRATION.md) §3.1 / §6 (and [NEXT-STEPS-FUTURE-WORK.md](NEXT-STEPS-FUTURE-WORK.md) if it is a notable slice).

**Conventions**

- Paths are relative to the **repository root** unless stated otherwise.
- Most JSON-RPC scripts use **`BOING_RPC_URL`** (trailing slash optional). Examples:
  - Local: `http://127.0.0.1:8545`
  - Public: `https://testnet-rpc.boing.network` (or your tunnel URL)
- **Go-live order** (genesis → bootnodes → block production → RPC → verify): [NETWORK-GO-LIVE-CHECKLIST.md](NETWORK-GO-LIVE-CHECKLIST.md). **Broader ops** (website env, VibeMiner, AMM **OPS-1**, monitoring): [TESTNET-OPS-RUNBOOK.md](TESTNET-OPS-RUNBOOK.md). **Self-hosted RPC + deploy pool + liquidity:** [DEVNET-OPERATOR-NATIVE-AMM.md](DEVNET-OPERATOR-NATIVE-AMM.md). **Public RPC node upgrades:** [PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md](PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md).
- **Per-script env tables** (secrets, block ranges, pool hex): [examples/native-boing-tutorial/README.md](../examples/native-boing-tutorial/README.md).
- **Piping JSON stdout:** Prefer **`node scripts/<name>.mjs`** from the tutorial directory (or full path from repo root). **`npm run`** can print lifecycle lines that break **`JSON.parse`** on stdout.

---

## 1. Suggested order (when nodes are running)

1. **Build & test** the Rust workspace (§2).
2. **Start** `boing-node` (validator / full node / faucet flags per [RUNBOOK.md](RUNBOOK.md) §8.1 for **P2P tx gossip** + **`--max-connections-per-ip`**, [TESTNET.md](TESTNET.md)).
3. **Optional infra helpers** — bootnode shell scripts, Cloudflare tunnel, config alignment (§3).
4. **RPC smoke** — raw probes (§3), **`preflight-rpc`** or **`check-testnet-rpc`** (§4–§5), SDK **`probe-rpc`** (§6).
5. **SDK** — build, unit tests, optional live **`verify`** (§6).
6. **Tutorial scripts** — indexer, logs, AMM, transfers (§5); install tutorial deps once:  
   `cd examples/native-boing-tutorial && npm install`
7. **Optional browser smoke** — Playwright + Boing Express (§7).

---

## 2. Rust workspace (`cargo`)

From repo root:

```bash
cargo build --release
cargo test
```

Run a local validator with RPC (example; adjust `--data-dir`, P2P, genesis as you use in prod):

```bash
./target/release/boing-node --validator --rpc-port 8545 --data-dir ./data
```

**Public testnet RPC (chain id in `boing_getNetworkInfo`):** before starting the process, set **`BOING_CHAIN_ID=6913`** and **`BOING_CHAIN_NAME=Boing Testnet`** (see [`tools/boing-node-public-testnet.env.example`](../tools/boing-node-public-testnet.env.example)):

```bash
export BOING_CHAIN_ID=6913
export BOING_CHAIN_NAME="Boing Testnet"
./target/release/boing-node --validator --rpc-port 8545 --data-dir ./data
```

**Rate limits / mempool**

| Mechanism | Notes |
|-----------|--------|
| **`--dev-rate-limits`** | Relaxed HTTP + mempool (**64** pending/sender default for this profile) — [RUNBOOK.md](RUNBOOK.md) § Dev rate-limit profile |
| **`BOING_RATE_PROFILE=dev`** | Same effect as **`--dev-rate-limits`** for JSON-RPC + mempool defaults |
| **`BOING_RATE_PROFILE=mainnet`** | Forces strict profile even if **`--dev-rate-limits`** is set (public RPC safety) |
| **`--pending-txs-per-sender N`** | Override mempool cap only (mainnet default **16**) |

Debug binary (faster compile for dev):

```bash
cargo build -p boing-node
./target/debug/boing-node --validator --rpc-port 8545 --data-dir ./data
```

**Native AMM pool bytecode dump** (for `deploy-native-amm-pool` / `pool.hex`):

```bash
cargo run -p boing-execution --example dump_native_amm_pool > pool.hex
```

**Native DEX pair-directory bytecode dump** (for `deploy-native-dex-directory`):

```bash
cargo run -p boing-execution --example dump_native_dex_factory > dex-factory.hex
```

---

## 3. Repo `scripts/` (bootnodes, tunnel, probes)

| Command | Purpose |
|--------|---------|
| `bash scripts/start-bootnode-1.sh` | Primary bootnode (see script for flags) |
| `bash scripts/start-bootnode-2.sh` | Secondary bootnode |
| `scripts/start-bootnode-1.bat` / `start-bootnode-2.bat` | Windows variants |
| `scripts/start-cloudflare-tunnel.bat` | Tunnel helper for public RPC hostname |
| `node scripts/check-cloudflared-alignment.mjs` | Verify `~/.cloudflared/config.yml` routes **`testnet-rpc.boing.network`** → local **8545** (optional **`CLOUDFLARED_CONFIG=...`**) |
| `node scripts/verify-public-testnet-rpc.mjs` | HTTPS probe: `boing_chainHeight`, `boing_getQaRegistry`, `boing_qaPoolConfig` (default URL or **`TESTNET_RPC_URL=...`**) |
| `BOING_RPC_URL=http://127.0.0.1:8545 node scripts/rpc-endpoint-check.mjs` | Raw JSON-RPC matrix **without** building `boing-sdk` |

Overview: [scripts/README.md](../scripts/README.md). Full infra: [INFRASTRUCTURE-SETUP.md](INFRASTRUCTURE-SETUP.md).

---

## 4. Monorepo root (`package.json` scripts)

From repo root, **`npm install` at the root is not required** for these wrappers. **`boing-sdk`** and **`examples/native-boing-tutorial`** each need **`npm install`** / **`npm ci`** (and **`boing-sdk`** **`npm run build`** where scripts import **`dist/`**) before running tutorial flows.

**Tutorial parity:** every script name in **`examples/native-boing-tutorial/package.json`** is also defined at the **repository root** with the same name. Most delegate with **`npm run <script> --prefix examples/native-boing-tutorial`**. **`probe-rpc`** at the root is **`node boing-sdk/scripts/probe-rpc.mjs`** (the same file the tutorial **`probe-rpc`** runs). Examples: **`transfer`**, **`dump-native-bytecodes`**, **`bootstrap-native-pool-and-dex`**, **`print-native-dex-deploy-salts`**, **`deploy-native-purpose-contract`**, **`deploy-native-dex-aux-contracts`**, **`deploy-native-dex-lp-aux-contracts`**, **`deploy-native-amm-pool`**, **`deploy-native-dex-directory`**, **`fetch-native-amm-reserves`**, **`native-amm-submit-contract-call`**, **`fetch-blocks-range`**, … — authoritative list: that package’s **`package.json`**. Per-script env tables: [examples/native-boing-tutorial/README.md](../examples/native-boing-tutorial/README.md).

| Command | What it runs |
|--------|----------------|
| **`npm run <tutorial-script>`** | Same as **`cd examples/native-boing-tutorial && npm run <tutorial-script>`** (see §5 and tutorial README) |
| `npm run probe-rpc` | `node boing-sdk/scripts/probe-rpc.mjs` — build **`boing-sdk`** first |
| `npm run rpc-endpoint-check` | `node scripts/rpc-endpoint-check.mjs` — raw JSON-RPC matrix without **`boing-sdk`** |
| `npm run check-canonical-pool` | **`scripts/check-canonical-native-amm-pool.mjs`** — canonical pool reserve probe |
| `npm run check-observer-readiness` | **`scripts/check-observer-readiness.mjs`** — deployed observer worker readiness |
| `npm run observer-ingest-ref-tick` | **`examples/observer-ingest-reference`** **`ingest-tick`** (`npm install` there first) |
| `npm run observer-ingest-sqlite-tick` | Same package, **`ingest-sqlite-tick`** — **`BOING_SQLITE_PATH`** (Node 22+ **`node:sqlite`**) |
| `npm run native-amm-e2e` | Playwright package (§7) |
| `npm run boing:smoke` | **`scripts/boing-smoke.mjs`** |

**Full probe after preflight:** `BOING_PROBE_FULL=1 npm run preflight-rpc` (first step honors **`BOING_PROBE_FULL`** via **`check-testnet-rpc`**).

---

## 5. Tutorial package — `examples/native-boing-tutorial`

**One-time setup:**

```bash
cd boing-sdk && npm install && npm run build
cd ../examples/native-boing-tutorial && npm install
```

**All `npm run` scripts** (each is `node scripts/<name>.mjs` unless noted; env details in tutorial README):

| Command | Notes |
|---------|--------|
| `npm run preflight-rpc` | Single-process **`preflight-rpc.mjs`**: **`boing_chainHeight`** (+ optional full probe) then best-effort **`boing_getSyncState`** (exit **1** if height fails). Optional **`BOING_PROBE_FULL=1`**. Avoids Windows **`UV_HANDLE_CLOSING`** from chained **`spawnSync`**. |
| `npm run check-testnet-rpc` | Preflight **`boing_chainHeight`**; optional **`BOING_PROBE_FULL=1`** |
| `npm run probe-rpc` | Same as SDK probe (requires **`boing-sdk` built**) |
| `npm run transfer` | Needs **`BOING_SECRET_HEX`**, **`BOING_TO_HEX`** |
| `npm run contract-call` | Reference token; needs **`BOING_CONTRACT_HEX`** |
| `npm run deploy-minimal` | Minimal contract deploy; **`BOING_SECRET_HEX`** |
| `npm run dump-native-bytecodes` | **`cargo`** dump → **`artifacts/pool-lines.hex`** + **`artifacts/native-dex-factory.hex`** (no keys; from tutorial cwd) |
| `npm run bootstrap-native-pool-and-dex` | Pool deploy + directory deploy (+ optional **`BOING_BOOTSTRAP_REGISTER_PAIR=1`**); needs **`BOING_SECRET_HEX`**; on CREATE2 “address in use” retries with **`BOING_USE_CREATE2=0`** unless **`BOING_BOOTSTRAP_NO_AUTO_NONCE=1`** — tutorial README §7c1 |
| `npm run print-native-dex-deploy-salts` | JSON: canonical CREATE2 salt hex for native routers / vault / share token ( **`boing-sdk`** built) |
| `npm run deploy-native-purpose-contract` | Generic purpose deploy: bytecode file + **`BOING_CREATE2_SALT_HEX`** (or nonce deploy); tutorial README §7c2b |
| `npm run deploy-native-dex-aux-contracts` | Multihop router + ledger routers v2–v3 (optional v1); **`BOING_SKIP_DUMP=1`** if **`artifacts/`** ready — tutorial README §7c2c |
| `npm run deploy-native-dex-lp-aux-contracts` | LP vault + LP share token (canonical CREATE2); **`BOING_SKIP_DUMP=1`** if **`artifacts/`** ready — tutorial README §7c2c |
| `npm run deploy-native-amm-pool` | Pool deploy; bytecode file/hex + **`BOING_SECRET_HEX`** |
| `npm run deploy-native-dex-directory` | Native **pair directory** deploy (+ optional second tx: **`register_pair`** when **`BOING_DEX_POOL_HEX`** + token ids set); [NATIVE-DEX-FACTORY.md](NATIVE-DEX-FACTORY.md) |
| `npm run native-amm-print-contract-call-tx` | Print **`contract_call`** JSON (swap/add/remove); optional **`BOING_TOKEN_A_HEX`** / **`BOING_TOKEN_B_HEX`** |
| `npm run native-amm-submit-contract-call` | Submit pool **`contract_call`** (seed liquidity / scripted swap); **`BOING_SECRET_HEX`** + same action env as print |
| `npm run native-amm-lp-vault-print-contract-call-tx` | LP vault configure / deposit-add JSON — [NATIVE-AMM-LP-VAULT.md](NATIVE-AMM-LP-VAULT.md) |
| `npm run native-amm-lp-vault-submit-contract-call` | Submit LP vault **`contract_call`** |
| `npm run native-lp-share-print-contract-call-tx` | LP share token print — [NATIVE-LP-SHARE-TOKEN.md](NATIVE-LP-SHARE-TOKEN.md) |
| `npm run native-lp-share-submit-contract-call` | Submit LP share **`contract_call`** |
| `npm run fetch-logs-range` | **`BOING_FROM_BLOCK`**, **`BOING_TO_BLOCK`** |
| `npm run fetch-blocks-range` | **`BOING_FROM_HEIGHT`**, **`BOING_TO_HEIGHT`** |
| `npm run indexer-chain-tips` | Sync / durable tips |
| `npm run indexer-ingest-tick` | Plan catch-up; **`BOING_FETCH=1`** to fetch; optional **`BOING_OMIT_MISSING=1`** (pruned RPC) |
| `npm run observer-chain-tip-poll` | JSON-RPC poll: height + **`boing_getSyncState`**; **`BOING_POLL_ONCE=1`** for one sample (exit **1** on error) — [TESTNET-OPS-RUNBOOK.md](TESTNET-OPS-RUNBOOK.md) §3 |
| `npm run fetch-native-amm-reserves` | Needs **`BOING_POOL_HEX`** |
| `npm run fetch-native-amm-logs` | Needs **`BOING_POOL_HEX`** |
| `npm run print-native-dex-routes` | Off-chain CP routes: **`TOKEN_IN`**, **`TOKEN_OUT`**, optional **`AMOUNT_IN`**, **`BOING_FROM_BLOCK`**, **`BOING_TO_BLOCK`**, factory/pool overrides — [HANDOFF-DEPENDENT-PROJECTS.md](HANDOFF-DEPENDENT-PROJECTS.md); tutorial README §7c3 |

Example (public RPC sanity check, no keys):

```bash
cd examples/native-boing-tutorial
export BOING_RPC_URL=https://your-public-rpc.example/
npm run preflight-rpc
# Or step-by-step:
npm run check-testnet-rpc
BOING_PROBE_FULL=1 npm run check-testnet-rpc
BOING_POLL_ONCE=1 node scripts/observer-chain-tip-poll.mjs
```

**CI-style JSON check** (no npm noise on stdout), from **`examples/native-boing-tutorial`**:

```bash
BOING_SENDER_HEX=0xabababababababababababababababababababababababababababababababab \
  BOING_POOL_HEX=0xcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd \
  BOING_AMOUNT_IN=1 node scripts/native-amm-print-contract-call-tx.mjs
```

---

## 6. `boing-sdk` package

From `boing-sdk/`:

```bash
npm install    # or npm ci in CI
npm run build
npm test       # vitest unit tests
npm run verify # vitest + note about optional live RPC tests
```

**Live RPC integration tests** (with a node on 8545): **`verify`** runs Vitest including **`tests/rpcIntegration.test.ts`** — height / sync fallback, blocks + receipts range, **`getLogsChunked`** (if implemented), **`getTransactionReceipt`** for unknown tx → **`null`**.

```bash
cd boing-sdk
npm run build
BOING_INTEGRATION_RPC_URL=http://127.0.0.1:8545 npm run verify
```

Stricter expectation (full RPC surface; used in CI):

```bash
BOING_INTEGRATION_RPC_URL=http://127.0.0.1:8545 BOING_EXPECT_FULL_RPC=1 npm run verify
```

**Capability probe** (after `npm run build`):

```bash
BOING_RPC_URL=http://127.0.0.1:8545 npm run probe-rpc
```

---

## 7. Optional Playwright — `examples/native-boing-playwright`

Headed browser smoke (Boing Express + site). Requires extension path and browser install per that package’s README. **Default GitHub-hosted CI** cannot run the real headed E2E — [PLAYWRIGHT-E2E-CI-OPS.md](PLAYWRIGHT-E2E-CI-OPS.md).

From repo root:

```bash
npm run native-amm-e2e
```

Or from the package:

```bash
cd examples/native-boing-playwright
npm install
npx playwright install chromium
npm run test:e2e
# npm run test:e2e:headed
# npm run test:e2e:debug
```

---

## 8. CI parity (GitHub Actions)

| Workflow | What it validates |
|----------|-------------------|
| **`.github/workflows/boing-sdk.yml`** | **`boing-sdk`**: **`npm ci`**, **`npm run build`**, **`npm test`** on SDK path changes |
| **`.github/workflows/boing-sdk-rpc-integration.yml`** | Builds **`boing-node`**, starts RPC **8545**, **`boing-sdk`**: **`probe-rpc`**, **`verify`** with **`BOING_EXPECT_FULL_RPC=1`**; **tutorial**: **`npm ci`**, **`preflight-rpc`** ( **`check-testnet-rpc`** + one-shot **`observer-chain-tip-poll`** ), **`indexer-ingest-tick`**, **`node scripts/native-amm-print-contract-call-tx.mjs`** + JSON **`ok`** check |
| **`.github/workflows/observer-ingest-d1.yml`** | **`boing-sdk` build** + **`examples/observer-d1-worker`**: **`tsc`**, **`wrangler deploy --dry-run`**; **`examples/observer-ingest-reference`**: **`node --check`** on ingest scripts |
| **`.github/workflows/native-boing-playwright.yml`** | Playwright package install + tests (**skip** without extension path) |

Use the RPC integration flow as a **local smoke** before publishing a new **`boing-node`** behind public RPC ([PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md](PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md)).

---

## 9. Related docs

| Doc | Use |
|-----|-----|
| [READINESS.md](READINESS.md) | Beta / launch checklist |
| [RUNBOOK.md](RUNBOOK.md) | Node flags, tunnel, HTTP 530 / 1033, monitoring §8.4 |
| [TESTNET.md](TESTNET.md) | Bootnodes, faucet, URLs |
| [NETWORK-GO-LIVE-CHECKLIST.md](NETWORK-GO-LIVE-CHECKLIST.md) | Operator sequence before announcing testnet |
| [TESTNET-OPS-RUNBOOK.md](TESTNET-OPS-RUNBOOK.md) | Umbrella: website env, **OPS-1**, monitoring, Playwright ops |
| [PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md](PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md) | Upgrading the node behind public JSON-RPC |
| [PLAYWRIGHT-E2E-CI-OPS.md](PLAYWRIGHT-E2E-CI-OPS.md) | Extension E2E vs default CI |
| [OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md](OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md) | Canonical testnet pool id (**published** — § Published); **`boing-sdk`** **`CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX`** |
| **`npm run check-canonical-pool`** (repo root) | No SDK build: **`boing_getContractStorage`** on canonical pool reserve A — default **`BOING_RPC_URL=https://testnet-rpc.boing.network/`**; override **`BOING_POOL_HEX`** if needed. **`BOING_REQUIRE_NONZERO_RESERVE=1`** fails if reserve A is zero (CI: **`.github/workflows/canonical-pool-public-rpc.yml`**) |
| **`npm run check-observer-readiness -- <worker-origin>`** | **`GET /api/readiness`** on a deployed **`observer-d1-worker`** — pass/fail exit code; **`BOING_OBSERVER_USE_HEAD=1`** for **HEAD** only. See [OBSERVER-HOSTED-SERVICE.md](OBSERVER-HOSTED-SERVICE.md) §8.1, [TESTNET-OPS-RUNBOOK.md](TESTNET-OPS-RUNBOOK.md) §3 |
| [NEXT-STEPS-FUTURE-WORK.md](NEXT-STEPS-FUTURE-WORK.md) | Larger backlog items |
| [EXECUTION-PARITY-TASK-LIST.md](EXECUTION-PARITY-TASK-LIST.md) | VM / receipts / RPC task history |

*Add commands here when you introduce new scripts; keep detailed env tables in the tutorial README.*
