/**
 * POST /api/portal/auth/sign-in
 * Wallet-based sign-in: Ed25519 only (Boing-native). No EVM/Solana/other-chain dependencies.
 * Supports both raw-message and BLAKE3(message) signing (Boing tx style).
 * BLAKE3 variants are tried first so Boing Express (signs BLAKE3(UTF-8 message)) verifies without extra variants.
 * After changing this file, redeploy the site so the live portal uses the new code (push to main or run npm run deploy from website/).
 * Body: { account_id_hex, message, signature }
 */
import { createPublicKey, verify } from 'node:crypto';
import { blake3 } from '@noble/hashes/blake3.js';

export async function onRequestPost(context) {
  const { env, request } = context;
  if (!env.DB) {
    return Response.json({ ok: false, message: 'Database not configured', error_code: 'config' }, { status: 503 });
  }

  const json401 = (message, error_code) =>
    Response.json({ ok: false, message, error_code }, { status: 401 });

  try {
    const body = await request.json();
    const account_id_hex = normalizeHex(body.account_id_hex);
    const messageRaw = typeof body.message === 'string' ? body.message : '';
    const message = normalizeMessage(messageRaw);
    const signatureHex = normalizeHex(body.signature || '');

    if (!message) {
      return Response.json({ ok: false, message: 'Missing message', error_code: 'missing_message' }, { status: 400 });
    }
    // Boing-native: 32-byte account (0x + 64 hex), 64-byte Ed25519 signature (128 hex) only
    if (!account_id_hex || account_id_hex.length !== 66) {
      return Response.json({ ok: false, message: 'Invalid account_id_hex (must be 0x + 64 hex chars, Boing Ed25519)', error_code: 'bad_account' }, { status: 400 });
    }
    const sigHex = signatureHex.replace(/^0x/, '');
    if (sigHex.length !== 128 || !/^[0-9a-f]+$/.test(sigHex)) {
      return Response.json({ ok: false, message: 'Invalid signature (must be 64-byte hex, Ed25519)', error_code: 'bad_signature' }, { status: 400 });
    }

    const publicKeyBytes = hexToBytes(account_id_hex);
    const signatureBytes = hexToBytes(signatureHex);
    if (!publicKeyBytes || !signatureBytes) {
      return Response.json({ ok: false, message: 'Invalid hex', error_code: 'bad_hex' }, { status: 400 });
    }

    // Try BLAKE3(message) first (Boing Express and Boing tx convention), then raw variants
    const blake3Variants = messageVariantsBLAKE3(messageRaw);
    let valid = false;
    for (const hashBuf of blake3Variants) {
      if (verifyEd25519(publicKeyBytes, hashBuf, signatureBytes)) {
        valid = true;
        break;
      }
    }
    if (!valid) {
      const variants = messageVariants(messageRaw);
      for (const msgBuf of variants) {
        if (verifyEd25519(publicKeyBytes, msgBuf, signatureBytes)) {
          valid = true;
          break;
        }
      }
    }
    if (!valid) {
      return json401('Invalid signature. Use a Boing-native wallet (e.g. Boing Express) and sign the exact message.', 'invalid_signature');
    }

    // Parse and validate message structure and nonce (after signature is valid)
    const messageInfo = parseSignInMessage(message);
    const messageError = validateMessageWindow(messageInfo);
    if (messageError) {
      const code = !messageInfo.timestamp ? 'invalid_message' : messageError.includes('expired') ? 'message_expired' : 'invalid_message';
      return json401(messageError, code);
    }
    if (messageInfo.nonce) {
      const nonceCheck = await checkNonce(env.DB, messageInfo.nonce, messageInfo.origin);
      if (!nonceCheck.ok) {
        const code = nonceCheck.message.includes('expired') ? 'nonce_expired' : nonceCheck.message.includes('already used') ? 'nonce_used' : 'nonce_invalid';
        return json401(nonceCheck.message, code);
      }
    }

    if (messageInfo.nonce) {
      const nonceOk = await consumeNonce(env.DB, messageInfo.nonce, messageInfo.origin);
      if (!nonceOk.ok) {
        return json401(nonceOk.message, 'nonce_used');
      }
    }

    const row = await env.DB.prepare(
      'SELECT account_id_hex, role, email, discord_handle, github_username, node_multiaddr, created_at FROM portal_registrations WHERE account_id_hex = ?'
    )
      .bind(account_id_hex)
      .first();

    if (!row) {
      return Response.json({ ok: false, message: 'Account not registered', error_code: 'not_registered' }, { status: 403 });
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

function normalizeMessage(s) {
  if (!s || typeof s !== 'string') return '';
  return s.replace(/\r\n/g, '\n').replace(/\r/g, '\n').trim();
}

function buildEIP191Message(message) {
  const msgBuf = Buffer.from(message, 'utf8');
  const prefix = Buffer.from(`\x19Ethereum Signed Message:\n${msgBuf.length}`, 'utf8');
  return Buffer.concat([prefix, msgBuf]);
}

/** All message byte variants to try for Ed25519 (wallet may sign raw, trimmed, with trailing newline, or EIP-191 style). */
function messageVariants(messageRaw) {
  const normalized = normalizeMessage(messageRaw);
  const trimRaw = messageRaw.trim();
  const add = (msg) => {
    if (typeof msg === 'string') return Buffer.from(msg, 'utf8');
    return msg;
  };
  return [
    add(messageRaw),
    add(trimRaw),
    add(normalized),
    add(messageRaw + '\n'),
    add(trimRaw + '\n'),
    add(normalized + '\n'),
    buildEIP191Message(messageRaw),
    buildEIP191Message(trimRaw),
    buildEIP191Message(normalized),
  ];
}

/** BLAKE3(message) variants — Boing tx signing uses Ed25519(BLAKE3(...)); wallet may do same for message sign. */
function messageVariantsBLAKE3(messageRaw) {
  const rawVariants = [
    messageRaw,
    messageRaw.trim(),
    normalizeMessage(messageRaw),
    messageRaw + '\n',
    messageRaw.trim() + '\n',
    normalizeMessage(messageRaw) + '\n',
  ];
  const out = [];
  for (const msg of rawVariants) {
    const buf = typeof msg === 'string' ? Buffer.from(msg, 'utf8') : msg;
    const u8 = buf instanceof Uint8Array ? buf : new Uint8Array(buf);
    out.push(blake3(u8));
  }
  return out;
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

function parseSignInMessage(message) {
  const normalized = normalizeMessage(message);
  const modern = normalized.match(
    /^Sign in to Boing Portal\s+Origin:\s*(.+?)\s+Timestamp:\s*(\d{4}-\d{2}-\d{2}T[\d.:]+Z)\s+Nonce:\s*([a-zA-Z0-9_-]+)\s*$/m
  );
  if (modern) {
    return {
      origin: normalizeOrigin(modern[1].trim()),
      timestamp: modern[2].trim(),
      nonce: modern[3].trim(),
      version: 'nonce',
    };
  }
  const legacy = normalized.match(/^Sign in to Boing Portal at (.+?) at (\d{4}-\d{2}-\d{2}T[\d.:]+Z)\s*$/);
  if (legacy) {
    return {
      origin: normalizeOrigin(legacy[1].trim()),
      timestamp: legacy[2].trim(),
      nonce: '',
      version: 'legacy',
    };
  }
  return { origin: '', timestamp: '', nonce: '', version: 'unknown' };
}

function validateMessageWindow(messageInfo) {
  if (!messageInfo.timestamp) {
    return 'Invalid sign-in message format';
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

async function checkNonce(db, nonce, origin) {
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
  return { ok: true };
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
