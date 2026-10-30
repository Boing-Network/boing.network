/**
 * POST /api/portal/register
 * Register a new portal account (developer | user | node_operator), or update an
 * existing one only with proof of ownership (portal password or wallet signature).
 * Body: { account_id_hex, role, password?, email?, discord_handle?, github_username?, node_multiaddr?,
 *         message?, signature? }
 * password is required for wallet sign-in later; min 8 characters (new registrations).
 * Updates of an existing account require `password` (matching stored) or Authorize signature (action: portal_update).
 */
import { hashPassword } from './auth/password.js';
import { isAccountHex, normalizeHex } from './_lib/hex.js';
import { verifyAccountOwnership } from './_lib/ownership.js';

export async function onRequestPost(context) {
  const { env, request } = context;
  if (!env.DB) {
    return Response.json({ ok: false, message: 'Database not configured' }, { status: 503 });
  }
  try {
    const body = await request.json();
    const account_id_hex = normalizeHex(body.account_id_hex);
    const role = body.role;
    if (!isAccountHex(account_id_hex)) {
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

    const email = sanitizeOptionalText(body.email, 254);
    const discord_handle = sanitizeOptionalText(body.discord_handle, 64);
    const github_username = sanitizeOptionalText(body.github_username, 64);
    const node_multiaddr = sanitizeOptionalText(body.node_multiaddr, 256);
    const now = new Date().toISOString();

    const existing = await env.DB.prepare(
      'SELECT account_id_hex, role, password_salt, password_hash FROM portal_registrations WHERE account_id_hex = ?'
    )
      .bind(account_id_hex)
      .first();

    if (existing) {
      // Do not allow unauthenticated overwrite of another account's details (audit #61 item 63).
      const auth = await verifyAccountOwnership(env, account_id_hex, body, 'portal_update', existing);
      if (!auth.ok) {
        const status = auth.error_code === 'auth_required' ? 409 : auth.status || 401;
        const message =
          auth.error_code === 'auth_required'
            ? 'Account already registered. Sign in and prove ownership to update your profile.'
            : auth.message;
        return Response.json({ ok: false, message, error_code: auth.error_code || 'conflict' }, { status });
      }

      let password_salt = existing.password_salt;
      let password_hash = existing.password_hash;
      if (password.length >= 8) {
        const cred = hashPassword(password);
        password_salt = cred.saltHex;
        password_hash = cred.hashHex;
      }

      await env.DB.prepare(
        `UPDATE portal_registrations SET
           role = ?,
           email = ?,
           discord_handle = ?,
           github_username = ?,
           node_multiaddr = ?,
           password_salt = ?,
           password_hash = ?,
           updated_at = ?
         WHERE account_id_hex = ?`
      )
        .bind(
          role,
          email,
          discord_handle,
          github_username,
          node_multiaddr,
          password_salt,
          password_hash,
          now,
          account_id_hex
        )
        .run();

      return Response.json({ ok: true, message: 'Updated', updated: true });
    }

    let password_salt = null;
    let password_hash = null;
    if (password.length >= 8) {
      const cred = hashPassword(password);
      password_salt = cred.saltHex;
      password_hash = cred.hashHex;
    }

    try {
      await env.DB.prepare(
        `INSERT INTO portal_registrations (account_id_hex, role, email, discord_handle, github_username, node_multiaddr, password_salt, password_hash, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`
      )
        .bind(account_id_hex, role, email, discord_handle, github_username, node_multiaddr, password_salt, password_hash, now, now)
        .run();
    } catch (insertErr) {
      const msg = String(insertErr && insertErr.message ? insertErr.message : insertErr);
      if (/UNIQUE|constraint/i.test(msg)) {
        return Response.json(
          {
            ok: false,
            message: 'Account already registered. Sign in and prove ownership to update your profile.',
            error_code: 'conflict',
          },
          { status: 409 }
        );
      }
      throw insertErr;
    }

    return Response.json({ ok: true, message: 'Registered' });
  } catch (e) {
    return Response.json({ ok: false, message: e.message || 'Server error' }, { status: 500 });
  }
}

function sanitizeOptionalText(value, maxLen) {
  if (value == null) return null;
  if (typeof value !== 'string') return null;
  const t = value.trim();
  if (!t) return null;
  // Strip angle brackets to reduce stored XSS risk if values are ever rendered unsafely.
  return t.replace(/[<>]/g, '').slice(0, maxLen) || null;
}
