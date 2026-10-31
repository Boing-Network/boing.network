/**
 * GET /api/portal/operator-leaderboard
 * Return ranked list of operators (by blocks proposed) for the Operator Hub Leaderboard page.
 * Data comes from D1 `blocks` table; when the indexer is running, the list is real.
 * Otherwise returns empty list so the UI shows the placeholder.
 */
export async function onRequestGet(context) {
  const { env } = context;
  if (!env.DB) {
    return Response.json({ ok: false, message: 'Database not configured' }, { status: 503 });
  }

  try {
    const blocksTable = await env.DB.prepare(
      "SELECT name FROM sqlite_master WHERE type='table' AND name='blocks'"
    ).first();
    if (!blocksTable) {
      return Response.json({
        ok: true,
        operators: [],
        total_operators: 0,
        total_blocks: 0,
        message: 'Indexer not configured',
      });
    }

    const rows = await env.DB.prepare(
      `SELECT proposer, COUNT(*) as blocks_proposed,
              MIN(created_at) as first_block_at, MAX(created_at) as last_block_at
       FROM blocks
       WHERE proposer IS NOT NULL AND proposer != ''
       GROUP BY proposer
       ORDER BY blocks_proposed DESC`
    ).all();

    const results = rows.results || [];
    let total_blocks = 0;
    const operators = results.map((row, index) => {
      const blocks_proposed = Number(row.blocks_proposed) || 0;
      total_blocks += blocks_proposed;
      return {
        rank: index + 1,
        proposer: row.proposer || '',
        blocks_proposed,
        first_block_at: row.first_block_at || null,
        last_block_at: row.last_block_at || null,
      };
    });

    return Response.json({
      ok: true,
      operators,
      total_operators: operators.length,
      total_blocks,
    });
  } catch (e) {
    return Response.json({ ok: false, message: 'Server error' }, { status: 500 });
  }
}
