#!/usr/bin/env node
/**
 * Verify public (or local) RPC can read the canonical native CP pool's reserve A slot.
 * No boing-sdk build — raw JSON-RPC only.
 *
 *   npm run check-canonical-pool
 *   BOING_RPC_URL=https://testnet-rpc.boing.network/ npm run check-canonical-pool
 *   BOING_POOL_HEX=0x... BOING_RPC_URL=... node scripts/check-canonical-native-amm-pool.mjs
 *
 * Exit 0: boing_getContractStorage returned a 32-byte word for reserve A key.
 * Exit 1: RPC error, wrong shape, or missing method.
 *
 * Optional strict mode (CI):
 *   BOING_REQUIRE_NONZERO_RESERVE=1  — exit 1 if reserve A word is all zeros (wrong chain, undeployed pool, or drained).
 *
 * Retries (transient HTML/WAF/5xx): BOING_RPC_RETRIES (default 3), BOING_RPC_RETRY_MS (default 2000, backoff × attempt).
 *
 * Note: Without strict mode, all-zero reserve still exits 0 — RPC round-trip succeeded.
 * Compare with docs: docs/RPC-API-SPEC.md (Native constant-product AMM).
 */
const requireNonzeroReserve =
  process.env.BOING_REQUIRE_NONZERO_RESERVE === '1' ||
  process.env.BOING_REQUIRE_NONZERO_RESERVE === 'true';
const DEFAULT_POOL =
  '0xffaa1290614441902ba813bf3bd8bf057624e0bd4f16160a9d32cd65d3f4d0c2';
/** `native_amm::reserve_a_key` — k[31] = 0x01 */
const RESERVE_A_KEY = `0x${'00'.repeat(31)}01`;

const pool = (process.env.BOING_POOL_HEX ?? DEFAULT_POOL).trim();
const base = (process.env.BOING_RPC_URL ?? 'https://testnet-rpc.boing.network/').replace(/\/$/, '');
const rpcRetries = Math.max(
  1,
  Math.min(8, parseInt(process.env.BOING_RPC_RETRIES ?? '3', 10) || 3),
);
const rpcRetryMs = Math.max(
  200,
  Math.min(30_000, parseInt(process.env.BOING_RPC_RETRY_MS ?? '2000', 10) || 2000),
);

const UA = 'boing.network-check-canonical-pool/1.0 (CI JSON-RPC probe)';

function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

function bodySnippet(text, max = 400) {
  const t = text.replace(/\s+/g, ' ').trim();
  return t.length <= max ? t : `${t.slice(0, max)}…`;
}

/**
 * @returns {Promise<{ ok: true, result: unknown } | { ok: false, error: Record<string, unknown> }>}
 */
async function rpcOnce(method, params) {
  let res;
  try {
    res = await fetch(base, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Accept: 'application/json',
        'User-Agent': UA,
      },
      body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
    });
  } catch (e) {
    return {
      ok: false,
      error: {
        message: 'fetch failed',
        detail: String(e?.message ?? e),
        rpc: base,
        method,
      },
    };
  }

  const ct = res.headers.get('content-type') ?? '';
  const text = await res.text();
  let j;
  try {
    j = JSON.parse(text);
  } catch (parseErr) {
    const looksHtml = /^\s*</.test(text);
    return {
      ok: false,
      error: {
        message: looksHtml
          ? 'RPC returned HTML, not JSON (wrong URL, CDN/WAF error page, outage, or blocked client)'
          : 'RPC response is not valid JSON',
        httpStatus: res.status,
        httpStatusText: res.statusText,
        contentType: ct,
        finalUrl: res.url,
        rpc: base,
        method,
        bodySnippet: bodySnippet(text),
        parseError: String(parseErr?.message ?? parseErr),
      },
    };
  }

  if (!res.ok) {
    return {
      ok: false,
      error: {
        message: `HTTP ${res.status} ${res.statusText}`,
        httpStatus: res.status,
        contentType: ct,
        finalUrl: res.url,
        rpc: base,
        method,
        jsonrpc: j,
      },
    };
  }

  if (j && typeof j === 'object' && j.error) {
    return { ok: false, error: j.error };
  }
  return { ok: true, result: j.result };
}

async function rpc(method, params) {
  let last = /** @type {{ ok: false, error: Record<string, unknown> }} */ ({
    ok: false,
    error: { message: 'no attempts' },
  });
  for (let attempt = 1; attempt <= rpcRetries; attempt += 1) {
    last = await rpcOnce(method, params);
    if (last.ok) return last;
    const retryable =
      last.error?.httpStatus === 429 ||
      last.error?.httpStatus === 502 ||
      last.error?.httpStatus === 503 ||
      last.error?.httpStatus === 504 ||
      (typeof last.error?.message === 'string' &&
        last.error.message.includes('HTML, not JSON')) ||
      last.error?.message === 'fetch failed';
    if (!retryable || attempt === rpcRetries) break;
    await sleep(rpcRetryMs * attempt);
  }
  return last;
}

function isZeroWord(hex) {
  const h = hex.replace(/^0x/i, '').toLowerCase();
  return /^0+$/.test(h);
}

async function main() {
  const height = await rpc('boing_chainHeight', []);
  if (!height.ok) {
    console.error(
      JSON.stringify(
        { ok: false, phase: 'chainHeight', rpc: base, error: height.error },
        null,
        2,
      ),
    );
    process.exit(1);
  }

  const storage = await rpc('boing_getContractStorage', [pool, RESERVE_A_KEY]);
  if (!storage.ok) {
    console.error(
      JSON.stringify(
        {
          ok: false,
          phase: 'boing_getContractStorage',
          rpc: base,
          pool,
          key: RESERVE_A_KEY,
          error: storage.error,
        },
        null,
        2,
      ),
    );
    process.exit(1);
  }

  const value = storage.result?.value;
  if (typeof value !== 'string' || !/^0x[0-9a-fA-F]{64}$/.test(value)) {
    console.error(
      JSON.stringify(
        {
          ok: false,
          phase: 'parse',
          rpc: base,
          pool,
          message: 'expected result.value as 0x + 64 hex',
          raw: storage.result,
        },
        null,
        2,
      ),
    );
    process.exit(1);
  }

  const reserveZero = isZeroWord(value);
  const out = {
    ok: true,
    rpc: base,
    pool,
    chainHeight: height.result,
    reserveA_storage_word: value,
    reserveA_all_zero: reserveZero,
    requireNonzeroReserve,
    hint: reserveZero
      ? 'Reserve A is zero — pool may have no liquidity on this chain, or this RPC is not the same network as the canonical deploy.'
      : 'Reserve A non-zero — pool contract readable on this RPC.',
  };

  if (requireNonzeroReserve && reserveZero) {
    console.error(
      JSON.stringify(
        {
          ok: false,
          error: 'BOING_REQUIRE_NONZERO_RESERVE: reserve A is zero',
          rpc: base,
          pool,
          chainHeight: height.result,
          reserveA_storage_word: value,
        },
        null,
        2,
      ),
    );
    process.exit(1);
  }

  console.log(JSON.stringify(out, null, 2));
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
