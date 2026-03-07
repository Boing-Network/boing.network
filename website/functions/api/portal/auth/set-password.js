/**
 * POST /api/portal/auth/set-password
 * Set or update portal password for an account. Requires wallet signature proof.
 * Body: { account_id_hex, message, signature, new_password }
 * Message format: "Set portal password for Boing Portal\nOrigin: {origin}\nTimestamp: {timestamp}\nNonce: {nonce}"
 * Uses same nonce as sign-in (GET /api/portal/auth/nonce).
 */
import { createPublicKey, verify } from 'node:crypto';
import { hashPassword } from './password.js';

export async function onRequestPost(context) {
  const { env, request } = context;
  if (!env.DB) {
    return Response.json({ ok: false, message: 'Database not configured' }, { status: 503 });
  }

  try {
    const body = await request.json();
    const account_id_hex = normalizeHex(body.account_id_hex);
    const message = typeof body.message === 'string' ? body.message : '';
    const signatureHex = normalizeHex(body.signature || '');
    const newPassword = typeof body.new_password === 'string' ? body.new_password.trim() : '';

    if (!account_id_hex || account_id_hex.length !== 66) {
      return Response.json({ ok: false, message: 'Invalid account_id_hex' }, { status: 400 });
    }
    if (!message || !signatureHex) {
      return Response.json({ ok: false, message: 'Missing message or signature' }, { status: 400 });
    }
    if (newPassword.length < 8) {
      return Response.json({ ok: false, message: 'Password must be at least 8 characters' }, { status: 400 });
    }
    const sigHex = signatureHex.replace(/^0x/, '');
    if (sigHex.length !== 128 || !/^[0-9a-f]+$/.test(sigHex)) {
      return Response.json({ ok: false, message: 'Invalid signature' }, { status: 400 });
    }

    const publicKeyBytes = hexToBytes(account_id_hex);
    const signatureBytes = hexToBytes(signatureHex);
    if (!publicKeyBytes || !signatureBytes) {
      return Response.json({ ok: false, message: 'Invalid hex' }, { status: 400 });
    }

    const valid = verifyEd25519(publicKeyBytes, Buffer.from(message, 'utf8'), signatureBytes);
    if (!valid) {
      return Response.json({ ok: false, message: 'Invalid signature' }, { status: 401 });
    }

    const messageInfo = parseSetPasswordMessage(message);
    if (!messageInfo.origin || !messageInfo.timestamp || !messageInfo.nonce) {
      return Response.json({ ok: false, message: 'Invalid set-password message format' }, { status: 400 });
    }
    const messageError = validateMessageWindow(messageInfo);
    if (messageError) {
      return Response.json({ ok: false, message: messageError }, { status: 401 });
    }

    const nonceOk = await consumeNonce(env.DB, messageInfo.nonce, messageInfo.origin);
    if (!nonceOk.ok) {
      return Response.json({ ok: false, message: nonceOk.message }, { status: 401 });
    }

    const row = await env.DB.prepare(
      'SELECT account_id_hex FROM portal_registrations WHERE account_id_hex = ?'
    )
      .bind(account_id_hex)
      .first();
    if (!row) {
      return Response.json({ ok: false, message: 'Account not registered' }, { status: 403 });
    }

    const { saltHex, hashHex } = hashPassword(newPassword);
    await env.DB.prepare(
      'UPDATE portal_registrations SET password_salt = ?, password_hash = ?, updated_at = ? WHERE account_id_hex = ?'
    )
      .bind(saltHex, hashHex, new Date().toISOString(), account_id_hex)
      .run();

    return Response.json({ ok: true, message: 'Portal password set. You can now sign in with your wallet.' });
  } catch (e) {
    return Response.json({ ok: false, message: e.message || 'Server error' }, { status: 500 });
  }
}

function normalizeHex(s) {
  if (!s || typeof s !== 'string') return '';
  const t = s.trim().toLowerCase();
  return t.startsWith('0x') ? t : '0x' + t;
}

function hexToBytes(hexStr) {
  const h = hexStr.replace(/^0x/, '');
  if (!/^[0-9a-f]+$/.test(h) || h.length % 2 !== 0) return null;
  const buf = Buffer.alloc(h.length / 2);
  for (let i = 0; i < h.length; i += 2) {
    buf[i / 2] = parseInt(h.slice(i, i + 2), 16);
  }
  return buf;
}

function verifyEd25519(publicKeyBytes, messageBytes, signatureBytes) {
  if (publicKeyBytes.length !== 32 || signatureBytes.length !== 64) return false;
  try {
    const key = createPublicKey({
      key: publicKeyBytes,
      format: 'raw',
      type: 'ed25519',
    });
    return verify(null, messageBytes, key, signatureBytes);
  } catch {
    return false;
  }
}

function parseSetPasswordMessage(message) {
  const m = message.match(
    /^Set portal password for Boing Portal\s+Origin:\s*(.+)\s+Timestamp:\s*(\d{4}-\d{2}-\d{2}T[\d.:]+Z)\s+Nonce:\s*([a-zA-Z0-9_-]+)\s*$/m
  );
  if (m) {
    return {
      origin: normalizeOrigin(m[1]),
      timestamp: m[2],
      nonce: m[3],
    };
  }
  return { origin: '', timestamp: '', nonce: '' };
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

function validateMessageWindow(messageInfo) {
  const date = new Date(messageInfo.timestamp);
  if (Number.isNaN(date.getTime())) return 'Invalid timestamp';
  const maxAgeMs = 5 * 60 * 1000;
  if (Date.now() - date.getTime() > maxAgeMs) return 'Message expired. Please try again.';
  if (date.getTime() - Date.now() > 60 * 1000) return 'Invalid timestamp';
  return null;
}

async function consumeNonce(db, nonce, origin) {
  const row = await db.prepare(
    'SELECT nonce, origin, expires_at, used_at FROM portal_auth_nonces WHERE nonce = ?'
  )
    .bind(nonce)
    .first();
  if (!row) return { ok: false, message: 'Invalid nonce' };
  if (row.origin !== origin) return { ok: false, message: 'Origin mismatch' };
  if (row.used_at) return { ok: false, message: 'Nonce already used' };
  const expiresAt = new Date(row.expires_at);
  if (Number.isNaN(expiresAt.getTime()) || Date.now() > expiresAt.getTime()) {
    return { ok: false, message: 'Nonce expired. Please try again.' };
  }
  const updated = await db.prepare(
    'UPDATE portal_auth_nonces SET used_at = ? WHERE nonce = ? AND used_at IS NULL'
  )
    .bind(new Date().toISOString(), nonce)
    .run();
  if (!updated.meta || updated.meta.changes !== 1) return { ok: false, message: 'Nonce already used' };
  return { ok: true };
}
