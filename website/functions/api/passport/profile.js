/**
 * GET /api/passport/profile?account_id_hex=0x...
 * Public, read-only Passport profile: tracks, indexed signals, badges, tiers + next-tier progress.
 * Data is written by the boing-credibility-indexer Worker (every 10 min). No PII is returned.
 */
import { isAccountHex, normalizeHex } from '../portal/_lib/hex.js';
import { PASSPORT_RULES, TRACK_IDS } from '../portal/_lib/quests.js';
import { nextTierProgress } from '../portal/_lib/passportScore.js';

export async function onRequestGet({ env, request }) {
  const account = normalizeHex(new URL(request.url).searchParams.get('account_id_hex') || '');
  if (!isAccountHex(account)) return Response.json({ ok: false, message: 'Missing or invalid account_id_hex' }, { status: 400 });
  if (!env.DB) return Response.json({ ok: false, message: 'Database not configured' }, { status: 503 });
  try {
    const [reg, tracks, signals, badges, tiers, quests] = await env.DB.batch([
      env.DB.prepare('SELECT role, created_at FROM portal_registrations WHERE lower(account_id_hex) = ?').bind(account),
      env.DB.prepare('SELECT track, joined_at FROM portal_tracks WHERE lower(account_id_hex) = ?').bind(account),
      env.DB.prepare('SELECT * FROM passport_signals WHERE account_id_hex = ?').bind(account),
      env.DB.prepare('SELECT badge_id, track, earned_at, historic, evidence FROM passport_badges WHERE account_id_hex = ? ORDER BY earned_at').bind(account),
      env.DB.prepare('SELECT track, tier, score, badge_count, updated_at FROM passport_tiers WHERE account_id_hex = ?').bind(account),
      env.DB.prepare('SELECT quest_id, verification_status, verified_at FROM quest_completions WHERE lower(account_id_hex) = ?').bind(account),
    ]);
    const r = reg.results?.[0] || null;
    const trackIds = (tracks.results || []).map((t) => t.track).filter((t) => TRACK_IDS.includes(t));
    if (!trackIds.length && r && TRACK_IDS.includes(r.role)) trackIds.push(r.role);
    const badgeRows = (badges.results || []).map((b) => ({ ...b, historic: !!b.historic }));
    const tierRows = Object.fromEntries((tiers.results || []).map((t) => [t.track, t]));
    const byId = Object.fromEntries(PASSPORT_RULES.badges.map((b) => [b.id, b]));
    const badgeObjs = badgeRows.map((b) => ({ ...b, id: b.badge_id, name: byId[b.badge_id]?.name || b.badge_id, points: byId[b.badge_id]?.points || 0 }));
    const trackOut = trackIds.map((t) => {
      const tier = tierRows[t]?.tier || 'none';
      return { track: t, tier, score: tierRows[t]?.score || 0, badge_count: tierRows[t]?.badge_count || 0,
        progress: nextTierProgress(t, badgeObjs, tier) };
    });
    const s = signals.results?.[0] || null;
    return Response.json({
      ok: true,
      account_id_hex: account,
      registered: !!r,
      registered_at: r?.created_at || null,
      rules_version: PASSPORT_RULES.rules_version,
      indexed: !!s,
      indexed_at: s?.updated_at || null,
      signals: s,
      tracks: trackOut,
      badges: badgeObjs,
      quests: quests.results || [],
      gold_note: PASSPORT_RULES.tiers.verified_social_status,
    }, { headers: { 'cache-control': 'public, max-age=60' } });
  } catch (e) {
    return Response.json({ ok: false, message: 'Server error' }, { status: 500 });
  }
}
