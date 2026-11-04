/**
 * Testnet Passport scoring (stage 2): signals -> badges -> tiers, plus the weekly snapshot Merkle root.
 * Pure functions shared by the `boing-credibility-indexer` Worker and the Pages API. No I/O here.
 */
import { PASSPORT_RULES } from './quests.js';

const lc = (s) => String(s || '').toLowerCase();
export const TIER_ORDER = ['none', 'bronze', 'silver', 'gold'];
export const tierRank = (t) => Math.max(0, TIER_ORDER.indexOf(t));
const tsIso = (sec) => (sec ? new Date(Number(sec) * 1000).toISOString() : null);

function calledContracts(tx) {
  const p = tx.payload || {};
  if (p.kind === 'contract_call' && p.contract) return [lc(p.contract)];
  if (p.kind === 'batch' && Array.isArray(p.steps)) return p.steps.filter((s) => s && s.contract).map((s) => lc(s.contract));
  return [];
}

/**
 * Walk the account's tx history oldest-first and record, per signal, the count and the
 * timestamp at which each count was reached (so badges get a real earned_at).
 */
export function computeSignals(account, txs, dex, validators) {
  const acct = lc(account);
  const faucet = lc(PASSPORT_RULES.verification.faucet_account_hex);
  const ordered = [...(txs || [])].filter((t) => t && t.success).sort((a, b) => (a.block_height - b.block_height) || (a.tx_index - b.tx_index));
  const hits = { tx_sent: [], faucet_drips: [], dex_calls: [], deploys: [], active_days: [], validator_joined: [] };
  const days = new Set();
  for (const t of ordered) {
    const sent = lc(t.sender) === acct;
    const kind = t.payload?.kind || '';
    const at = t.timestamp || null;
    const ev = { at, tx: t.tx_id, height: t.block_height };
    if (lc(t.sender) === faucet && kind === 'transfer' && lc(t.payload.to) === acct) hits.faucet_drips.push(ev);
    if (!sent) continue;
    hits.tx_sent.push(ev);
    if (at) {
      const day = new Date(at * 1000).toISOString().slice(0, 10);
      if (!days.has(day)) { days.add(day); hits.active_days.push(ev); }
    }
    if (kind.startsWith('contract_deploy')) hits.deploys.push(ev);
    if (calledContracts(t).some((c) => dex && dex.has(c))) hits.dex_calls.push(ev);
    if (kind === 'validator_register' && hits.validator_joined.length === 0) hits.validator_joined.push(ev);
  }
  const v = (validators || []).find((x) => lc(x.id) === acct || lc(x.owner) === acct);
  if (v && hits.validator_joined.length === 0) hits.validator_joined.push({ at: null, tx: null, evidence: `validator ${v.id}` });
  const counts = Object.fromEntries(Object.entries(hits).map(([k, arr]) => [k, arr.length]));
  const first = ordered.find((t) => lc(t.sender) === acct);
  const last = [...ordered].reverse().find((t) => lc(t.sender) === acct);
  return {
    counts,
    hits,
    validator: v ? { id: v.id, status: v.status, in_set: !!v.in_set } : null,
    first_activity_at: tsIso(first?.timestamp),
    last_activity_at: tsIso(last?.timestamp),
  };
}

/**
 * @param {object} p
 * @param {ReturnType<typeof computeSignals>} p.signals
 * @param {Record<string,string>} p.verifiedQuests quest_id -> verified_at ISO
 * @param {{avg_pct:number|null, samples:number}} p.uptime
 * @param {string|null} p.registeredAt ISO
 * @param {string} p.nowIso
 */
