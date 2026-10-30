/**
 * Simple D1-backed sliding-window rate limit for portal APIs (issue #61 item 68).
 *
 * key: typically `${route}:${ip}`
 * Returns { ok: true } or { ok: false, retryAfterSecs }.
 */

/**
 * @param {D1Database} db
 * @param {string} key
 * @param {number} limit
 * @param {number} windowSecs
 */
export async function checkRateLimit(db, key, limit, windowSecs) {
  if (!db || !key || limit <= 0 || windowSecs <= 0) {
    return { ok: true };
  }
  const now = Date.now();
  const windowStart = new Date(now - windowSecs * 1000).toISOString();
  const nowIso = new Date(now).toISOString();

  // Best-effort prune of expired rows for this key (keep table small).
  try {
    await db
      .prepare('DELETE FROM portal_rate_limits WHERE bucket_key = ? AND created_at < ?')
      .bind(key, windowStart)
      .run();
  } catch {
    // Table may not exist yet on older D1 — fail open for availability.
    return { ok: true };
  }

  let count = 0;
  try {
    const row = await db
      .prepare(
        'SELECT COUNT(*) AS c FROM portal_rate_limits WHERE bucket_key = ? AND created_at >= ?',
      )
      .bind(key, windowStart)
      .first();
    count = Number(row?.c || 0);
  } catch {
    return { ok: true };
  }

  if (count >= limit) {
    return { ok: false, retryAfterSecs: windowSecs };
  }

  try {
    await db
      .prepare('INSERT INTO portal_rate_limits (bucket_key, created_at) VALUES (?, ?)')
      .bind(key, nowIso)
      .run();
  } catch {
    return { ok: true };
  }
  return { ok: true };
}

/** Client IP from Cloudflare / forwarded headers (best effort). */
export function clientIp(request) {
  const cf = request.headers.get('cf-connecting-ip');
  if (cf && cf.trim()) return cf.trim();
  const xff = request.headers.get('x-forwarded-for');
  if (xff) {
    const first = xff.split(',')[0]?.trim();
    if (first) return first;
  }
  return 'unknown';
}

export function rateLimitedResponse(retryAfterSecs) {
  return Response.json(
    {
      ok: false,
      message: 'Too many requests. Please try again later.',
      error_code: 'rate_limited',
      retry_after_secs: retryAfterSecs,
    },
    {
      status: 429,
      headers: { 'Retry-After': String(retryAfterSecs || 60) },
    },
  );
}
