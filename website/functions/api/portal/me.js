/**
 * GET /api/portal/me?account_id_hex=0x...
 * Return registration and role-specific summary for the given account.
 */
export async function onRequestGet(context) {
  const { env, request } = context;
  if (!env.DB) {
    return Response.json({ ok: false, message: 'Database not configured' }, { status: 503 });
  }
  const url = new URL(request.url);
  const account_id_hex = normalizeHex(url.searchParams.get('account_id_hex') || '');
  if (!account_id_hex || account_id_hex.length !== 66) {
    return Response.json({ ok: false, message: 'Missing or invalid account_id_hex' }, { status: 400 });
  }

  try {
    const row = await env.DB.prepare(
      'SELECT account_id_hex, role, email, discord_handle, github_username, node_multiaddr, created_at FROM portal_registrations WHERE account_id_hex = ?'
    )
      .bind(account_id_hex)
      .first();

    if (!row) {
      return Response.json({ ok: true, registered: false });
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
