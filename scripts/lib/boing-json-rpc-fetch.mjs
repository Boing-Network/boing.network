/**
 * Minimal JSON-RPC helpers for repo-root operator scripts (no boing-sdk).
 * @param {string} base  Base URL without trailing slash
 */
export async function boingRpc(base, method, params) {
  const res = await fetch(base, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
  });
  const text = await res.text();
  let j;
  try {
    j = JSON.parse(text);
  } catch {
    throw new Error(`Non-JSON from ${base}: HTTP ${res.status} ${text.slice(0, 200)}`);
  }
  if (j.error) {
    const err = new Error(j.error.message ?? 'JSON-RPC error');
    err.code = j.error.code;
    throw err;
  }
  return j.result;
}

/** @param {string} base */
export async function chainHeight(base) {
  const h = await boingRpc(base, 'boing_chainHeight', []);
  if (typeof h !== 'number' || !Number.isFinite(h) || h < 0) {
    throw new Error(`Unexpected boing_chainHeight result: ${JSON.stringify(h)}`);
  }
  return h;
}

/** @param {string} base */
export async function networkInfo(base) {
  try {
    return await boingRpc(base, 'boing_getNetworkInfo', []);
  } catch {
    return null;
  }
}

/** @param {string} base */
export async function clientVersion(base) {
  try {
    const v = await boingRpc(base, 'boing_clientVersion', []);
    return typeof v === 'string' ? v : null;
  } catch {
    return null;
  }
}

/** @param {string} base */
export async function syncState(base) {
  try {
    return await boingRpc(base, 'boing_getSyncState', []);
  } catch {
    return null;
  }
}

/**
 * Parse `/ip4/a.b.c.d/tcp/port` multiaddrs from a comma-separated CLI string.
 * @param {string} raw
 * @returns {{ host: string, port: number }[]}
 */
export function parseIp4TcpMultiaddrs(raw) {
  const out = [];
  for (const seg of raw.split(',')) {
    const m = seg.trim().match(/^\/ip4\/(\d{1,3}(?:\.\d{1,3}){3})\/tcp\/(\d{1,5})$/);
    if (m) out.push({ host: m[1], port: Number(m[2]) });
  }
  return out;
}
