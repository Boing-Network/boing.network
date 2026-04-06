# Playwright native AMM E2E — CI and operations

The package **`examples/native-boing-playwright`** runs **headed** Chromium with an **unpacked Boing Express** extension ([README](../examples/native-boing-playwright/README.md)). Tests call **`chromium.launchPersistentContext`** with **`headless: false`** and expect an operator to **unlock** the wallet within **`BOING_E2E_PANEL_TIMEOUT_MS`** (default **120s**).

---

## Why default GitHub Actions does not run the real E2E

On **`ubuntu-latest`**:

- There is no interactive display suitable for “unlock extension” flows.
- Chrome extension loading in **fully automated** headless mode is not what this harness targets.

The workflow **`.github/workflows/native-boing-playwright.yml`** therefore **installs** dependencies and runs **`npm run test:e2e`**, which **exits successfully** when **`BOING_EXPRESS_EXTENSION_PATH`** is unset (tests **skip**). This validates the package and Playwright install **without** a browser secret.

---

## Options for teams that want CI-style coverage

### A. Self-hosted runner with a display

1. Register a **self-hosted** GitHub Actions runner on a machine (or VM) where you can run headed Chromium.
2. Pre-install or sync an **unpacked Boing Express** build to a known path on that machine.
3. Add a **private** workflow (in your org repo) that sets:

   ```yaml
   env:
     BOING_EXPRESS_EXTENSION_PATH: /opt/boing-express-unpacked
   ```

   and runs **`npm run test:e2e`** in **`examples/native-boing-playwright`**. A human may still need to **unlock** once per run unless you automate storage state (out of scope for this repo’s public harness).

### B. Zip artifact + download (advanced)

If you publish a **zip** of the unpacked extension to a **private** URL:

- Repository secrets (names are conventional — set in **Settings → Secrets**):

  | Secret | Purpose |
  |--------|---------|
  | **`BOING_EXPRESS_EXTENSION_ZIP_URL`** | HTTPS URL to a zip file that contains **`manifest.json`** somewhere under the archive root |
  | **`BOING_EXPRESS_EXTENSION_ZIP_TOKEN`** | Optional `Authorization: Bearer …` for private object storage |

- Your workflow should **`unzip`**, **`find …/manifest.json`**, set **`BOING_EXPRESS_EXTENSION_PATH`** to that directory, then run tests.

You still need a **display** (e.g. **Xvfb**) and likely **manual unlock** unless you invest in persisted extension profiles — expect flakiness on unattended shared runners.

### C. Keep E2E manual

Recommended default for **public** repos: run Playwright **locally** or on an **ops workstation** per [NATIVE-AMM-E2E-SMOKE.md](NATIVE-AMM-E2E-SMOKE.md).

### D. Scheduled headless public URL smoke (no extension)

The package includes a second Playwright **project** **`public`**: [`public-swap-page-smoke.spec.ts`](../examples/native-boing-playwright/tests/public-swap-page-smoke.spec.ts) (**headless** Chromium loads **`BOING_E2E_SWAP_URL`** (default **`https://boing.finance/swap`**) and, if that response is not OK (common: **403** from CDN/WAF for datacenter or automation traffic), tries **`BOING_E2E_SWAP_FALLBACK_URLS`** then built-in **`https://boing.network/`** / **`https://www.boing.network/`**. It asserts a successful HTTP response and non-trivial document text. This does **not** unlock Boing Express or click **Refresh reserves**; it is a coarse uptime / deploy sanity check. The same **`public`** project runs [`tutorial-readme-wiring.spec.ts`](../examples/native-boing-playwright/tests/tutorial-readme-wiring.spec.ts) (no network): it reads **`examples/native-boing-tutorial/README.md`** and asserts LP vault / LP share **`npm run`** script names stay documented.

The **GitHub Actions** workflow sets **`BOING_E2E_SWAP_URL=https://boing.network/`** so CI does not depend on the finance app allowing GitHub runner IPs.

- **Local:** `npm run test:e2e:public-smoke` in **`examples/native-boing-playwright`** — the URL smoke test needs network; the tutorial README wiring test does not.
- **CI:** [`.github/workflows/native-boing-playwright-public-smoke.yml`](../.github/workflows/native-boing-playwright-public-smoke.yml) runs **`workflow_dispatch`** and **weekly** (Mondays 16:00 UTC). It is **not** wired into every PR, so a bad CDN hour does not block merges.
- **Stricter checks:** set **`BOING_E2E_REQUIRE_NATIVE_PANEL=1`** to enable the optional test that waits for **`[data-testid="native-amm-panel"]`** (use on a known-good environment; may fail if the panel is gated behind wallet connect).

Default PR workflow [`.github/workflows/native-boing-playwright.yml`](../.github/workflows/native-boing-playwright.yml) still runs **`npm run test:e2e`**, which is **`--project=extension`** only (skipped without **`BOING_EXPRESS_EXTENSION_PATH`**).

---

## Related

- [TESTNET-OPS-RUNBOOK.md](TESTNET-OPS-RUNBOOK.md) §4  
- [NATIVE-AMM-INTEGRATION-CHECKLIST.md](NATIVE-AMM-INTEGRATION-CHECKLIST.md) **A4.3**
