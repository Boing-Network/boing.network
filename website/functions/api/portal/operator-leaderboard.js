/**
 * GET /api/portal/operator-leaderboard
 * Validators ranked by Passport tier, then sampled uptime. Live set comes from boing_getValidators;
 * history comes from validator_uptime_samples written every ~15 min by boing-credibility-indexer.
 */
import { rpcCall, DEFAULT_RPC_URL } from './_lib/passportVerify.js';
import { shortHex, tierRank } from './_lib/passportScore.js';

export async function onRequestGet({ env }) {
  if (!env.DB) return Response.json({ ok: false, message: 'Database not configured' }, { status: 503 });
  try {
    const [agg, tiers, state] = await env.DB.batch([
      env.DB.prepare(`SELECT validator_id, MAX(owner) AS owner, COUNT(*) AS samples,
          AVG(CASE WHEN in_set = 1 THEN uptime_pct END) AS avg_uptime, SUM(in_set) AS in_set_samples,
          MIN(sampled_at) AS first_seen, MAX(sampled_at) AS last_seen
        FROM validator_uptime_samples GROUP BY validator_id`),
      env.DB.prepare("SELECT account_id_hex, tier, score FROM passport_tiers WHERE track = 'node_operator'"),
      env.DB.prepare("SELECT updated_at FROM indexer_state WHERE key = 'credibility_last_tick'"),
    ]);
    let live = [];
    try { live = (await rpcCall(env.TESTNET_RPC_URL || DEFAULT_RPC_URL, 'boing_getValidators', [])).validators || []; } catch { /* history only */ }
    const tierBy = Object.fromEntries((tiers.results || []).map((t) => [t.account_id_hex, t]));
    const byId = new Map();
    for (const a of agg.results || []) byId.set(a.validator_id, { id: a.validator_id, owner: a.owner, samples: a.samples, in_set_samples: a.in_set_samples,
      avg_uptime: a.avg_uptime == null ? null : Math.round(a.avg_uptime * 100) / 100, first_seen: a.first_seen, last_seen: a.last_seen });
    for (const v of live) {
      const id = String(v.id).toLowerCase();
      const e = byId.get(id) || { id, owner: String(v.owner || '').toLowerCase(), samples: 0, in_set_samples: 0, avg_uptime: null };
      Object.assign(e, { kind: v.kind, status: v.status, in_set: !!v.in_set, uptime_pct: v.uptime_pct,
        blocks_proposed: Math.max(0, (v.scheduled_proposals || 0) - (v.missed_proposals || 0)), missed_proposals: v.missed_proposals || 0 });
      byId.set(id, e);
    }
    const ops = [...byId.values()].map((e) => {
      const t = tierBy[e.owner] || tierBy[e.id] || null;
      return { ...e, tier: t?.tier || 'none', score: t?.score || 0, id_short: shortHex(e.id), owner_short: shortHex(e.owner) };
    }).sort((a, b) => (tierRank(b.tier) - tierRank(a.tier)) || ((b.avg_uptime ?? -1) - (a.avg_uptime ?? -1)) || (b.blocks_proposed || 0) - (a.blocks_proposed || 0));
    const operators = ops.map(({ id, owner, ...rest }, i) => ({ rank: i + 1, ...rest }));
    return Response.json({
      ok: true,
      operators,
      total_operators: operators.length,
      total_blocks: operators.reduce((s, o) => s + (o.blocks_proposed || 0), 0),
      indexer_last_tick: state.results?.[0]?.updated_at || null,
    }, { headers: { 'cache-control': 'public, max-age=60' } });
  } catch (e) {
    return Response.json({ ok: false, message: 'Server error' }, { status: 500 });
  }
}
