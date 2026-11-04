/**
 * GET /api/passport/leaderboard?track=user|developer|node_operator[&limit=100]
 * Public per-track leaderboard ordered by tier, then score. Addresses are shortened server-side.
 */
import { isTrackId, PASSPORT_RULES } from '../portal/_lib/quests.js';
import { shortHex } from '../portal/_lib/passportScore.js';

export async function onRequestGet({ env, request }) {
  const url = new URL(request.url);
  const track = url.searchParams.get('track') || 'user';
  if (!isTrackId(track)) return Response.json({ ok: false, message: 'Unknown track' }, { status: 400 });
  const limit = Math.min(200, Math.max(1, Number(url.searchParams.get('limit')) || 100));
  if (!env.DB) return Response.json({ ok: false, message: 'Database not configured' }, { status: 503 });
  try {
    const [rows, counts, state] = await env.DB.batch([
      env.DB.prepare(`SELECT account_id_hex, tier, score, badge_count, updated_at FROM passport_tiers
        WHERE track = ? ORDER BY tier_rank DESC, score DESC, updated_at ASC LIMIT ?`).bind(track, limit),
      env.DB.prepare('SELECT tier, COUNT(*) AS n FROM passport_tiers WHERE track = ? GROUP BY tier').bind(track),
      env.DB.prepare("SELECT updated_at FROM indexer_state WHERE key = 'credibility_last_tick'"),
    ]);
    const entries = (rows.results || []).map((r, i) => ({
      rank: i + 1, account: shortHex(r.account_id_hex), tier: r.tier, score: r.score, badges: r.badge_count, updated_at: r.updated_at,
    }));
    return Response.json({
      ok: true, track, rules_version: PASSPORT_RULES.rules_version,
      tiers: Object.fromEntries((counts.results || []).map((c) => [c.tier, c.n])),
      indexer_last_tick: state.results?.[0]?.updated_at || null,
      entries,
    }, { headers: { 'cache-control': 'public, max-age=60' } });
  } catch (e) {
    return Response.json({ ok: false, message: 'Server error' }, { status: 500 });
  }
}
