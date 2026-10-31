/**
 * GET /api/portal/operator-stats?account_id_hex=0x...
 * Return per-operator stats (blocks proposed, rank) for the Operator Hub My Dashboard.
 * Data comes from D1 `blocks` table; when an indexer is running and filling blocks,
 * stats will be real. Otherwise returns nulls so the UI shows "—".
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
    const blocksTable = await env.DB.prepare(
      "SELECT name FROM sqlite_master WHERE type='table' AND name='blocks'"
    ).first();
    if (!blocksTable) {
      return Response.json({
        ok: true,
        blocks_proposed: null,
        rank: null,
        total_operators: 0,
        uptime_estimate: null,
        message: 'Indexer not configured',
      });
    }

    const countResult = await env.DB.prepare(
      'SELECT COUNT(*) as n FROM blocks WHERE proposer = ?'
    )
      .bind(account_id_hex)
      .first();
    const blocks_proposed = countResult?.n ?? 0;

    const allProposers = await env.DB.prepare(
      'SELECT proposer, COUNT(*) as c FROM blocks GROUP BY proposer ORDER BY c DESC'
    ).all();
    const rows = allProposers.results || [];
    const total_operators = rows.length;
    let rank = null;
    for (let i = 0; i < rows.length; i++) {
      if ((rows[i].proposer || '').toLowerCase() === account_id_hex.toLowerCase()) {
        rank = i + 1;
        break;
      }
    }

    const firstLast = await env.DB.prepare(
      'SELECT MIN(created_at) as first_at, MAX(created_at) as last_at FROM blocks WHERE proposer = ?'
    )
      .bind(account_id_hex)
      .first();
    const uptime_estimate =
      firstLast?.first_at && firstLast?.last_at
        ? { first_block_at: firstLast.first_at, last_block_at: firstLast.last_at }
        : null;

    return Response.json({
      ok: true,
      blocks_proposed: blocks_proposed > 0 ? blocks_proposed : null,
      rank,
      total_operators,
      uptime_estimate,
    });
  } catch (e) {
    return Response.json({ ok: false, message: 'Server error' }, { status: 500 });
  }
}

function normalizeHex(s) {
  if (!s || typeof s !== 'string') return '';
  const t = s.trim().toLowerCase();
  return t.startsWith('0x') ? t : '0x' + t;
}
