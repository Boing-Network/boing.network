/**
 * Prove control of a portal account via portal password or Ed25519 wallet signature.
 *
 * Password body: { password }
 * Signature body: { message, signature } with message:
 *   Authorize Boing Portal
 *   Action: {action}
 *   Account: {account_id_hex}
 *   Origin: {origin}
 *   Timestamp: {iso}
 *   Nonce: {nonce}
 */
import { createPublicKey, verify } from 'node:crypto';
import { verifyPassword } from '../auth/password.js';
import { hexToBytes, normalizeHex } from './hex.js';

const MAX_MESSAGE_AGE_MS = 5 * 60 * 1000;

/**
 * @param {object} env - Pages Function env with DB
 * @param {string} accountIdHex - normalized 0x account
 * @param {object} body - request JSON
 * @param {string} action - e.g. portal_update | dapp_register | quest_submit
 * @param {{ password_salt?: string|null, password_hash?: string|null } | null} [existingRow]
 */
export async function verifyAccountOwnership(env, accountIdHex, body, action, existingRow = null) {
  const password = typeof body?.password === 'string' ? body.password : '';
  const message = typeof body?.message === 'string' ? body.message : '';
  const signatureHex = normalizeHex(body?.signature || '');

  if (password) {
    let row = existingRow;
    if (!row || row.password_hash == null) {
      row = await env.DB.prepare(
        'SELECT password_salt, password_hash FROM portal_registrations WHERE account_id_hex = ?'
      )
        .bind(accountIdHex)
        .first();
    }
    if (!row || row.password_hash == null || row.password_hash === '') {
      return {
        ok: false,
        status: 401,
        message: 'This account has no portal password. Sign with your wallet or set a password first.',
        error_code: 'need_signature_or_password',
      };
    }
    if (!verifyPassword(password, row.password_salt || '', row.password_hash)) {
      return { ok: false, status: 401, message: 'Incorrect password', error_code: 'bad_password' };
    }
    return { ok: true, method: 'password' };
  }

  if (message && signatureHex) {
    const sigHex = signatureHex.replace(/^0x/, '');
    if (sigHex.length !== 128 || !/^[0-9a-f]+$/.test(sigHex)) {
      return { ok: false, status: 400, message: 'Invalid signature', error_code: 'bad_signature' };
    }
    const publicKeyBytes = hexToBytes(accountIdHex);
    const signatureBytes = hexToBytes(signatureHex);
    if (!publicKeyBytes || !signatureBytes) {
      return { ok: false, status: 400, message: 'Invalid hex', error_code: 'bad_hex' };
    }
    if (!verifyEd25519(publicKeyBytes, Buffer.from(message, 'utf8'), signatureBytes)) {
      return { ok: false, status: 401, message: 'Invalid signature', error_code: 'invalid_signature' };
    }
    const info = parseAuthorizeMessage(message);
    if (!info.ok) {
      return { ok: false, status: 400, message: info.message, error_code: 'invalid_message' };
    }
    if (info.action !== action) {
      return { ok: false, status: 401, message: 'Signature action mismatch', error_code: 'action_mismatch' };
    }
    if (info.account !== accountIdHex) {
      return { ok: false, status: 401, message: 'Signature account mismatch', error_code: 'account_mismatch' };
    }
    const windowError = validateMessageWindow(info.timestamp);
    if (windowError) {
      return { ok: false, status: 401, message: windowError, error_code: 'message_expired' };
    }
    const nonceOk = await consumeNonce(env.DB, info.nonce, info.origin);
    if (!nonceOk.ok) {
      return { ok: false, status: 401, message: nonceOk.message, error_code: 'nonce_invalid' };
    }
    return { ok: true, method: 'signature' };
  }

  return {
    ok: false,
    status: 401,
    message: 'Authentication required (portal password or wallet signature)',
    error_code: 'auth_required',
  };
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

function parseAuthorizeMessage(message) {
  const normalized = String(message || '')
    .replace(/\r\n/g, '\n')
    .replace(/\r/g, '\n')
    .trim();
  const m = normalized.match(
    /^Authorize Boing Portal\s+Action:\s*([a-z0-9_]+)\s+Account:\s*(0x[0-9a-fA-F]{64})\s+Origin:\s*(.+?)\s+Timestamp:\s*(\d{4}-\d{2}-\d{2}T[\d.:]+Z)\s+Nonce:\s*([a-zA-Z0-9_-]+)\s*$/m
  );
  if (!m) {
    return { ok: false, message: 'Invalid authorize message format' };
  }
  const origin = normalizeOrigin(m[3].trim());
  if (!origin) {
    return { ok: false, message: 'Invalid or missing origin' };
  }
  return {
    ok: true,
    action: m[1].trim(),
    account: normalizeHex(m[2]),
    origin,
    timestamp: m[4].trim(),
    nonce: m[5].trim(),
  };
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

function validateMessageWindow(timestamp) {
  const date = new Date(timestamp);
  if (Number.isNaN(date.getTime())) return 'Invalid timestamp';
  if (Date.now() - date.getTime() > MAX_MESSAGE_AGE_MS) return 'Message expired. Please try again.';
  if (date.getTime() - Date.now() > 60 * 1000) return 'Invalid timestamp';
  return null;
}

async function consumeNonce(db, nonce, origin) {
  const row = await db
    .prepare('SELECT nonce, origin, expires_at, used_at FROM portal_auth_nonces WHERE nonce = ?')
    .bind(nonce)
    .first();
  if (!row) return { ok: false, message: 'Invalid nonce' };
  if (row.origin !== origin) return { ok: false, message: 'Origin mismatch' };
  if (row.used_at) return { ok: false, message: 'Nonce already used' };
  const expiresAt = new Date(row.expires_at);
  if (Number.isNaN(expiresAt.getTime()) || Date.now() > expiresAt.getTime()) {
    return { ok: false, message: 'Nonce expired. Please try again.' };
  }
  const updated = await db
    .prepare('UPDATE portal_auth_nonces SET used_at = ? WHERE nonce = ? AND used_at IS NULL')
    .bind(new Date().toISOString(), nonce)
    .run();
  if (!updated.meta || updated.meta.changes !== 1) return { ok: false, message: 'Nonce already used' };
  return { ok: true };
}
