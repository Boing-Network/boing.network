/** Faucet abuse checks: per-address cooldown, per-IP daily budget, batch refusal, success-only recording. */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import worker, { resetFaucetGuard } from "../src/index";
import {
  checkFaucet,
  createGuardStore,
  faucetGuardConfig,
  faucetKeys,
  humanWait,
  normalizeAddress,
  recordFaucet,
} from "../src/faucetGuard";

type Env = Parameters<typeof worker.fetch>[1];
const ORIGIN = "https://boing-testnet-1.fly.dev";
const A = "0x" + "ab".repeat(32);
const B = "0x" + "cd".repeat(32);
const C = "0x" + "ef".repeat(32);
const D = "0x" + "12".repeat(32);
const HOUR = 3_600_000;

describe("faucetGuard logic", () => {
  it("defaults to a 24 h address cooldown and 3 claims per IP per day; 0 disables", () => {
    expect(faucetGuardConfig({})).toEqual({ addressCooldownSecs: 86_400, ipMaxPerDay: 3 });
    expect(faucetGuardConfig({ FAUCET_ADDRESS_COOLDOWN_SECS: "3600", FAUCET_IP_MAX_PER_DAY: "0" })).toEqual({
      addressCooldownSecs: 3600,
      ipMaxPerDay: 0,
    });
    expect(faucetGuardConfig({ FAUCET_ADDRESS_COOLDOWN_SECS: "junk" }).addressCooldownSecs).toBe(86_400);
  });

  it("rejects rapid repeat claims for the same address until 24 h pass", async () => {
    const store = createGuardStore(null);
    const cfg = faucetGuardConfig({ FAUCET_IP_MAX_PER_DAY: "0" });
    const t0 = 1_800_000_000_000;
    const k = await faucetKeys("198.51.100.9", normalizeAddress(A));
    expect((await checkFaucet(store, cfg, k, t0)).ok).toBe(true);
    await recordFaucet(store, cfg, k, t0);
    for (const dt of [1_000, 60_000, 10 * 60_000, 23 * HOUR]) {
      const v = await checkFaucet(store, cfg, k, t0 + dt);
      expect(v.ok).toBe(false);
      if (!v.ok) expect(v.reason).toBe("faucet_address_cooldown");
    }
    const other = await faucetKeys("203.0.113.50", normalizeAddress(A));
    expect((await checkFaucet(store, cfg, other, t0 + HOUR)).ok, "same address from another IP").toBe(false);
    expect((await checkFaucet(store, cfg, k, t0 + 24 * HOUR)).ok).toBe(true);
  });

  it("caps fresh-address farming from one IP per rolling day", async () => {
    const store = createGuardStore(null);
    const cfg = faucetGuardConfig({});
    const t0 = 1_800_000_000_000;
    for (const [i, addr] of [A, B, C].entries()) {
      const k = await faucetKeys("198.51.100.9", normalizeAddress(addr));
      expect((await checkFaucet(store, cfg, k, t0 + i)).ok).toBe(true);
      await recordFaucet(store, cfg, k, t0 + i);
    }
    const k4 = await faucetKeys("198.51.100.9", normalizeAddress(D));
    const v = await checkFaucet(store, cfg, k4, t0 + HOUR);
    expect(v.ok).toBe(false);
    if (!v.ok) {
      expect(v.reason).toBe("faucet_ip_daily_limit");
      expect(v.retryAfter).toBe(23 * 3600);
      expect(v.message).toContain("23h 0m");
    }
    expect((await checkFaucet(store, cfg, await faucetKeys("203.0.113.50", normalizeAddress(D)), t0)).ok).toBe(true);
    expect((await checkFaucet(store, cfg, k4, t0 + 24 * HOUR)).ok).toBe(true);
  });

  it("formats waits and normalizes addresses", () => {
    expect(humanWait(86_399)).toBe("23h 59m");
    expect(humanWait(61)).toBe("1m 1s");
    expect(normalizeAddress(A.toUpperCase().replace("0X", "0x"))).toBe("ab".repeat(32));
    expect(normalizeAddress("0x1234")).toBeNull();
  });
});

