/**
 * POST /api/passport/discord/unlink
 * Body: { account_id_hex, password } or signed Authorize message with Action: discord_unlink.
 * Deletes the link, queues removal of any tier roles the sync bot assigned, and marks the account for re-scoring.
 */
import { normalizeHex, isAccountHex } from '../../portal/_lib/hex.js';
import { verifyAccountOwnership } from '../../portal/_lib/ownership.js';
import { checkRateLimit, clientIp, rateLimitedResponse } from '../../portal/_lib/rateLimit.js';
import { markStale } from '../../portal/_lib/discordOAuth.js';

export async function onRequestPost({ env, request }) {
  if (!env.DB) return Response.json({ ok: false, message: 'Database not configured' }, { status: 503 });
  const rl = await checkRateLimit(env.DB, `discord-unlink:${clientIp(request)}`, 10, 300);
  if (!rl.ok) return rateLimitedResponse(rl.retryAfterSecs);
  let body;
  try { body = await request.json(); } catch { return Response.json({ ok: false, message: 'Invalid JSON' }, { status: 400 }); }
  const account = normalizeHex(body.account_id_hex || '');
  if (!isAccountHex(account)) return Response.json({ ok: false, message: 'Invalid account_id_hex' }, { status: 400 });
  const reg = await env.DB.prepare('SELECT account_id_hex, password_salt, password_hash FROM portal_registrations WHERE account_id_hex = ?').bind(account).first();
  if (!reg) return Response.json({ ok: false, message: 'Not registered' }, { status: 404 });
  const auth = await verifyAccountOwnership(env, account, body, 'discord_unlink', reg);
  if (!auth.ok) return Response.json({ ok: false, message: auth.message, error_code: auth.error_code }, { status: auth.status || 401 });

  const link = await env.DB.prepare('SELECT discord_id, roles_synced FROM discord_links WHERE account_id_hex = ?').bind(account).first();
  if (!link) return Response.json({ ok: true, linked: false });
  const now = new Date().toISOString();
  const stmts = [env.DB.prepare('DELETE FROM discord_links WHERE account_id_hex = ?').bind(account), markStale(env.DB, account)];
  if (link.roles_synced && link.roles_synced !== '[]') {
    stmts.push(env.DB.prepare('INSERT OR REPLACE INTO discord_role_removals (discord_id, role_ids, queued_at) VALUES (?, ?, ?)')
      .bind(link.discord_id, link.roles_synced, now));
  }
  await env.DB.batch(stmts);
  return Response.json({ ok: true, linked: false });
}
