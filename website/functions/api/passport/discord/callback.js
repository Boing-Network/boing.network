/**
 * GET /api/passport/discord/callback?code=&state=
 * Discord OAuth2 redirect target. Validates the single-use state (D1 + browser cookie), exchanges the code,
 * reads the Discord user (scope identify), stores the verified link, and redirects back to the Passport page.
 * Discord access tokens are used once and never stored.
 */
import {
  DISCORD_API, STATE_COOKIE, discordOAuthConfigured, redirectUri, readCookie, stateCookie, markStale,
} from '../../portal/_lib/discordOAuth.js';

function back(request, account, result) {
  const u = new URL('/testnet/passport/', new URL(request.url).origin);
  if (account) u.searchParams.set('account', account);
  u.searchParams.set('discord', result);
  return new Response(null, { status: 302, headers: { Location: u.toString(), 'Set-Cookie': stateCookie('', 0), 'Cache-Control': 'no-store' } });
}

export async function onRequestGet({ env, request }) {
  if (!discordOAuthConfigured(env) || !env.DB) return back(request, null, 'disabled');
  const url = new URL(request.url);
  const state = url.searchParams.get('state') || '';
  const code = url.searchParams.get('code') || '';
  if (!/^[0-9a-f]{64}$/.test(state)) return back(request, null, 'invalid_state');
  if (readCookie(request, STATE_COOKIE) !== state) return back(request, null, 'state_mismatch');

  const nowIso = new Date().toISOString();
  const row = await env.DB.prepare('SELECT account_id_hex, expires_at, used_at FROM discord_oauth_states WHERE state = ?').bind(state).first();
  if (!row) return back(request, null, 'invalid_state');
  const used = await env.DB.prepare('UPDATE discord_oauth_states SET used_at = ? WHERE state = ? AND used_at IS NULL AND expires_at > ?')
    .bind(nowIso, state, nowIso).run();
  if (!used.meta || used.meta.changes !== 1) return back(request, row.account_id_hex, 'expired');
  const account = row.account_id_hex;
  if (url.searchParams.get('error') || !code) return back(request, account, 'cancelled');

  const tokenRes = await fetch(`${DISCORD_API}/oauth2/token`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: new URLSearchParams({
      grant_type: 'authorization_code', code, redirect_uri: redirectUri(env, request),
      client_id: env.DISCORD_CLIENT_ID, client_secret: env.DISCORD_CLIENT_SECRET,
    }),
  });
  if (!tokenRes.ok) return back(request, account, 'token_error');
  const tok = await tokenRes.json();
  const meRes = await fetch(`${DISCORD_API}/users/@me`, { headers: { Authorization: `Bearer ${tok.access_token}` } });
  if (!meRes.ok) return back(request, account, 'user_error');
  const me = await meRes.json();
  if (!/^\d{17,20}$/.test(String(me.id || ''))) return back(request, account, 'user_error');
  // Best-effort revoke: we only needed the identity once.
  try {
    await fetch(`${DISCORD_API}/oauth2/token/revoke`, {
      method: 'POST', headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
      body: new URLSearchParams({ token: tok.access_token, client_id: env.DISCORD_CLIENT_ID, client_secret: env.DISCORD_CLIENT_SECRET }),
    });
  } catch (_) {}

  const other = await env.DB.prepare('SELECT account_id_hex FROM discord_links WHERE discord_id = ?').bind(me.id).first();
  if (other && other.account_id_hex !== account) return back(request, account, 'already_linked_elsewhere');

  const prev = await env.DB.prepare('SELECT discord_id, roles_synced FROM discord_links WHERE account_id_hex = ?').bind(account).first();
  const stmts = [];
  if (prev && prev.discord_id !== me.id && prev.roles_synced && prev.roles_synced !== '[]') {
    // Re-linking a different Discord account: queue removal of roles given to the old one.
    stmts.push(env.DB.prepare('INSERT OR REPLACE INTO discord_role_removals (discord_id, role_ids, queued_at) VALUES (?, ?, ?)')
      .bind(prev.discord_id, prev.roles_synced, nowIso));
  }
  const username = String(me.username || '').slice(0, 64);
  stmts.push(env.DB.prepare(
    `INSERT INTO discord_links (account_id_hex, discord_id, discord_username, linked_at, in_guild, roles_synced, last_synced_at, sync_error)
     VALUES (?, ?, ?, ?, NULL, '[]', NULL, NULL)
     ON CONFLICT(account_id_hex) DO UPDATE SET discord_id=excluded.discord_id, discord_username=excluded.discord_username, linked_at=excluded.linked_at,
       in_guild = CASE WHEN discord_links.discord_id = excluded.discord_id THEN discord_links.in_guild ELSE NULL END,
       roles_synced = CASE WHEN discord_links.discord_id = excluded.discord_id THEN discord_links.roles_synced ELSE '[]' END,
       last_synced_at = NULL, sync_error = NULL`,
  ).bind(account, me.id, username, nowIso));
  stmts.push(markStale(env.DB, account));
  await env.DB.batch(stmts);
  return back(request, account, 'linked');
}
