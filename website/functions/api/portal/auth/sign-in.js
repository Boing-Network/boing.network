/**
 * POST /api/portal/auth/sign-in
 * Wallet-based sign-in: verify Ed25519(message, signature) with public key = account_id_hex,
 * then verify portal password if account has one. Returns registration payload or need_password.
 * Body: { account_id_hex, message, signature, password? } — password required when account has a portal password.
 */
import { createPublicKey, verify } from 'node:crypto';
import { verifyPassword } from './password.js';

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
    const password = typeof body.password === 'string' ? body.password : '';

    if (!account_id_hex || account_id_hex.length !== 66) {
      return Response.json({ ok: false, message: 'Invalid account_id_hex (must be 32-byte hex with 0x)' }, { status: 400 });
    }
    if (!message) {
      return Response.json({ ok: false, message: 'Missing message' }, { status: 400 });
    }
    // Ed25519 signature is 64 bytes = 128 hex chars (with or without 0x prefix)
    const sigHex = signatureHex.replace(/^0x/, '');
    if (sigHex.length !== 128 || !/^[0-9a-f]+$/.test(sigHex)) {
      return Response.json({ ok: false, message: 'Invalid signature (must be 64-byte hex)' }, { status: 400 });
    }

    const publicKeyBytes = hexToBytes(account_id_hex);
    const signatureBytes = hexToBytes(signatureHex);
    if (!publicKeyBytes || !signatureBytes) {
      return Response.json({ ok: false, message: 'Invalid hex in account_id_hex or signature' }, { status: 400 });
    }

    const valid = verifyEd25519(publicKeyBytes, Buffer.from(message, 'utf8'), signatureBytes);
    if (!valid) {
      return Response.json({ ok: false, message: 'Invalid signature' }, { status: 401 });
    }

    const messageInfo = parseSignInMessage(message);
    const messageError = validateMessageWindow(messageInfo);
    if (messageError) {
      return Response.json({ ok: false, message: messageError }, { status: 401 });
    }

    if (messageInfo.nonce) {
      const nonceOk = await consumeNonce(env.DB, messageInfo.nonce, messageInfo.origin);
      if (!nonceOk.ok) {
        return Response.json({ ok: false, message: nonceOk.message }, { status: 401 });
      }
    }

    const row = await env.DB.prepare(
      'SELECT account_id_hex, role, email, discord_handle, github_username, node_multiaddr, password_salt, password_hash, created_at FROM portal_registrations WHERE account_id_hex = ?'
    )
      .bind(account_id_hex)
      .first();

    if (!row) {
      return Response.json({ ok: false, message: 'Account not registered' }, { status: 403 });
    }

    if (row.password_hash != null && row.password_hash !== '') {
      if (!password) {
        return Response.json({ ok: false, message: 'Portal password required for wallet sign-in', need_password: false }, { status: 403 });
      }
      if (!verifyPassword(password, row.password_salt || '', row.password_hash)) {
        return Response.json({ ok: false, message: 'Incorrect portal password' }, { status: 403 });
      }
    } else {
      if (!password) {
        return Response.json({
          ok: false,
          message: 'Set a portal password first so you can sign in with your wallet.',
          need_password: true,
          account_id_hex: row.account_id_hex,
        }, { status: 403 });
      }
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

/**
 * Verify Ed25519 signature using Node built-in crypto (no external libs).
 * @param {Buffer} publicKeyBytes - 32-byte Ed25519 public key (Boing account_id)
 * @param {Buffer} messageBytes - Raw bytes of the message that was signed (e.g. UTF-8)
 * @param {Buffer} signatureBytes - 64-byte Ed25519 signature
 * @returns {boolean}
 */
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

function parseSignInMessage(message) {
  const modern = message.match(
    /^Sign in to Boing Portal\s+Origin:\s*(.+)\s+Timestamp:\s*(\d{4}-\d{2}-\d{2}T[\d.:]+Z)\s+Nonce:\s*([a-zA-Z0-9_-]+)\s*$/m
  );
  if (modern) {
    return {
      origin: normalizeOrigin(modern[1]),
      timestamp: modern[2],
      nonce: modern[3],
      version: 'nonce',
    };
  }

  const legacy = message.match(/^Sign in to Boing Portal at (.+) at (\d{4}-\d{2}-\d{2}T[\d.:]+Z)\s*$/);
  if (legacy) {
    return {
      origin: normalizeOrigin(legacy[1]),
      timestamp: legacy[2],
      nonce: '',
      version: 'legacy',
    };
  }

  return {
    origin: '',
    timestamp: '',
    nonce: '',
    version: 'unknown',
  };
}

function validateMessageWindow(messageInfo) {
  if (!messageInfo.timestamp) {
    return 'Invalid sign-in message';
  }
  if (!messageInfo.origin) {
    return 'Invalid or missing sign-in origin';
  }
  const date = new Date(messageInfo.timestamp);
  if (Number.isNaN(date.getTime())) {
    return 'Invalid sign-in timestamp';
  }
  const maxAgeMs = 5 * 60 * 1000;
  if (Date.now() - date.getTime() > maxAgeMs) {
    return 'Sign-in message expired. Please try again.';
  }
  if (date.getTime() - Date.now() > 60 * 1000) {
    return 'Invalid sign-in timestamp';
  }
  return null;
}

async function consumeNonce(db, nonce, origin) {
  const row = await db.prepare(
    'SELECT nonce, origin, expires_at, used_at FROM portal_auth_nonces WHERE nonce = ?'
  )
    .bind(nonce)
    .first();

  if (!row) {
    return { ok: false, message: 'Invalid sign-in nonce' };
  }
  if (row.origin !== origin) {
    return { ok: false, message: 'Sign-in origin mismatch' };
  }
  if (row.used_at) {
    return { ok: false, message: 'Sign-in nonce already used' };
  }
  const expiresAt = new Date(row.expires_at);
  if (Number.isNaN(expiresAt.getTime()) || Date.now() > expiresAt.getTime()) {
    return { ok: false, message: 'Sign-in nonce expired. Please try again.' };
  }

  const updated = await db.prepare(
    'UPDATE portal_auth_nonces SET used_at = ? WHERE nonce = ? AND used_at IS NULL'
  )
    .bind(new Date().toISOString(), nonce)
    .run();

  if (!updated.meta || updated.meta.changes !== 1) {
    return { ok: false, message: 'Sign-in nonce already used' };
  }

  return { ok: true };
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
