/**
 * boing-credibility-indexer — Testnet Passport stage 2.
 * Runs every 15 min: POST /tick from boing-native-dex-indexer's cron via service binding (no own cron: Free 5-cron cap).
 * Also has a scheduled() handler so a cron can be re-added later. Read-only against the chain; writes only to D1 passport_* / validator_uptime_samples / indexer_state.
 */
import { PASSPORT_RULES, TRACK_IDS } from '../../../website/functions/api/portal/_lib/quests.js';
import { rpcCall, fetchDexContracts } from '../../../website/functions/api/portal/_lib/passportVerify.js';
import {
  computeSignals, computeBadges, computeTiers, tierRank, snapshotLeaf, merkleRoot, isoWeek,
} from '../../../website/functions/api/portal/_lib/passportScore.js';
import { syncDiscordRoles } from './discord.js';

const lc = (s) => String(s || '').toLowerCase();
const num = (v, d) => (Number.isFinite(Number(v)) && Number(v) > 0 ? Number(v) : d);

async function fetchHistory(rpc, account, maxPages) {
  const txs = [];
  let cursor = null;
  let pages = 0;
  while (pages < maxPages) {
    const r = await rpcCall(rpc, 'boing_getAccountTransactions', [account, { cursor, limit: 100 }]);
    pages++;
    txs.push(...(r.transactions || []));
    cursor = r.next_cursor;
    if (!cursor) return { txs, complete: true, pages };
  }
  return { txs, complete: false, pages };
}

