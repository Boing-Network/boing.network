/** Normalize 32-byte account / token / hash hex to lowercase `0x` + 64 hex. */
export function normalizeHex64(raw: unknown): string | null {
  if (typeof raw !== 'string' || !raw.trim()) return null;
  const s = raw.trim();
  const hex = s.startsWith('0x') || s.startsWith('0X') ? s.slice(2) : s;
  if (!/^[0-9a-fA-F]{64}$/.test(hex)) return null;
  return `0x${hex.toLowerCase()}`;
}

export function zeros32(): string {
  return '0x' + '00'.repeat(32);
}

export function isZeroHex64(h: string | null | undefined): boolean {
  if (h == null) return true;
  const n = normalizeHex64(h);
  return n == null || n === zeros32();
}

export type HeadersJsonOpts = { cacheControl?: string };

export function parseCorsOrigins(raw: string | undefined): string[] | '*' {
  if (raw == null || !raw.trim() || raw.trim() === '*') return '*';
  const list = raw
    .split(',')
    .map((s) => s.trim())
    .filter(Boolean);
  return list.length ? list : '*';
}

export function headersJson(
  corsOrigins: string[] | '*',
  requestOrigin: string | null,
  opts?: HeadersJsonOpts
): Headers {
  const h = new Headers({
    'Content-Type': 'application/json; charset=utf-8',
    'X-Content-Type-Options': 'nosniff',
    'Cache-Control': opts?.cacheControl ?? 'no-store',
  });
  if (corsOrigins === '*') {
    h.set('Access-Control-Allow-Origin', '*');
  } else if (requestOrigin && corsOrigins.includes(requestOrigin)) {
    h.set('Access-Control-Allow-Origin', requestOrigin);
    h.set('Vary', 'Origin');
  } else if (corsOrigins.length === 1) {
    h.set('Access-Control-Allow-Origin', corsOrigins[0]!);
  } else {
    h.set('Access-Control-Allow-Origin', '*');
  }
  h.set('Access-Control-Allow-Methods', 'GET, HEAD, POST, OPTIONS');
  h.set('Access-Control-Allow-Headers', 'Content-Type, Authorization');
  return h;
}

export function handleOptions(corsOrigins: string[] | '*', requestOrigin: string | null): Response {
  return new Response(null, { status: 204, headers: headersJson(corsOrigins, requestOrigin) });
}

export function jsonRes(
  body: unknown,
  status: number,
  corsOrigins: string[] | '*',
  requestOrigin: string | null,
  opts?: HeadersJsonOpts
): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: headersJson(corsOrigins, requestOrigin, opts),
  });
}
