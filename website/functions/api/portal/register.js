/**
 * POST /api/portal/register
 * Register or update portal registration (developer | user | node_operator).
 * Body: { account_id_hex, role, password?, email?, discord_handle?, github_username?, node_multiaddr? }
 * password is required for wallet sign-in later; min 8 characters.
 */
import { hashPassword } from './auth/password.js';

export async function onRequestPost(context) {
  const { env, request } = context;
  if (!env.DB) {
    return Response.json({ ok: false, message: 'Database not configured' }, { status: 503 });
  }
  try {
    const body = await request.json();
    const account_id_hex = normalizeHex(body.account_id_hex);
    const role = body.role;
    if (!account_id_hex || account_id_hex.length !== 66) {
      return Response.json({ ok: false, message: 'Invalid account_id_hex (must be 32-byte hex with 0x)' }, { status: 400 });
    }
    const validRoles = ['developer', 'user', 'node_operator'];
    if (!role || !validRoles.includes(role)) {
      return Response.json({ ok: false, message: 'Invalid role (developer | user | node_operator)' }, { status: 400 });
    }
    const password = typeof body.password === 'string' ? body.password.trim() : '';
    if (password.length > 0 && password.length < 8) {
      return Response.json({ ok: false, message: 'Portal password must be at least 8 characters' }, { status: 400 });
    }
    let password_salt = null;
    let password_hash = null;
    if (password.length >= 8) {
      const cred = hashPassword(password);
      password_salt = cred.saltHex;
      password_hash = cred.hashHex;
    }
    const email = body.email?.trim() || null;
    const discord_handle = body.discord_handle?.trim() || null;
    const github_username = body.github_username?.trim() || null;
    const node_multiaddr = body.node_multiaddr?.trim() || null;
    const now = new Date().toISOString();

    await env.DB.prepare(
      `INSERT INTO portal_registrations (account_id_hex, role, email, discord_handle, github_username, node_multiaddr, password_salt, password_hash, created_at, updated_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
       ON CONFLICT(account_id_hex) DO UPDATE SET
         role = excluded.role,
         email = excluded.email,
         discord_handle = excluded.discord_handle,
         github_username = excluded.github_username,
         node_multiaddr = excluded.node_multiaddr,
         password_salt = COALESCE(excluded.password_salt, password_salt),
         password_hash = COALESCE(excluded.password_hash, password_hash),
         updated_at = excluded.updated_at`
    )
      .bind(account_id_hex, role, email, discord_handle, github_username, node_multiaddr, password_salt, password_hash, now, now)
      .run();

    return Response.json({ ok: true, message: 'Registered' });
  } catch (e) {
    return Response.json({ ok: false, message: e.message || 'Server error' }, { status: 500 });
  }
}

function normalizeHex(s) {
  if (!s || typeof s !== 'string') return '';
  const t = s.trim().toLowerCase();
  return t.startsWith('0x') ? t : '0x' + t;
}
