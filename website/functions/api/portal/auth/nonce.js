/**
 * GET /api/portal/auth/nonce?origin=https://boing.network
 * Issue a short-lived nonce for wallet sign-in so the signed message cannot be replayed.
 */
import { checkRateLimit, clientIp, rateLimitedResponse } from '../_lib/rateLimit.js';

export async function onRequestGet(context) {
  const { env, request } = context;
  if (!env.DB) {
    return Response.json({ ok: false, message: 'Database not configured' }, { status: 503 });
  }

  const rl = await checkRateLimit(env.DB, `nonce:${clientIp(request)}`, 30, 60);
  if (!rl.ok) return rateLimitedResponse(rl.retryAfterSecs);

  try {
    const url = new URL(request.url);
    const origin = normalizeOrigin(url.searchParams.get('origin') || '');
    if (!origin) {
      return Response.json({ ok: false, message: 'Missing or invalid origin' }, { status: 400 });
    }

    const nonce = crypto.randomUUID().replace(/-/g, '');
    const now = new Date();
    const expires = new Date(now.getTime() + 5 * 60 * 1000);

    await env.DB.prepare(
      'INSERT INTO portal_auth_nonces (nonce, origin, created_at, expires_at, used_at) VALUES (?, ?, ?, ?, NULL)',
    )
      .bind(nonce, origin, now.toISOString(), expires.toISOString())
      .run();

    // Opportunistic cleanup of expired / used nonces so the table cannot grow unbounded.
    try {
      await env.DB.prepare(
        "DELETE FROM portal_auth_nonces WHERE expires_at < ? OR (used_at IS NOT NULL AND used_at < ?)",
      )
        .bind(now.toISOString(), new Date(now.getTime() - 24 * 60 * 60 * 1000).toISOString())
        .run();
    } catch {
      /* ignore cleanup failures */
    }

    return Response.json({
      ok: true,
      nonce,
      origin,
      issued_at: now.toISOString(),
      expires_at: expires.toISOString(),
      message_template:
        'Sign in to Boing Portal\nOrigin: {origin}\nTimestamp: {timestamp}\nNonce: {nonce}',
    });
  } catch (e) {
    return Response.json({ ok: false, message: e.message || 'Server error' }, { status: 500 });
  }
}

function normalizeOrigin(value) {
  if (!value || typeof value !== 'string') return '';
  try {
    const url = new URL(value);
    if (url.protocol !== 'http:' && url.protocol !== 'https:') return '';
    return url.origin;
  } catch {
    return '';
  }
}
