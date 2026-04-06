# Native Boing — Playwright smoke (Boing Express + boing.finance)

Optional **headed** automation for [NATIVE-AMM-E2E-SMOKE.md](../../docs/NATIVE-AMM-E2E-SMOKE.md): Chrome loads an **unpacked Boing Express** build, opens the swap page, waits for you to **unlock the wallet and connect** on **Boing testnet (6913)**, then asserts the native AMM panel and clicks **Refresh reserves**.

**CI / ops:** Default GitHub Actions validates install + **extension** project tests (**skipped** without **`BOING_EXPRESS_EXTENSION_PATH`**). Headed extension E2E on shared runners is not supported without a display — see [PLAYWRIGHT-E2E-CI-OPS.md](../../docs/PLAYWRIGHT-E2E-CI-OPS.md) and [TESTNET-OPS-RUNBOOK.md](../../docs/TESTNET-OPS-RUNBOOK.md) §4.

A separate **headless** **public** project runs **`public-swap-page-smoke`** (network URL load) plus **`tutorial-readme-wiring`** (offline README string checks for LP vault / LP share scripts). Run locally with **`npm run test:e2e:public-smoke`**. A **weekly + manual** workflow runs that project — see **§ D** in [PLAYWRIGHT-E2E-CI-OPS.md](../../docs/PLAYWRIGHT-E2E-CI-OPS.md).

This complements the Rust integration test `native_amm_rpc_happy_path` (RPC only, no browser).

## Prerequisites

1. **Node 18+** and npm.
2. **Playwright Chromium** — after `npm install`, run **`npm run install:browsers`** (or `npx playwright install chromium`).
3. **Unpacked Boing Express** directory (contains `manifest.json`). Find Chrome’s unpacked or packed extension folder, or extract a build zip to e.g. `~/boing-express-unpacked`.
4. **Pool + RPC:** Same as [NATIVE-AMM-E2E-SMOKE.md](../../docs/NATIVE-AMM-E2E-SMOKE.md) — **boing.finance** must be built with the canonical testnet pool (`boingCanonicalTestnetPool.js` / **`REACT_APP_BOING_NATIVE_AMM_POOL`** if used). Public testnet pool id: [RPC-API-SPEC.md](../../docs/RPC-API-SPEC.md) § Native AMM; repo health check: **`npm run check-canonical-pool`** (root).

## Setup

```bash
cd examples/native-boing-playwright
npm install
npx playwright install chrome
```

## Run

```bash
export BOING_EXPRESS_EXTENSION_PATH="/absolute/path/to/boing-express-unpacked"
npm run test:e2e
# Headless public URL smoke (no extension; needs network):
# npm run test:e2e:public-smoke
```

A Chromium window opens with the extension loaded. **Unlock Boing Express** and **connect** to the site on **testnet 6913** before the **panel timeout** (default **120s**). The test then expects `[data-testid="native-amm-panel"]` and clicks **Refresh reserves**.

### Environment

| Variable | Required | Description |
|----------|----------|-------------|
| `BOING_EXPRESS_EXTENSION_PATH` | **yes** | Absolute path to unpacked extension directory |
| `BOING_E2E_SWAP_URL` | no | Default `https://boing.finance/swap` |
| `BOING_E2E_PANEL_TIMEOUT_MS` | no | Default `120000` — wait for native AMM panel after load |
| `BOING_E2E_PAUSE` | no | Set `1` to call `page.pause()` after navigation (Playwright inspector) |
| `BOING_E2E_REQUIRE_NATIVE_PANEL` | no | Set `1` with **`test:e2e:public-smoke`** to assert **`native-amm-panel`** visibility (stricter) |

See [env.e2e.example](./env.e2e.example).

### Without `BOING_EXPRESS_EXTENSION_PATH`

**`npm run test:e2e`** (extension project only) **skips** the headed suite and prints the reason — safe for `npm test` style invocations from a parent repo. Use **`npm run test:e2e:public-smoke`** for headless URL checks without an extension.

### Debug

```bash
npm run test:e2e:debug
```

## Monorepo root

From repo root (after `npm install` in this folder):

```bash
npm run test:e2e --prefix examples/native-boing-playwright
```

(Still requires `BOING_EXPRESS_EXTENSION_PATH` and Playwright Chromium.)
