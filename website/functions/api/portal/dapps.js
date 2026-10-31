/**
 * POST /api/portal/dapps
 * Register a dApp for testnet (developer). Requires owner proof (password or wallet signature).
 * Body: { contract_hex, owner_account_hex, name?, password? | message+signature }
 * Signature action: dapp_register
 * GET /api/portal/dapps — List all registered dApps (for hub community page).
 */
import { isAccountHex, normalizeHex } from './_lib/hex.js';
import { verifyAccountOwnership } from './_lib/ownership.js';

export async function onRequestGet(context) {
  const { env } = context;
  if (!env.DB) {
    return Response.json({ ok: false, message: 'Database not configured' }, { status: 503 });
  }
  try {
    const result = await env.DB.prepare(
      'SELECT contract_hex, owner_account_hex, name, registered_at FROM portal_dapps ORDER BY registered_at DESC'
    ).all();
    return Response.json({ ok: true, dapps: result.results || [] });
  } catch (e) {
    return Response.json({ ok: false, message: 'Server error' }, { status: 500 });
  }
}

export async function onRequestPost(context) {
  const { env, request } = context;
  if (!env.DB) {
    return Response.json({ ok: false, message: 'Database not configured' }, { status: 503 });
  }
  try {
    const body = await request.json();
    const contract_hex = normalizeHex(body.contract_hex);
    const owner_account_hex = normalizeHex(body.owner_account_hex || '');
    const name = sanitizeOptionalText(body.name, 128);

    if (!isAccountHex(contract_hex)) {
      return Response.json({ ok: false, message: 'Invalid contract_hex (must be 32-byte hex with 0x)' }, { status: 400 });
    }
    if (!isAccountHex(owner_account_hex)) {
      return Response.json({ ok: false, message: 'owner_account_hex is required (must be 32-byte hex with 0x)' }, { status: 400 });
    }

    // Prove the caller controls the claimed owner account (audit #61 item 64).
    const auth = await verifyAccountOwnership(env, owner_account_hex, body, 'dapp_register');
    if (!auth.ok) {
      return Response.json(
        { ok: false, message: auth.message, error_code: auth.error_code },
        { status: auth.status || 401 }
      );
    }

    const ownerReg = await env.DB.prepare(
      'SELECT account_id_hex, role FROM portal_registrations WHERE account_id_hex = ?'
    )
      .bind(owner_account_hex)
      .first();
    if (!ownerReg) {
      return Response.json({ ok: false, message: 'Owner account is not registered', error_code: 'not_registered' }, { status: 403 });
    }
    if (ownerReg.role !== 'developer') {
      return Response.json({ ok: false, message: 'Owner must be registered as a developer', error_code: 'wrong_role' }, { status: 403 });
    }

    const existing = await env.DB.prepare(
      'SELECT contract_hex, owner_account_hex, name FROM portal_dapps WHERE contract_hex = ?'
    )
      .bind(contract_hex)
      .first();

    const now = new Date().toISOString();

    if (existing) {
      if (existing.owner_account_hex && existing.owner_account_hex !== owner_account_hex) {
        return Response.json(
          {
            ok: false,
            message: 'This contract is already registered to another owner',
            error_code: 'owner_conflict',
          },
          { status: 409 }
        );
      }
      await env.DB.prepare(
        'UPDATE portal_dapps SET owner_account_hex = ?, name = COALESCE(?, name) WHERE contract_hex = ?'
      )
        .bind(owner_account_hex, name, contract_hex)
        .run();
      return Response.json({ ok: true, message: 'dApp updated', updated: true });
    }

    await env.DB.prepare(
      'INSERT INTO portal_dapps (contract_hex, owner_account_hex, name, registered_at) VALUES (?, ?, ?, ?)'
    )
      .bind(contract_hex, owner_account_hex, name, now)
      .run();

    return Response.json({ ok: true, message: 'dApp registered' });
  } catch (e) {
    return Response.json({ ok: false, message: 'Server error' }, { status: 500 });
  }
}

function sanitizeOptionalText(value, maxLen) {
  if (value == null) return null;
  if (typeof value !== 'string') return null;
  const t = value.trim();
  if (!t) return null;
  return t.replace(/[<>]/g, '').slice(0, maxLen) || null;
}
