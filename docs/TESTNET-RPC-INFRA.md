# Testnet operations, public RPC, and infrastructure — one map

**Audience:** Operators and integrators who need **bootnodes**, **public JSON-RPC**, **Cloudflare tunnel / website env**, and **canonical native AMM** without hunting three parallel runbooks.

This file **does not replace** the specialist guides; it **overlaps their scope** on purpose so you pick the right doc once. Detailed commands stay in the linked pages.

---

## 1. Pick your doc (routing)

| You need… | Start here | Also useful |
|-----------|------------|-------------|
| **Order of go-live** (genesis → bootnodes → RPC → faucet → optional pool) | [NETWORK-GO-LIVE-CHECKLIST.md](NETWORK-GO-LIVE-CHECKLIST.md) | [TESTNET-OPS-RUNBOOK.md](TESTNET-OPS-RUNBOOK.md) §1 |
| **Copy-paste commands** (`cargo`, `boing-node`, tutorial `npm run`) | [PRE-VIBEMINER-NODE-COMMANDS.md](PRE-VIBEMINER-NODE-COMMANDS.md) | [RUNBOOK.md](RUNBOOK.md) |
| **Two-machine bootnode + tunnel + website secrets** | [INFRASTRUCTURE-SETUP.md](INFRASTRUCTURE-SETUP.md) | [TESTNET.md](TESTNET.md) §6 |
| **User-facing testnet** (join, faucet URL, bootnode table) | [TESTNET.md](TESTNET.md) | [READINESS.md](READINESS.md) §3 |
| **Upgrade / restart the node behind public RPC** | [PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md](PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md) | [RUNBOOK.md](RUNBOOK.md) §8 |
| **JSON-RPC methods, errors, canonical pool hex** | [RPC-API-SPEC.md](RPC-API-SPEC.md) § Native AMM | [BOING-RPC-ERROR-CODES-FOR-DAPPS.md](BOING-RPC-ERROR-CODES-FOR-DAPPS.md) |
| **Canonical native CP pool (OPS-1) + rotations** | [OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md](OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md) § Published | [NATIVE-AMM-E2E-SMOKE.md](NATIVE-AMM-E2E-SMOKE.md) |
| **Beta readiness / six pillars** | [READINESS.md](READINESS.md) | [BUILD-ROADMAP.md](BUILD-ROADMAP.md) |

**Umbrella operator narrative** (env matrix, monitoring without observer, Playwright notes): [TESTNET-OPS-RUNBOOK.md](TESTNET-OPS-RUNBOOK.md).

---

## 2. Environment variables (shared matrix)

Values must stay consistent across **website build**, **scripts**, and **downstream apps**.

| Consumer | Typical vars | Where documented |
|----------|----------------|------------------|
| **Cloudflare Pages / GitHub Actions (website)** | `PUBLIC_TESTNET_RPC_URL`, `PUBLIC_BOOTNODES` | [INFRASTRUCTURE-SETUP.md](INFRASTRUCTURE-SETUP.md), [WEBSITE-AND-DEPLOYMENT.md](WEBSITE-AND-DEPLOYMENT.md), [READINESS.md](READINESS.md) §3.3 |
| **boing.finance** | `CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX` in `frontend/src/config/boingCanonicalTestnetPool.js`; **`REACT_APP_BOING_NATIVE_AMM_POOL`** for build; `nativeConstantProductPool` for chain **6913** | [RPC-API-SPEC.md](RPC-API-SPEC.md) § Native constant-product AMM, [OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md](OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md) §3 |
| **This repo — tutorials / scripts** | `BOING_RPC_URL`, `BOING_POOL_HEX` when exercising AMM helpers | [examples/native-boing-tutorial/README.md](../examples/native-boing-tutorial/README.md) |
| **Public RPC node process** | Same as RUNBOOK; plus **`BOING_CHAIN_ID`** (e.g. **6913**) and optional **`BOING_CHAIN_NAME`** so **`boing_getNetworkInfo`** exposes them — [RPC-API-SPEC.md](RPC-API-SPEC.md) § **boing_getNetworkInfo** | [RUNBOOK.md](RUNBOOK.md) §8, [PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md](PUBLIC-RPC-NODE-UPGRADE-CHECKLIST.md) |

---

## 3. Canonical native AMM pool (integration contract)

1. **Source of truth for the hex:** [RPC-API-SPEC.md](RPC-API-SPEC.md) § **Native constant-product AMM** and [TESTNET.md](TESTNET.md) §5.3 — **`0x7247ddc3180fdc4d3fd1e716229bfa16bad334a07d28aa9fda9ad1bfa7bdacc3`** (published **2026-05-21**, [OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md](OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md) § Published). **`boing_getNetworkInfo.end_user`** on public RPC should match.
2. **Procedure to publish:** [OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md](OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md).
3. **Optional npm package:** use monorepo **`boing-sdk`** for RPC client helpers / `explainBoingRpcError` instead of duplicating formatters in app code ([BOING-RPC-ERROR-CODES-FOR-DAPPS.md](BOING-RPC-ERROR-CODES-FOR-DAPPS.md)).

---

## 4. After deploy — quick verification

| Layer | Check |
|-------|--------|
| **RPC from internet** | `npm run preflight-rpc` or `check-testnet-rpc` with public `BOING_RPC_URL` ([PRE-VIBEMINER-NODE-COMMANDS.md](PRE-VIBEMINER-NODE-COMMANDS.md)) |
| **Canonical native AMM pool on RPC** | `npm run check-canonical-pool` (repo root) — `boing_getContractStorage` on published pool ([scripts/check-canonical-native-amm-pool.mjs](../scripts/check-canonical-native-amm-pool.mjs)). **GitHub Actions:** [canonical-pool-public-rpc.yml](../.github/workflows/canonical-pool-public-rpc.yml) (daily + manual; **`BOING_REQUIRE_NONZERO_RESERVE=1`**) |
| **HTTP 530 / Cloudflare 1033** | Public RPC is fronted by a **Cloudflare Tunnel**. **530** + `tunnel_error` means Cloudflare cannot reach the tunnel connector — **infra**: restart/repair the tunnel on the host that runs `boing-node`, not an app bug. The check script **retries** 520–530 and honors **`retry_after`** from the error JSON when present. |
| **Tunnel** | HTTP **530** / Cloudflare **1033** → origin or `cloudflared`, not clients ([RUNBOOK.md](RUNBOOK.md) §8.3) |
| **boing.finance + Boing Express (manual)** | Connect Express on **6913**; exercise **Swap** native panel (if pool configured), **Deploy Token** native section, **Create Pool** toasts, and chain switch (e.g. Sepolia) per [NATIVE-AMM-E2E-SMOKE.md](NATIVE-AMM-E2E-SMOKE.md) |
| **Playwright (optional CI)** | [PLAYWRIGHT-E2E-CI-OPS.md](PLAYWRIGHT-E2E-CI-OPS.md), [examples/native-boing-playwright/README.md](../examples/native-boing-playwright/README.md); pool-aware builds may set `REACT_APP_BOING_NATIVE_AMM_POOL` (see boing.finance **`build-with-test-pool.mjs`** if present) |

---

## 5. Line endings (Windows)

If shared **`.js`** files show spurious git diffs on Windows, the repo root **`.gitattributes`** enforces **`eol=lf`** for `*.js` / `*.mjs` / `*.cjs`. Re-clone or `git add --renormalize .` once if your tree was checked out with CRLF.

---

*Boing Network — Authentic. Decentralized. Optimal. Sustainable.*