export function computeBadges({ signals, verifiedQuests = {}, uptime = { avg_pct: null, samples: 0 }, registeredAt = null, nowIso }) {
  const out = [];
  for (const b of PASSPORT_RULES.badges) {
    const r = b.rule;
    let earned = null;
    if (r.quest) {
      if (verifiedQuests[r.quest]) earned = { at: verifiedQuests[r.quest], evidence: `quest ${r.quest}` };
    } else if (r.signal === 'uptime') {
      if (uptime.samples >= r.min_samples && uptime.avg_pct != null && uptime.avg_pct >= r.min_pct) {
        earned = { at: nowIso, evidence: `avg ${uptime.avg_pct.toFixed(2)}% over ${uptime.samples} samples` };
      }
    } else {
      const ev = signals.hits[r.signal]?.[r.gte - 1];
      if (ev) earned = { at: tsIso(ev.at) || nowIso, evidence: ev.tx || ev.evidence || null };
    }
    if (earned) {
      out.push({
        id: b.id,
        track: b.track,
        points: b.points,
        earned_at: earned.at,
        evidence: earned.evidence,
        historic: !!(registeredAt && earned.at && earned.at < registeredAt),
      });
    }
  }
  return out;
}

/** Tier per track from earned badge ids. verifiedSocial gates Gold (stage 4: Discord OAuth). */
export function computeTiers(badges, tracks, { verifiedSocial = false } = {}) {
  const res = {};
  for (const track of tracks) {
    const spec = PASSPORT_RULES.tiers[track];
    if (!spec) continue;
    const mine = badges.filter((b) => b.track === track || b.track === 'all');
    const ids = new Set(mine.map((b) => b.id));
    const score = mine.reduce((s, b) => s + (b.points || 0), 0);
    let tier = 'none';
    for (const t of ['bronze', 'silver', 'gold']) {
      const req = spec[t];
      const ok = mine.length >= req.min_badges && req.requires.every((id) => ids.has(id)) && (!req.requires_verified_social || verifiedSocial);
      if (!ok) break;
      tier = t;
    }
    res[track] = { tier, score, badge_count: mine.length };
  }
  return res;
}

/** Progress toward the next tier for display. */
export function nextTierProgress(track, badges, currentTier) {
  const spec = PASSPORT_RULES.tiers[track];
  const next = TIER_ORDER[tierRank(currentTier) + 1];
  if (!spec || !next || !spec[next]) return null;
  const mine = badges.filter((b) => b.track === track || b.track === 'all');
  const ids = new Set(mine.map((b) => b.id));
  const req = spec[next];
  return {
    next,
    have_badges: mine.length,
    need_badges: req.min_badges,
    missing_required: req.requires.filter((id) => !ids.has(id)),
    needs_verified_social: !!req.requires_verified_social,
  };
}

async function sha256Hex(str) {
  const buf = await crypto.subtle.digest('SHA-256', typeof str === 'string' ? new TextEncoder().encode(str) : str);
  return [...new Uint8Array(buf)].map((b) => b.toString(16).padStart(2, '0')).join('');
}
const hexBytes = (h) => Uint8Array.from(h.match(/../g).map((x) => parseInt(x, 16)));

export async function snapshotLeaf(e) {
  return sha256Hex(`${lc(e.account_id_hex)}|${e.track}|${e.tier}|${e.score}`);
}

/** Merkle root over sorted leaf hashes; odd node is promoted unchanged. */
export async function merkleRoot(leaves) {
  let level = [...leaves].sort();
  if (level.length === 0) return null;
  while (level.length > 1) {
    const next = [];
    for (let i = 0; i < level.length; i += 2) {
      if (i + 1 === level.length) next.push(level[i]);
      else next.push(await sha256Hex(new Uint8Array([...hexBytes(level[i]), ...hexBytes(level[i + 1])])));
    }
    level = next;
  }
  return '0x' + level[0];
}

/** ISO week id (e.g. 2026-W45) for a Date, UTC. */
export function isoWeek(d) {
  const t = new Date(Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate()));
  const day = t.getUTCDay() || 7;
  t.setUTCDate(t.getUTCDate() + 4 - day);
  const y0 = new Date(Date.UTC(t.getUTCFullYear(), 0, 1));
  const w = Math.ceil(((t - y0) / 86400000 + 1) / 7);
  return `${t.getUTCFullYear()}-W${String(w).padStart(2, '0')}`;
}

export function shortHex(h) {
  const s = String(h || '');
  return s.length > 16 ? `${s.slice(0, 8)}…${s.slice(-6)}` : s;
}
