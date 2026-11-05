/**
 * Edge abuse checks for `boing_faucetRequest` (added Nov 2026 after scripted faucet farming).
 *
 * On top of the per-IP RL_FAUCET binding (short burst limit) this enforces:
 *   - a per-address cooldown (FAUCET_ADDRESS_COOLDOWN_SECS, default 86400 = 24 h), and
 *   - a per-IP daily budget (FAUCET_IP_MAX_PER_DAY, default 3 successful claims per rolling 24 h).
 * Set either to 0 to disable it. Only successful claims are recorded, so a node error never locks a user out.
 *
 * State lives in the Cloudflare Cache API (free, no daily write quota, unlike KV) plus an in-isolate map.
 * The Cache API is per data centre, so this is a strong deterrent rather than a global guarantee; the node
 * keeps its own per-address cooldown (BOING_FAUCET_COOLDOWN_SECS) as the authoritative check.
 * IPs and addresses are stored only as SHA-256 hashes.
 */

export type FaucetGuardConfig = { addressCooldownSecs: number; ipMaxPerDay: number };

const DAY_SECS = 86_400;

function nonNegInt(raw: unknown, fallback: number): number {
  if (typeof raw !== "string" || raw.trim() === "") return fallback;
  const n = Number.parseInt(raw.trim(), 10);
  return Number.isFinite(n) && n >= 0 ? n : fallback;
}

export function faucetGuardConfig(env: Record<string, unknown>): FaucetGuardConfig {
  return {
    addressCooldownSecs: nonNegInt(env.FAUCET_ADDRESS_COOLDOWN_SECS, DAY_SECS),
    ipMaxPerDay: nonNegInt(env.FAUCET_IP_MAX_PER_DAY, 3),
  };
}

/** Timestamps (ms) of successful claims, newest last. */
export interface GuardStore {
  get(key: string): Promise<number[] | null>;
  put(key: string, value: number[], ttlSecs: number): Promise<void>;
}

/** In-isolate map, optionally backed by the Cache API. Never throws. */
export function createGuardStore(cache: Cache | null): GuardStore {
  const local = new Map<string, { v: number[]; exp: number }>();
  const url = (key: string) => `https://faucet-guard.boing-gateway.invalid/${key}`;
  return {
    async get(key) {
      const hit = local.get(key);
      if (hit && hit.exp > Date.now()) return hit.v;
      if (!cache) return null;
      try {
        const res = await cache.match(url(key));
        if (!res) return null;
        const v = (await res.json()) as unknown;
        return Array.isArray(v) ? v.filter((x): x is number => typeof x === "number") : null;
      } catch {
        return null;
      }
    },
    async put(key, value, ttlSecs) {
      if (local.size > 20_000) local.clear();
      local.set(key, { v: value, exp: Date.now() + ttlSecs * 1000 });
      if (!cache) return;
      try {
        await cache.put(
          url(key),
          new Response(JSON.stringify(value), { headers: { "Cache-Control": `max-age=${Math.max(1, ttlSecs)}` } })
        );
      } catch {
        /* cache unavailable: in-isolate state still applies */
      }
    },
  };
}

async function sha256Hex(s: string): Promise<string> {
  const d = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(s));
  return [...new Uint8Array(d)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

export function normalizeAddress(raw: unknown): string | null {
  if (typeof raw !== "string") return null;
  const h = raw.trim().toLowerCase().replace(/^0x/, "");
  return /^[0-9a-f]{64}$/.test(h) ? h : null;
}

export type FaucetKeys = { ip: string; address: string | null };

export async function faucetKeys(ip: string, address: string | null): Promise<FaucetKeys> {
  return {
    ip: `ip/${await sha256Hex(`faucet-ip:${ip}`)}`,
    address: address ? `addr/${await sha256Hex(`faucet-addr:${address}`)}` : null,
  };
}

export type FaucetVerdict =
  | { ok: true }
  | { ok: false; reason: "faucet_address_cooldown" | "faucet_ip_daily_limit"; retryAfter: number; message: string };

export function humanWait(secs: number): string {
  const s = Math.max(1, Math.floor(secs));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m ${s % 60}s`;
  return `${s}s`;
}

export async function checkFaucet(
  store: GuardStore,
  cfg: FaucetGuardConfig,
  keys: FaucetKeys,
  now = Date.now()
): Promise<FaucetVerdict> {
  if (cfg.addressCooldownSecs > 0 && keys.address) {
    const last = (await store.get(keys.address))?.at(-1);
    if (last !== undefined) {
      const left = (last + cfg.addressCooldownSecs * 1000 - now) / 1000;
      if (left > 0) {
        return {
          ok: false,
          reason: "faucet_address_cooldown",
          retryAfter: Math.ceil(left),
          message: `Faucet cooldown: this address already received testnet BOING. Try again in ${humanWait(left)}.`,
        };
      }
    }
  }
  if (cfg.ipMaxPerDay > 0) {
    const recent = ((await store.get(keys.ip)) ?? []).filter((t) => now - t < DAY_SECS * 1000);
    if (recent.length >= cfg.ipMaxPerDay) {
      const left = (recent[0] + DAY_SECS * 1000 - now) / 1000;
      return {
        ok: false,
        reason: "faucet_ip_daily_limit",
        retryAfter: Math.ceil(left),
        message: `Faucet limit reached: at most ${cfg.ipMaxPerDay} faucet requests per network per day. Try again in ${humanWait(left)}.`,
      };
    }
  }
  return { ok: true };
}

export async function recordFaucet(store: GuardStore, cfg: FaucetGuardConfig, keys: FaucetKeys, now = Date.now()): Promise<void> {
  if (cfg.addressCooldownSecs > 0 && keys.address) {
    await store.put(keys.address, [now], cfg.addressCooldownSecs);
  }
  if (cfg.ipMaxPerDay > 0) {
    const recent = ((await store.get(keys.ip)) ?? []).filter((t) => now - t < DAY_SECS * 1000);
    recent.push(now);
    await store.put(keys.ip, recent.slice(-Math.max(cfg.ipMaxPerDay, 1)), DAY_SECS);
  }
}

/** True when the faucet node response reports a successful dispense. */
export function isFaucetSuccess(text: string): boolean {
  try {
    const o = JSON.parse(text) as { result?: { ok?: unknown } };
    return !!o && typeof o === "object" && o.result?.ok === true;
  } catch {
    return false;
  }
}
