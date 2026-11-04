/**
 * Discord role sync for Testnet Passport tiers (stage 4).
 * No-ops unless DISCORD_BOT_TOKEN (secret) and DISCORD_GUILD_ID are set and at least one DISCORD_ROLE_* var maps a tier.
 *
 * Role config (Wrangler vars on boing-credibility-indexer, values are Discord role ids):
 *   DISCORD_ROLE_BRONZE / _SILVER / _GOLD            -> highest tier across all tracks (only the highest is held)
 *   DISCORD_ROLE_USER_BRONZE ... _NODE_OPERATOR_GOLD -> optional per-track roles (only the highest per track is held)
 * Only roles this sync assigned (recorded in discord_links.roles_synced) are ever removed; other member roles are untouched.
 * Rate limits: at most DISCORD_MAX_CALLS (default 8) role calls per tick, sequential; any 429 stops the sync until next tick.
 */
import { tierRank } from '../../../website/functions/api/portal/_lib/passportScore.js';

const API = 'https://discord.com/api/v10';
const TIERS = ['bronze', 'silver', 'gold'];
const TRACKS = ['user', 'developer', 'node_operator'];
const SNOWFLAKE = /^\d{17,20}$/;

export function roleConfig(env) {
  const overall = {};
  const perTrack = {};
  for (const t of TIERS) {
    const id = String(env[`DISCORD_ROLE_${t.toUpperCase()}`] || '').trim();
    if (SNOWFLAKE.test(id)) overall[t] = id;
    for (const tr of TRACKS) {
      const v = String(env[`DISCORD_ROLE_${tr.toUpperCase()}_${t.toUpperCase()}`] || '').trim();
      if (SNOWFLAKE.test(v)) (perTrack[tr] ||= {})[t] = v;
    }
  }
  const all = new Set([...Object.values(overall), ...Object.values(perTrack).flatMap((m) => Object.values(m))]);
  return { overall, perTrack, all };
}

export function discordSyncEnabled(env) {
  return !!(env.DISCORD_BOT_TOKEN && SNOWFLAKE.test(String(env.DISCORD_GUILD_ID || '').trim()) && roleConfig(env).all.size);
}

/** Desired managed role ids for one account given its passport_tiers rows. Pure. */
export function desiredRoles(tierRows, cfg) {
  const want = new Set();
  let best = 'none';
  for (const r of tierRows) {
    if (tierRank(r.tier) > tierRank(best)) best = r.tier;
    const m = cfg.perTrack[r.track];
    if (m && m[r.tier]) want.add(m[r.tier]);
  }
  if (cfg.overall[best]) want.add(cfg.overall[best]);
  return want;
}

/** Plan add/remove calls. Pure. `synced` = roles we previously assigned. */
export function planRoleChanges(want, synced, cfg) {
  const add = [...want].filter((r) => !synced.has(r));
  const remove = [...synced].filter((r) => !want.has(r) && cfg.all.has(r));
  // Roles we assigned that are no longer configured at all: drop from our record without calling Discord.
  const forget = [...synced].filter((r) => !cfg.all.has(r));
  return { add, remove, forget };
}

async function roleCall(env, method, discordId, roleId) {
  const res = await fetch(`${API}/guilds/${env.DISCORD_GUILD_ID}/members/${discordId}/roles/${roleId}`, {
    method,
    headers: { Authorization: `Bot ${env.DISCORD_BOT_TOKEN}`, 'X-Audit-Log-Reason': 'Boing Testnet Passport tier sync' },
  });
  if (res.status === 204 || res.ok) return { ok: true };
  let body = {};
  try { body = await res.json(); } catch (_) {}
  if (res.status === 429) return { ok: false, rateLimited: true, retryAfter: Number(body.retry_after || res.headers.get('retry-after') || 5) };
  // 10007 Unknown Member, 10011 Unknown Role, 50013 Missing Permissions (role hierarchy / Manage Roles)
  return { ok: false, status: res.status, code: body.code || null };
}

