/**
 * GET /api/portal/quests-stats
 * Return hub-wide quest completion stats for the Community Hub page.
 */
export async function onRequestGet(context) {
  const { env } = context;
  if (!env.DB) {
    return Response.json({ ok: false, message: 'Database not configured' }, { status: 503 });
  }
  try {
    const total = await env.DB.prepare(
      'SELECT COUNT(*) as n FROM quest_completions'
    ).first();
    const byQuest = await env.DB.prepare(
      'SELECT quest_id, COUNT(*) as n FROM quest_completions GROUP BY quest_id'
    ).all();
    const by_quest = {};
    (byQuest.results || []).forEach((row) => {
      by_quest[row.quest_id] = row.n;
    });
    return Response.json({
      ok: true,
      total_completions: total?.n ?? 0,
      by_quest,
    });
  } catch (e) {
    return Response.json({ ok: false, message: 'Server error' }, { status: 500 });
  }
}