describe("faucet guard through the gateway (enforce)", () => {
  let nodeOk = true;
  let posts = 0;
  beforeEach(() => {
    resetFaucetGuard();
    posts = 0;
    nodeOk = true;
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: string | URL | Request, init?: RequestInit) => {
        if (String(input) === `${ORIGIN}/live`) return new Response("ok", { status: 200 });
        if (init?.method === "POST") posts += 1;
        const body = nodeOk
          ? { jsonrpc: "2.0", id: 1, result: { ok: true, amount_boing: "20" } }
          : { jsonrpc: "2.0", id: 1, error: { code: -32000, message: "Faucet balance too low." } };
        return new Response(JSON.stringify(body), { status: 200, headers: { "Content-Type": "application/json" } });
      })
    );
  });
  afterEach(() => vi.unstubAllGlobals());

  const env = { RPC_BACKENDS: ORIGIN, EDGE_POLICY_MODE: "enforce" } as Env;
  const ask = (addr: string, ip = "198.51.100.9", batch = false) => {
    const call = { jsonrpc: "2.0", id: 7, method: "boing_faucetRequest", params: [addr] };
    return worker.fetch(
      new Request("https://testnet-rpc.boing.network/", {
        method: "POST",
        headers: { "Content-Type": "application/json", "cf-connecting-ip": ip },
        body: JSON.stringify(batch ? [call] : call),
      }),
      env
    );
  };

  it("lets the first claim through and refuses a repeat with a clear 429", async () => {
    expect((await ask(A)).status).toBe(200);
    const res = await ask(A);
    expect(res.status).toBe(429);
    expect(Number(res.headers.get("Retry-After"))).toBeGreaterThan(86_000);
    const j = (await res.json()) as { id: unknown; error: { code: number; message: string; data: { reason: string } } };
    expect(j.id).toBe(7);
    expect(j.error.code).toBe(-32016);
    expect(j.error.data.reason).toBe("faucet_address_cooldown");
    // Ceiling can land on 24h 0m or 23h 59m depending on ms timing.
    expect(j.error.message).toMatch(/Try again in 2[34]h \d+m/);
    expect(posts).toBe(1);
  });

  it("does not record failed claims, so a node error never locks the user out", async () => {
    nodeOk = false;
    expect((await ask(A)).status).toBe(200);
    nodeOk = true;
    expect((await ask(A)).status).toBe(200);
    expect(posts).toBe(2);
  });

  it("limits one IP to 3 fresh addresses a day", async () => {
    for (const a of [A, B, C]) expect((await ask(a)).status).toBe(200);
    const res = await ask(D);
    expect(res.status).toBe(429);
    expect(((await res.json()) as { error: { data: { reason: string } } }).error.data.reason).toBe("faucet_ip_daily_limit");
    expect((await ask(D, "203.0.113.50")).status).toBe(200);
  });

  it("refuses faucet requests inside a batch", async () => {
    const res = await ask(A, "198.51.100.9", true);
    expect(res.status).toBe(400);
    expect(posts).toBe(0);
  });

  it("log mode only tags", async () => {
    const logEnv = { RPC_BACKENDS: ORIGIN, EDGE_POLICY_MODE: "log" } as Env;
    const req = () =>
      worker.fetch(
        new Request("https://testnet-rpc.boing.network/", {
          method: "POST",
          headers: { "Content-Type": "application/json", "cf-connecting-ip": "198.51.100.77" },
          body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "boing_faucetRequest", params: [A] }),
        }),
        logEnv
      );
    expect((await req()).status).toBe(200);
    const second = await req();
    expect(second.status).toBe(200);
    expect(second.headers.get("X-Boing-Edge-Policy")).toBe("would-deny:faucet_address_cooldown");
  });
});
