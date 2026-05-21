# Testnet operations runbook (umbrella)

**Routing:** For a single **testnet + RPC + infra** map (tables only), see [TESTNET-RPC-INFRA.md](TESTNET-RPC-INFRA.md).

**Audience:** Operators bringing **bootnodes**, **public JSON-RPC**, **faucet**, **VibeMiner-visible connectivity**, **canonical native AMM pool**, and **optional monitoring / browser QA** online.

This page **links** the detailed docs; it does not duplicate every command. For copy-paste command tables see [PRE-VIBEMINER-NODE-COMMANDS.md](PRE-VIBEMINER-NODE-COMMANDS.md).

---

## Current status snapshot (2026-05-21)

| Surface | Status | Verify |
|---------|--------|--------|
| **Public RPC** | **Live** — `https://testnet-rpc.boing.network/` | `npm run preflight-rpc` |
| **QA transparency RPC** | **Live** — registry + pool config | `npm run verify-public-testnet-rpc` |
| **Canonical CP pool** | **`0x7247ddc3…`** (reserve A readable) | `BOING_REQUIRE_NONZERO_RESERVE=1 npm run check-canonical-pool` |
| **QA registry vs docs** | **Matches** canonical JSON | `npm run verify-qa-alignment` |
| **Explorer** | **Live** — [boing.observer](https://boing.observer) | `/qa`, `/tokens`, `/asset/:address` |
| **Chain tip** | Operator-dependent (may reset after redeploy) | `npm run observer-chain-tip-poll` |

**Note:** After a chain reset, **`head_height`** may be **0** while native DEX contracts from a prior bootstrap remain readable. Treat **`boing_getNetworkInfo.end_user`** as the live contract hint source.

---

## 1. Go-live sequence (order matters)

| Phase | Doc | What to verify |
|-------|-----|----------------|
| Genesis + binaries | [READINESS.md](READINESS.md) §1 | `cargo test`, `boing-sdk` tests |
| Bootnodes + P2P | [INFRASTRUCTURE-SETUP.md](INFRASTRUCTURE-SETUP.md), [TESTNET.md](TESTNET.md) §1–6 | Two stable multiaddrs, firewall **4001** |
| Validators / height | [RUNBOOK.md](RUNBOOK.md) | Height advances, peers connected |
| Public RPC + tunnel | [RUNBOOK.md](RUNBOOK.md) §8.3, [NETWORK-GO-LIVE-CHECKLIST.md](NETWORK-GO-LIVE-CHECKLIST.md) | No HTTP **530** / **1033** on public URL |
| RPC smoke (internet) | [NETWORK-GO-LIVE-CHECKLIST.md](NETWORK-GO-LIVE-CHECKLIST.md) | **`npm run preflight-rpc`** or **`check-testnet-rpc`** with public **`BOING_RPC_URL`** ([PRE-VIBEMINER-NODE-COMMANDS.md](PRE-VIBEMINER-NODE-COMMANDS.md)) |
| Faucet | [TESTNET.md](TESTNET.md), [RUNBOOK.md](RUNBOOK.md) | `boing-node --faucet-enable`; test `boing_faucetRequest` |
| Website / portal env | [READINESS.md](READINESS.md) §3.3, [WEBSITE-AND-DEPLOYMENT.md](WEBSITE-AND-DEPLOYMENT.md) | `PUBLIC_BOOTNODES`, `PUBLIC_TESTNET_RPC_URL` (or product-specific names) |
| VibeMiner | [VIBEMINER-INTEGRATION.md](VIBEMINER-INTEGRATION.md) | App sees bootnodes after above |
| Optional: canonical AMM pool | [OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md](OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md) § Published | **2026-05-21** live pool **`0x7247ddc3…`**; CI: **`check-canonical-pool`** |

---

## 2. Environment variables (integrators)

Consolidated matrix (website, boing.finance, tutorials, node flags): [TESTNET-RPC-INFRA.md](TESTNET-RPC-INFRA.md) §2.

| Consumer | Typical vars | Notes |
|----------|--------------|--------|
| **Website / portal** | Public bootnode list, public RPC URL | [READINESS.md](READINESS.md) §3.3 |
| **boing.finance** | `boingCanonicalTestnetPool.js`, **`REACT_APP_BOING_NATIVE_AMM_POOL`**, chain **6913** pool in `contracts.js` | [OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md](OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md) §3; self-hosted RPC: [DEVNET-OPERATOR-NATIVE-AMM.md](DEVNET-OPERATOR-NATIVE-AMM.md) |
| **Tutorial / scripts** | `BOING_RPC_URL`, `BOING_POOL_HEX` when using AMM helpers | Default pool: **`0x7247ddc3…`** via **`boing-sdk`** |
| **`boing-node` (relaxed testnet)** | `BOING_RATE_PROFILE=dev` or `--dev-rate-limits` | [RUNBOOK.md](RUNBOOK.md) § Dev rate-limit profile |
| **`boing-node` (public testnet RPC)** | **`BOING_CHAIN_ID=6913`**, **`BOING_CHAIN_NAME=Boing Testnet`** for **`boing_getNetworkInfo`** — [`tools/boing-node-public-testnet.env.example`](../tools/boing-node-public-testnet.env.example) | [RUNBOOK.md](RUNBOOK.md) §8.2, [PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md](PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md) |

Canonical pool **`AccountId`** is **`0x7247ddc3180fdc4d3fd1e716229bfa16bad334a07d28aa9fda9ad1bfa7bdacc3`** ([RPC-API-SPEC.md](RPC-API-SPEC.md), [TESTNET.md](TESTNET.md) §5.3, [OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md](OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md) § Published, **2026-05-21**). Confirm **boing.finance** env matches; daily CI runs **`check-canonical-pool`** + QA RPC probes.

---

## 3. Monitoring without a hosted observer

**boing.observer** (full explorer) is a **separate deploy** — see [BOING-OBSERVER-AND-EXPRESS.md](BOING-OBSERVER-AND-EXPRESS.md). Until that exists, operators can:

1. **JSON-RPC poll** — Tutorial script **`npm run observer-chain-tip-poll`** ([observer-chain-tip-poll.mjs](../examples/native-boing-tutorial/scripts/observer-chain-tip-poll.mjs)): logs **`boing_chainHeight`** + **`boing_getSyncState`** on an interval; optional stall warning via **`BOING_STALL_WARN_SECS`**. Set **`BOING_POLL_ONCE=1`** for a single sample (exit **1** on RPC error). Point **`BOING_RPC_URL`** at public RPC or tunnel origin.
2. **Existing probes** — `npm run check-testnet-rpc`, `node scripts/verify-public-testnet-rpc.mjs`, `npm run probe-rpc` (see [PRE-VIBEMINER-NODE-COMMANDS.md](PRE-VIBEMINER-NODE-COMMANDS.md)).
3. **Indexer scripts** — `npm run indexer-chain-tips` / `indexer-ingest-tick` for durable-tip visibility ([INDEXER-RECEIPT-AND-LOG-INGESTION.md](INDEXER-RECEIPT-AND-LOG-INGESTION.md)).

For a **production-grade** explorer backend (durable blocks/receipts/logs, reorg handling, read API), see **[OBSERVER-HOSTED-SERVICE.md](OBSERVER-HOSTED-SERVICE.md)** (OBS-1).

**Deployed D1 observer Worker** ([`examples/observer-d1-worker`](../examples/observer-d1-worker/)):

- Point **uptime / synthetic checks** at **`GET /api/readiness`** (or **`HEAD /api/readiness`**) — not **`/health`** alone — so failures include **RPC** and **D1** (see [OBSERVER-HOSTED-SERVICE.md](OBSERVER-HOSTED-SERVICE.md) §8.1).
- **`BOING_READINESS_MAX_LAG_FINALIZED`** (template **512**) only triggers **503** after **`readiness_lag_guard_armed`** flips (**auto** when **`lagVsFinalized ≤ BOING_READINESS_ARM_WHEN_LAG_LTE`** on a cron tick — migration **`0003`**). Apply **`d1:apply:remote`** when upgrading.
- Local probe: **`npm run check-observer-readiness -- <worker-origin>`** ([`scripts/check-observer-readiness.mjs`](../scripts/check-observer-readiness.mjs)).

---

## 4. Playwright + Boing Express (browser smoke)

The harness in **`examples/native-boing-playwright`** uses **headed** Chromium, loads an **unpacked Boing Express**, and expects a **human** to unlock the wallet (**`headless: false`**). That is why **default GitHub-hosted CI** only installs dependencies and runs tests that **skip** without **`BOING_EXPRESS_EXTENSION_PATH`**.

**Ops options:**

- **Local / manual:** [examples/native-boing-playwright/README.md](../examples/native-boing-playwright/README.md), [NATIVE-AMM-E2E-SMOKE.md](NATIVE-AMM-E2E-SMOKE.md).
- **CI with a display:** Use a **self-hosted** runner (or another environment with a real or virtual display) where the extension path exists and an operator can complete unlock, **or** maintain a private pipeline that fits your security model. See [PLAYWRIGHT-E2E-CI-OPS.md](PLAYWRIGHT-E2E-CI-OPS.md) for secret naming and limitations.

---

## 5. Upgrading the public RPC binary

When replacing **`boing-node`** behind the public URL, follow [PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md](PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md) (build, test, tunnel, **`check-testnet-rpc`**, rollback).

---

## 6. Security and incidents

| Topic | Doc |
|-------|-----|
| Disclosure | [SECURITY-STANDARDS.md](SECURITY-STANDARDS.md) §5 |
| Incident steps | [RUNBOOK.md](RUNBOOK.md) §6 |

---

## 7. After canonical pool publish (**OPS-1**)

1. Follow the table in [OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md](OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md) §2 (RPC-API-SPEC, TESTNET, checklist, monorepo apps).
2. Run tutorial **`BOING_POOL_HEX=<canonical> npm run fetch-native-amm-reserves`** against public RPC.
3. Update **out-of-repo** frontends per OPS doc §3.

---

*Keep this file as the high-level map; put new operator procedures in the linked specialist docs.*