async function sampleValidators(env, rpc, nowIso) {
  const r = await rpcCall(rpc, 'boing_getValidators', []);
  const vals = r.validators || [];
  if (vals.length) {
    await env.DB.batch(vals.map((v) => env.DB.prepare(
      `INSERT OR IGNORE INTO validator_uptime_samples (sampled_at, validator_id, owner, in_set, status, uptime_pct, proposals_scheduled, proposals_missed)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
    ).bind(nowIso, lc(v.id), lc(v.owner), v.in_set ? 1 : 0, v.status ?? null,
      v.uptime_pct == null ? null : Number(v.uptime_pct), v.scheduled_proposals ?? null, v.missed_proposals ?? null)));
  }
  return vals;
}

async function refreshAccount(env, ctx, row) {
  const acct = lc(row.account_id_hex);
  const pages = Math.min(PASSPORT_RULES.verification.max_history_pages || 10, ctx.pageBudget);
  const hist = await fetchHistory(ctx.rpc, acct, pages);
  ctx.pageBudget -= hist.pages;

  const [tracksRes, questsRes, upRes, discordRes] = await env.DB.batch([
    env.DB.prepare('SELECT track FROM portal_tracks WHERE account_id_hex = ?').bind(acct),
    env.DB.prepare("SELECT quest_id, verified_at FROM quest_completions WHERE account_id_hex = ? AND verification_status = 'verified'").bind(acct),
    env.DB.prepare('SELECT AVG(uptime_pct) AS avg, COUNT(*) AS n FROM validator_uptime_samples WHERE (validator_id = ? OR owner = ?) AND in_set = 1').bind(acct, acct),
    env.DB.prepare('SELECT 1 AS linked FROM discord_links WHERE account_id_hex = ?').bind(acct),
  ]);
  let tracks = (tracksRes.results || []).map((r) => r.track).filter((t) => TRACK_IDS.includes(t));
  if (!tracks.length && TRACK_IDS.includes(row.role)) tracks = [row.role];
  const verifiedQuests = Object.fromEntries((questsRes.results || []).map((q) => [q.quest_id, q.verified_at || ctx.nowIso]));
  const up = upRes.results?.[0] || {};
  const uptime = { avg_pct: up.avg == null ? null : Number(up.avg), samples: Number(up.n || 0) };

  const signals = computeSignals(acct, hist.txs, ctx.dex, ctx.validators);
  const badges = computeBadges({ signals, verifiedQuests, uptime, registeredAt: row.created_at, nowIso: ctx.nowIso });
  // Gold requires a verified social link: a Discord account linked via OAuth (stage 4).
  const verifiedSocial = (discordRes.results || []).length > 0;
  const tiers = computeTiers(badges, tracks, { verifiedSocial });
  const c = signals.counts;

  const stmts = [
    env.DB.prepare(
      `INSERT INTO passport_signals (account_id_hex, tx_sent, deploys, dex_calls, faucet_drips, active_days, validator_joined,
         uptime_avg_pct, uptime_samples, first_activity_at, last_activity_at, history_complete, rules_version, updated_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
       ON CONFLICT(account_id_hex) DO UPDATE SET tx_sent=excluded.tx_sent, deploys=excluded.deploys, dex_calls=excluded.dex_calls,
         faucet_drips=excluded.faucet_drips, active_days=excluded.active_days, validator_joined=excluded.validator_joined,
         uptime_avg_pct=excluded.uptime_avg_pct, uptime_samples=excluded.uptime_samples, first_activity_at=excluded.first_activity_at,
         last_activity_at=excluded.last_activity_at, history_complete=excluded.history_complete, rules_version=excluded.rules_version,
         updated_at=excluded.updated_at`,
    ).bind(acct, c.tx_sent, c.deploys, c.dex_calls, c.faucet_drips, c.active_days, c.validator_joined ? 1 : 0,
      uptime.avg_pct, uptime.samples, signals.first_activity_at, signals.last_activity_at, hist.complete ? 1 : 0,
      PASSPORT_RULES.rules_version, ctx.nowIso),
  ];
  // Badges are permanent once earned (first earned_at wins).
  for (const b of badges) {
    stmts.push(env.DB.prepare(
      `INSERT OR IGNORE INTO passport_badges (account_id_hex, badge_id, track, earned_at, historic, evidence) VALUES (?, ?, ?, ?, ?, ?)`,
    ).bind(acct, b.id, b.track, b.earned_at, b.historic ? 1 : 0, b.evidence ? String(b.evidence).slice(0, 200) : null));
  }
  for (const [track, t] of Object.entries(tiers)) {
    stmts.push(env.DB.prepare(
      `INSERT INTO passport_tiers (account_id_hex, track, tier, tier_rank, score, badge_count, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)
       ON CONFLICT(account_id_hex, track) DO UPDATE SET tier=excluded.tier, tier_rank=excluded.tier_rank, score=excluded.score,
         badge_count=excluded.badge_count, updated_at=excluded.updated_at`,
    ).bind(acct, track, t.tier, tierRank(t.tier), t.score, t.badge_count, ctx.nowIso));
  }
  await env.DB.batch(stmts);
  return { account: acct, pages: hist.pages, badges: badges.length };
}

async function maybeSnapshot(env, now) {
  const week = isoWeek(now);
  const have = await env.DB.prepare('SELECT 1 FROM passport_snapshots WHERE week = ?').bind(week).first();
  if (have) return null;
  const { results } = await env.DB.prepare(
    "SELECT account_id_hex, track, tier, score FROM passport_tiers WHERE tier != 'none' ORDER BY track, tier_rank DESC, score DESC, account_id_hex",
  ).all();
  const entries = results || [];
  if (!entries.length) return null; // nothing ranked yet; retry next tick
  const leaves = [];
  for (const e of entries) leaves.push(await snapshotLeaf(e));
  const root = await merkleRoot(leaves);
  const body = { week, rules_version: PASSPORT_RULES.rules_version, created_at: now.toISOString(), merkle_root: root,
    leaf: PASSPORT_RULES.verification.snapshot.leaf, entries };
  await env.DB.prepare('INSERT OR IGNORE INTO passport_snapshots (week, merkle_root, rules_version, entries, json, created_at) VALUES (?, ?, ?, ?, ?, ?)')
    .bind(week, root, PASSPORT_RULES.rules_version, entries.length, JSON.stringify(body), now.toISOString()).run();
  return { week, root, entries: entries.length };
}

export async function runTick(env, now = new Date()) {
  const nowIso = now.toISOString();
  const rpc = env.RPC_URL || 'https://testnet-rpc.boing.network/';
  const out = { at: nowIso, rules_version: PASSPORT_RULES.rules_version, accounts: [], errors: [] };
  const ctx = { rpc, nowIso, pageBudget: num(env.PAGE_BUDGET, 36), validators: [], dex: new Set() };

  try { ctx.validators = await sampleValidators(env, rpc, nowIso); out.validators_sampled = ctx.validators.length; }
  catch (e) { out.errors.push(`validators: ${e.message}`); }
  try { ctx.dex = await fetchDexContracts(rpc); } catch (e) { out.errors.push(`dex: ${e.message}`); }

  const cutoff = new Date(now.getTime() - num(env.REFRESH_MINUTES, 60) * 60000).toISOString();
  const { results: due } = await env.DB.prepare(
    `SELECT r.account_id_hex, r.role, r.created_at FROM portal_registrations r
     LEFT JOIN passport_signals s ON s.account_id_hex = lower(r.account_id_hex)
     WHERE s.updated_at IS NULL OR s.updated_at < ?
     ORDER BY (s.updated_at IS NOT NULL), s.updated_at LIMIT ?`,
  ).bind(cutoff, num(env.ACCOUNTS_PER_TICK, 4)).all();
  for (const row of due || []) {
    if (ctx.pageBudget <= 0) break;
    try { out.accounts.push(await refreshAccount(env, ctx, row)); }
    catch (e) { out.errors.push(`${String(row.account_id_hex).slice(0, 10)}: ${e.message}`); }
  }

  // Discord tier-role sync (no-op until DISCORD_BOT_TOKEN, DISCORD_GUILD_ID and DISCORD_ROLE_* are set).
  try { out.discord = await syncDiscordRoles(env, now); } catch (e) { out.errors.push(`discord: ${e.message}`); }

  const keep = new Date(now.getTime() - num(env.UPTIME_RETENTION_DAYS, 30) * 86400000).toISOString();
  await env.DB.prepare('DELETE FROM validator_uptime_samples WHERE sampled_at < ?').bind(keep).run();
  try { out.snapshot = await maybeSnapshot(env, now); } catch (e) { out.errors.push(`snapshot: ${e.message}`); }

  await env.DB.prepare('INSERT INTO indexer_state (key, value, updated_at) VALUES (?, ?, ?) ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=excluded.updated_at')
    .bind('credibility_last_tick', JSON.stringify(out), nowIso).run();
  return out;
}

export default {
  async scheduled(_event, env, ctx) {
    ctx.waitUntil(runTick(env).then((r) => console.log(JSON.stringify(r))));
  },
  async fetch(req, env) {
    const url = new URL(req.url);
    if (url.pathname === '/tick' && req.method === 'POST') {
      // workers_dev/preview URLs are off, so this is only reachable via the service binding.
      const out = await runTick(env);
      console.log(JSON.stringify(out));
      return Response.json(out);
    }
    if (url.pathname === '/health' || url.pathname === '/') {
      const row = await env.DB.prepare("SELECT value, updated_at FROM indexer_state WHERE key = 'credibility_last_tick'").first();
      return Response.json({ ok: true, worker: 'boing-credibility-indexer', schedule: '*/15 via boing-native-dex-indexer', last_tick: row ? JSON.parse(row.value) : null },
        { headers: { 'cache-control': 'no-store' } });
    }
    return new Response('not found', { status: 404 });
  },
};
