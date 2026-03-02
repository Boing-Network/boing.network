/**
 * POST /api/portal/dapps
 * Register a dApp for testnet (developer). Body: { contract_hex, owner_account_hex?, name? }
 */
export async function onRequestPost(context) {
  const { env, request } = context;
  if (!env.DB) {
    return Response.json({ ok: false, message: 'Database not configured' }, { status: 503 });
  }
  try {
    const body = await request.json();
    const contract_hex = normalizeHex(body.contract_hex);
    const owner_account_hex = body.owner_account_hex ? normalizeHex(body.owner_account_hex) : null;
    const name = body.name?.trim() || null;

    if (!contract_hex || contract_hex.length !== 66) {
      return Response.json({ ok: false, message: 'Invalid contract_hex (must be 32-byte hex with 0x)' }, { status: 400 });
    }
    if (owner_account_hex && owner_account_hex.length !== 66) {
      return Response.json({ ok: false, message: 'Invalid owner_account_hex (must be 32-byte hex with 0x)' }, { status: 400 });
    }

    const now = new Date().toISOString();
    await env.DB.prepare(
      'INSERT INTO portal_dapps (contract_hex, owner_account_hex, name, registered_at) VALUES (?, ?, ?, ?) ON CONFLICT(contract_hex) DO UPDATE SET owner_account_hex = excluded.owner_account_hex, name = excluded.name'
    )
      .bind(contract_hex, owner_account_hex, name, now)
      .run();

    return Response.json({ ok: true, message: 'dApp registered' });
  } catch (e) {
    return Response.json({ ok: false, message: e.message || 'Server error' }, { status: 500 });
  }
}

function normalizeHex(s) {
  if (!s || typeof s !== 'string') return '';
  const t = s.trim().toLowerCase();
  return t.startsWith('0x') ? t : '0x' + t;
}
