/**
 * Headless Chromium — no extension. Verifies a public Boing surface URL loads over the network.
 * Primary target is the swap app; datacenter / headless traffic often gets **403** from the finance
 * CDN, so we fall back to **boing.network** (see `buildPublicPageUrls`).
 * For full Boing Express + panel flows see native-amm-smoke.spec.ts.
 */
import { test, expect } from '@playwright/test';

/** Prefer **`BOING_E2E_SWAP_URL`**; default is the public swap app. */
const swapUrlPrimary =
  process.env.BOING_E2E_SWAP_URL?.trim() || 'https://boing.finance/swap';

const requireNativePanel =
  process.env.BOING_E2E_REQUIRE_NATIVE_PANEL === '1' ||
  process.env.BOING_E2E_REQUIRE_NATIVE_PANEL === 'true';

const DEFAULT_FALLBACK_URLS = ['https://boing.network/', 'https://www.boing.network/'];

function buildPublicPageUrls(): string[] {
  const fromEnv =
    process.env.BOING_E2E_SWAP_FALLBACK_URLS?.split(/[,;\s]+/).map((s) => s.trim()).filter(Boolean) ??
    [];
  const seen = new Set<string>();
  const out: string[] = [];
  for (const u of [swapUrlPrimary, ...fromEnv, ...DEFAULT_FALLBACK_URLS]) {
    if (!seen.has(u)) {
      seen.add(u);
      out.push(u);
    }
  }
  return out;
}

test.describe('Public swap page (headless)', () => {
  test('swap URL responds and renders document', async ({ page }) => {
    const urls = buildPublicPageUrls();
    let response = null;
    let lastStatus = 0;
    for (const url of urls) {
      response = await page.goto(url, {
        waitUntil: 'domcontentloaded',
        timeout: 60_000,
      });
      lastStatus = response?.status() ?? 0;
      if (response?.ok()) break;
    }
    expect(response, 'navigation should return a response').toBeTruthy();
    expect(
      response!.ok(),
      `expected HTTP success from one of [${urls.join(', ')}]; last status ${lastStatus}`
    ).toBe(true);

    await expect(page.locator('body')).toBeVisible();
    const bodyText = (await page.locator('body').innerText()).trim();
    expect(bodyText.length).toBeGreaterThan(50);
  });

  test('optional: native AMM panel test id when BOING_E2E_REQUIRE_NATIVE_PANEL=1', async ({
    page,
  }) => {
    test.skip(!requireNativePanel, 'set BOING_E2E_REQUIRE_NATIVE_PANEL=1 to enable');

    await page.goto(swapUrlPrimary, {
      waitUntil: 'domcontentloaded',
      timeout: 60_000,
    });
    const panel = page.getByTestId('native-amm-panel');
    await expect(panel).toBeVisible({ timeout: 30_000 });
  });
});
