/**
 * POST /api/passport/discord/start
 * Body: { account_id_hex, password } or { account_id_hex, message, signature } (Authorize Boing Portal, Action: discord_link).
 * Proves control of the Boing account, then returns a Discord authorize URL whose state is bound to that account
 * (D1 row, 10 min, single use) and to this browser (HttpOnly cookie).
 */
import { normalizeHex, isAccountHex } from '../../portal/_lib/hex.js';
import { verifyAccountOwnership } from '../../portal/_lib/ownership.js';
import { checkRateLimit, clientIp, rateLimitedResponse } from '../../portal/_lib/rateLimit.js';
import { discordOAuthConfigured, authorizeUrl, randomState, stateCookie, STATE_TTL_MS } from '../../portal/_lib/discordOAuth.js';

export async function onRequestPost({ env, request }) {
  if (!discordOAuthConfigured(env)) {
    return Response.json({ ok: false, configured: false, message: 'Discord linking is not enabled yet.' }, { status: 503 });
  }
  if (!env.DB) return Response.json({ ok: false, message: 'Database not configured' }, { status: 503 });
  const rl = await checkRateLimit(env.DB, `discord-start:${clientIp(request)}`, 10, 300);
  if (!rl.ok) return rateLimitedResponse(rl.retryAfterSecs);

  let body;
  try { body = await request.json(); } catch { return Response.json({ ok: false, message: 'Invalid JSON' }, { status: 400 }); }
  const account = normalizeHex(body.account_id_hex || '');
  if (!isAccountHex(account)) return Response.json({ ok: false, message: 'Invalid account_id_hex' }, { status: 400 });

  const reg = await env.DB.prepare('SELECT account_id_hex, password_salt, password_hash FROM portal_registrations WHERE account_id_hex = ?').bind(account).first();
  if (!reg) return Response.json({ ok: false, message: 'Register on the testnet portal first.' }, { status: 404 });
  const auth = await verifyAccountOwnership(env, account, body, 'discord_link', reg);
  if (!auth.ok) return Response.json({ ok: false, message: auth.message, error_code: auth.error_code }, { status: auth.status || 401 });

  const now = Date.now();
  const state = randomState();
  await env.DB.batch([
    env.DB.prepare('DELETE FROM discord_oauth_states WHERE expires_at < ?').bind(new Date(now).toISOString()),
    env.DB.prepare('INSERT INTO discord_oauth_states (state, account_id_hex, created_at, expires_at) VALUES (?, ?, ?, ?)')
      .bind(state, account, new Date(now).toISOString(), new Date(now + STATE_TTL_MS).toISOString()),
  ]);
  return Response.json({ ok: true, url: authorizeUrl(env, request, state) }, {
    headers: { 'Set-Cookie': stateCookie(state, STATE_TTL_MS / 1000), 'Cache-Control': 'no-store' },
  });
}
