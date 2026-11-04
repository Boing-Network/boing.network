/**
 * Discord OAuth2 helpers for Testnet Passport (stage 4).
 * Configured by Pages env: DISCORD_CLIENT_ID (var), DISCORD_CLIENT_SECRET (secret), optional DISCORD_REDIRECT_URI.
 * Scope: identify only (guild membership is checked later by the role-sync bot, not via the user's token).
 */
export const DISCORD_API = 'https://discord.com/api/v10';
export const STATE_COOKIE = 'boing_discord_state';
export const STATE_TTL_MS = 10 * 60 * 1000;

export function discordOAuthConfigured(env) {
  return !!(env.DISCORD_CLIENT_ID && env.DISCORD_CLIENT_SECRET);
}

export function redirectUri(env, request) {
  return env.DISCORD_REDIRECT_URI || `${new URL(request.url).origin}/api/passport/discord/callback`;
}

export function authorizeUrl(env, request, state) {
  const u = new URL('https://discord.com/oauth2/authorize');
  u.searchParams.set('response_type', 'code');
  u.searchParams.set('client_id', env.DISCORD_CLIENT_ID);
  u.searchParams.set('scope', 'identify');
  u.searchParams.set('state', state);
  u.searchParams.set('redirect_uri', redirectUri(env, request));
  u.searchParams.set('prompt', 'consent');
  return u.toString();
}

export function randomState() {
  const b = new Uint8Array(32);
  crypto.getRandomValues(b);
  return [...b].map((x) => x.toString(16).padStart(2, '0')).join('');
}

export function readCookie(request, name) {
  const h = request.headers.get('cookie') || '';
  for (const part of h.split(';')) {
    const [k, ...v] = part.trim().split('=');
    if (k === name) return v.join('=');
  }
  return '';
}

export function stateCookie(value, maxAgeSecs) {
  return `${STATE_COOKIE}=${value}; Path=/api/passport/discord; HttpOnly; Secure; SameSite=Lax; Max-Age=${maxAgeSecs}`;
}

/** Mask a Discord username for public display: "nicobuilds" -> "ni•••ds". */
export function maskUsername(u) {
  const s = String(u || '');
  if (s.length <= 4) return s ? `${s[0]}•••` : '';
  return `${s.slice(0, 2)}•••${s.slice(-2)}`;
}

/** Force the credibility indexer to recompute this account on its next tick (Gold depends on the link). */
export function markStale(db, account) {
  return db.prepare("UPDATE passport_signals SET updated_at = '1970-01-01T00:00:00.000Z' WHERE account_id_hex = ?").bind(account);
}