export async function syncDiscordRoles(env, now = new Date()) {
  if (!discordSyncEnabled(env)) return { enabled: false };
  const cfg = roleConfig(env);
  const nowIso = now.toISOString();
  let budget = Number(env.DISCORD_MAX_CALLS) > 0 ? Number(env.DISCORD_MAX_CALLS) : 8;
  const out = { enabled: true, calls: 0, added: 0, removed: 0, errors: [] };
  const spend = () => { budget--; out.calls++; };

  // 1) Unlinked accounts: remove the roles we had assigned.
  const { results: removals } = await env.DB.prepare('SELECT discord_id, role_ids FROM discord_role_removals ORDER BY queued_at LIMIT 5').all();
  for (const r of removals || []) {
    let left = JSON.parse(r.role_ids || '[]').filter((id) => cfg.all.has(id));
    while (left.length && budget > 0) {
      spend();
      const res = await roleCall(env, 'DELETE', r.discord_id, left[0]);
      if (res.rateLimited) { out.rate_limited = res.retryAfter; break; }
      if (!res.ok && ![404].includes(res.status)) out.errors.push(`remove ${r.discord_id.slice(-4)}: ${res.status}/${res.code}`);
      out.removed += res.ok ? 1 : 0;
      left = left.slice(1);
    }
    if (out.rate_limited) break;
    if (!left.length) await env.DB.prepare('DELETE FROM discord_role_removals WHERE discord_id = ?').bind(r.discord_id).run();
    else await env.DB.prepare('UPDATE discord_role_removals SET role_ids = ? WHERE discord_id = ?').bind(JSON.stringify(left), r.discord_id).run();
    if (budget <= 0) break;
  }

  // 2) Linked accounts, least-recently synced first. Members not in the guild are retried every 6 h.
  if (!out.rate_limited && budget > 0) {
    const retryNotInGuild = new Date(now.getTime() - 6 * 3600000).toISOString();
    const { results: links } = await env.DB.prepare(
      `SELECT account_id_hex, discord_id, roles_synced, in_guild, last_synced_at FROM discord_links
       WHERE in_guild != 0 OR last_synced_at IS NULL OR last_synced_at < ?
       ORDER BY (last_synced_at IS NOT NULL), last_synced_at LIMIT 25`,
    ).bind(retryNotInGuild).all();
    for (const l of links || []) {
      if (budget <= 0 || out.rate_limited) break;
      const { results: tierRows } = await env.DB.prepare('SELECT track, tier FROM passport_tiers WHERE account_id_hex = ?').bind(l.account_id_hex).all();
      const synced = new Set(JSON.parse(l.roles_synced || '[]'));
      const plan = planRoleChanges(desiredRoles(tierRows || [], cfg), synced, cfg);
      for (const f of plan.forget) synced.delete(f);
      let inGuild = l.in_guild === 0 ? 0 : 1;
      let err = null;
      let completed = true;
      const ops = [...plan.add.map((r) => ['PUT', r]), ...plan.remove.map((r) => ['DELETE', r])];
      for (const [method, roleId] of ops) {
        if (budget <= 0) { completed = false; break; }
        spend();
        const res = await roleCall(env, method, l.discord_id, roleId);
        if (res.rateLimited) { out.rate_limited = res.retryAfter; completed = false; break; }
        if (res.ok) {
          if (method === 'PUT') { synced.add(roleId); out.added++; } else { synced.delete(roleId); out.removed++; }
          inGuild = 1;
        } else if (res.code === 10007) { inGuild = 0; err = 'not_in_guild'; break; }
        else if (method === 'DELETE' && res.code === 10011) { synced.delete(roleId); }
        else { err = res.code === 50013 ? 'missing_permissions' : `http_${res.status}_${res.code}`; out.errors.push(`${l.discord_id.slice(-4)}: ${err}`); break; }
      }
      await env.DB.prepare('UPDATE discord_links SET roles_synced = ?, in_guild = ?, sync_error = ?, last_synced_at = ? WHERE account_id_hex = ?')
        .bind(JSON.stringify([...synced]), inGuild, err, completed ? nowIso : l.last_synced_at, l.account_id_hex).run();
    }
  }
  return out;
}
