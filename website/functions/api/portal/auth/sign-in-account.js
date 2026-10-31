/**
 * POST /api/portal/auth/sign-in-account
 * Sign in with account ID + portal password (no wallet). Verifies password and returns same payload as wallet sign-in.
 * Body: { account_id_hex, password }
 */
import { verifyPassword } from './password.js';
import { checkRateLimit, clientIp, rateLimitedResponse } from '../_lib/rateLimit.js';

export async function onRequestPost(context) {
  const { env, request } = context;
  if (!env.DB) {
    return Response.json({ ok: false, message: 'Database not configured' }, { status: 503 });
  }

  const rl = await checkRateLimit(env.DB, `sign-in-account:${clientIp(request)}`, 10, 60);
  if (!rl.ok) return rateLimitedResponse(rl.retryAfterSecs);

  try {
    const body = await request.json();
    const account_id_hex = normalizeHex(body.account_id_hex);
    const password = typeof body.password === 'string' ? body.password : '';

    if (!account_id_hex || account_id_hex.length !== 66) {
      return Response.json({ ok: false, message: 'Invalid account_id_hex (must be 32-byte hex with 0x)' }, { status: 400 });
    }
    if (!password) {
      return Response.json({ ok: false, message: 'Password required for account sign-in' }, { status: 400 });
    }

    const row = await env.DB.prepare(
      'SELECT account_id_hex, role, email, discord_handle, github_username, node_multiaddr, password_salt, password_hash, created_at FROM portal_registrations WHERE account_id_hex = ?'
    )
      .bind(account_id_hex)
      .first();

    if (!row) {
      return Response.json({ ok: false, message: 'Account not registered' }, { status: 403 });
    }

    if (row.password_hash == null || row.password_hash === '') {
      return Response.json({
        ok: false,
        message: 'Set a portal password first so you can sign in with account ID.',
        need_password: true,
        account_id_hex: row.account_id_hex,
      }, { status: 403 });
    }

    if (!verifyPassword(password, row.password_salt || '', row.password_hash)) {
      return Response.json({ ok: false, message: 'Incorrect password' }, { status: 403 });
    }

    const result = {
      ok: true,
      registered: true,
      account_id_hex: row.account_id_hex,
      role: row.role,
      email: row.email,
      discord_handle: row.discord_handle,
      github_username: row.github_username,
      node_multiaddr: row.node_multiaddr,
      created_at: row.created_at,
    };

    if (row.role === 'developer') {
      const dapps = await env.DB.prepare(
        'SELECT contract_hex, name, registered_at FROM portal_dapps WHERE owner_account_hex = ? ORDER BY registered_at DESC'
      )
        .bind(account_id_hex)
        .all();
      result.dapps = dapps.results || [];
    }

    if (row.role === 'user') {
      const quests = await env.DB.prepare(
        'SELECT quest_id, submitted_at, verified_at FROM quest_completions WHERE account_id_hex = ? ORDER BY submitted_at DESC'
      )
        .bind(account_id_hex)
        .all();
      result.quest_completions = quests.results || [];
    }

    return Response.json(result);
  } catch (e) {
    return Response.json({ ok: false, message: 'Server error' }, { status: 500 });
  }
}

function normalizeHex(s) {
  if (!s || typeof s !== 'string') return '';
  const t = s.trim().toLowerCase();
  return t.startsWith('0x') ? t : '0x' + t;
}
