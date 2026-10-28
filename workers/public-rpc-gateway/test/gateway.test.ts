/**
 * Smoke tests for the public RPC gateway Worker (src/index.ts), driven through its `fetch` handler with a
 * stubbed upstream. They pin the edge-policy decisions in each mode:
 *   - log:     never blocks; would-be denials are forwarded and tagged `X-Boing-Edge-Policy: would-deny:...`
 *   - enforce: refuses operator/unknown methods (403), oversized batches (400), rate-limited classes (429)
 *   - off:     skips the policy entirely
 * Pure policy logic (classification, generated tables, Cloudflare rules) is covered in boing-sdk/tests/publicRpcPolicy.test.ts.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import worker from "../src/index";
import { CLASS_LIMITS, MAX_BATCH_SIZE } from "../src/methodPolicy.generated";

type Env = Parameters<typeof worker.fetch>[1];

const ORIGIN = "https://boing-testnet-1.fly.dev";
const OK_BODY = JSON.stringify({ jsonrpc: "2.0", id: 1, result: "ok" });

let upstreamPosts: string[] = [];

beforeEach(() => {
  upstreamPosts = [];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: string | URL | Request, init?: RequestInit) => {
      const url = String(input);
      if (url === `${ORIGIN}/live`) return new Response("ok", { status: 200 });
      if (init?.method === "POST") upstreamPosts.push(String(init.body ? new TextDecoder().decode(init.body as ArrayBuffer) : ""));
      return new Response(OK_BODY, { status: 200, headers: { "Content-Type": "application/json" } });
    })
  );
});

afterEach(() => {
  vi.unstubAllGlobals();
});

const rpc = (body: unknown, headers: Record<string, string> = {}) =>
  new Request("https://testnet-rpc.boing.network/", {
    method: "POST",
    headers: { "Content-Type": "application/json", "cf-connecting-ip": "203.0.113.7", ...headers },
    body: typeof body === "string" ? body : JSON.stringify(body),
  });

const call = (method: string, id: number | string = 1) => ({ jsonrpc: "2.0", id, method, params: [] });
const bigBatch = () => Array.from({ length: MAX_BATCH_SIZE + 1 }, (_, i) => call("boing_health", i));

const env = (mode?: string, extra: Record<string, unknown> = {}): Env => ({ RPC_BACKENDS: ORIGIN, EDGE_POLICY_MODE: mode, ...extra });

describe("gateway in log mode (default): never blocks, tags would-deny", () => {
  it("forwards an allowed read untouched, with no policy header", async () => {
    const res = await worker.fetch(rpc(call("boing_getBalance")), env("log"));
    expect(res.status).toBe(200);
    expect(res.headers.get("X-Boing-Edge-Policy")).toBeNull();
    expect(upstreamPosts).toHaveLength(1);
  });

  it("defaults to log when the mode is unset or unknown", async () => {
    for (const mode of [undefined, "", "nonsense"]) {
      const res = await worker.fetch(rpc(call("boing_operatorApplyFeePolicy")), env(mode));
      expect(res.status).toBe(200);
      expect(res.headers.get("X-Boing-Edge-Policy")).toBe("would-deny:method_not_public:boing_operatorApplyFeePolicy");
    }
  });

  it("forwards but tags an operator method as would-deny", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const res = await worker.fetch(rpc(call("boing_operatorApplyFeePolicy")), env("log"));
    expect(res.status).toBe(200);
    expect(res.headers.get("X-Boing-Edge-Policy")).toBe("would-deny:method_not_public:boing_operatorApplyFeePolicy");
    expect(upstreamPosts).toHaveLength(1);
    const line = JSON.parse(String(warn.mock.calls[0]?.[0]));
    expect(line).toMatchObject({ edge_policy: "log", reason: "method_not_public", class: "blocked", method: "boing_operatorApplyFeePolicy", ip: "203.0.113.7" });
    warn.mockRestore();
  });

  it("tags unknown methods and oversized batches but still forwards them", async () => {
    const unknown = await worker.fetch(rpc(call("eth_sendRawTransaction")), env("log"));
    expect(unknown.status).toBe(200);
    expect(unknown.headers.get("X-Boing-Edge-Policy")).toBe("would-deny:unknown_method:eth_sendRawTransaction");

    const batch = await worker.fetch(rpc(bigBatch()), env("log"));
    expect(batch.status).toBe(200);
    expect(batch.headers.get("X-Boing-Edge-Policy")).toBe("would-deny:batch_too_large");
    expect(upstreamPosts).toHaveLength(2);
  });

  it("accepts a batch of exactly the maximum size", async () => {
    const batch = Array.from({ length: MAX_BATCH_SIZE }, (_, i) => call("boing_health", i));
    const res = await worker.fetch(rpc(batch), env("log"));
    expect(res.headers.get("X-Boing-Edge-Policy")).toBeNull();
  });
});

describe("gateway in enforce mode: refuses before the request reaches the node", () => {
  it("returns 403 / -32601 for operator methods, keeping the request id, and never contacts the node", async () => {
    const res = await worker.fetch(rpc(call("boing_operatorApplyFeePolicy", 7)), env("enforce"));
    expect(res.status).toBe(403);
    expect(res.headers.get("X-Boing-Edge-Policy")).toBe("denied:method_not_public:boing_operatorApplyFeePolicy");
    const body = (await res.json()) as { id: number; error: { code: number } };
    expect(body.id).toBe(7);
    expect(body.error.code).toBe(-32601);
    expect(upstreamPosts).toHaveLength(0);
  });

  it("returns 403 for unknown methods", async () => {
    const res = await worker.fetch(rpc(call("admin_nodeInfo")), env("enforce"));
    expect(res.status).toBe(403);
    expect(upstreamPosts).toHaveLength(0);
  });

  it("returns 400 / -32600 for batches over the cap", async () => {
    const res = await worker.fetch(rpc(bigBatch()), env("enforce"));
    expect(res.status).toBe(400);
    const body = (await res.json()) as { error: { code: number } };
    expect(body.error.code).toBe(-32600);
    expect(upstreamPosts).toHaveLength(0);
  });

  it("still serves allowed methods and malformed bodies (the node answers those)", async () => {
    expect((await worker.fetch(rpc(call("boing_getBalance")), env("enforce"))).status).toBe(200);
    expect((await worker.fetch(rpc("not json"), env("enforce"))).status).toBe(200);
    expect(upstreamPosts).toHaveLength(2);
  });

  it("returns 429 / -32016 with Retry-After when a class is over its limit, charging the client IP", async () => {
    const keys: string[] = [];
    const binding = { limit: async ({ key }: { key: string }) => (keys.push(key), { success: false }) };
    const res = await worker.fetch(rpc(call("boing_getBalance")), env("enforce", { [CLASS_LIMITS.read.binding]: binding }));
    expect(res.status).toBe(429);
    expect(res.headers.get("Retry-After")).toBe(String(CLASS_LIMITS.read.period));
    expect(((await res.json()) as { error: { code: number } }).error.code).toBe(-32016);
    expect(keys).toEqual(["read:203.0.113.7"]);
    expect(upstreamPosts).toHaveLength(0);
  });

  it("lets a trusted caller skip rate limits but never the blocklist", async () => {
    const binding = { limit: async () => ({ success: false }) };
    const e = env("enforce", { [CLASS_LIMITS.read.binding]: binding, EDGE_TRUSTED_TOKEN: "s3cret" });
    const trusted = { "x-boing-edge-trusted": "s3cret" };
    expect((await worker.fetch(rpc(call("boing_getBalance"), trusted), e)).status).toBe(200);
    expect((await worker.fetch(rpc(call("boing_getBalance"), { "x-boing-edge-trusted": "wrong" }), e)).status).toBe(429);
    expect((await worker.fetch(rpc(call("boing_operatorApplyFeePolicy"), trusted), e)).status).toBe(403);
  });
});

describe("gateway with the policy off, and non-RPC routes", () => {
  it("skips every check when the mode is off", async () => {
    const res = await worker.fetch(rpc(call("boing_operatorApplyFeePolicy")), env("off"));
    expect(res.status).toBe(200);
    expect(res.headers.get("X-Boing-Edge-Policy")).toBeNull();
  });

  it("answers CORS preflight with 204 and exposes the policy header", async () => {
    const res = await worker.fetch(new Request("https://testnet-rpc.boing.network/", { method: "OPTIONS" }), env("enforce"));
    expect(res.status).toBe(204);
    expect(res.headers.get("Access-Control-Expose-Headers")).toContain("X-Boing-Edge-Policy");
  });

  it("rejects methods other than GET, HEAD, POST and OPTIONS", async () => {
    const res = await worker.fetch(new Request("https://testnet-rpc.boing.network/", { method: "PUT", body: "{}" }), env("enforce"));
    expect(res.status).toBe(405);
  });

  it("serves /__gateway/health from the backend /live probe", async () => {
    const res = await worker.fetch(new Request("https://testnet-rpc.boing.network/__gateway/health"), env("enforce"));
    expect(res.status).toBe(200);
    const body = (await res.json()) as { ok: boolean; backends: Array<{ origin: string; live: boolean }> };
    expect(body.ok).toBe(true);
    expect(body.backends).toEqual([{ origin: ORIGIN, live: true }]);
  });

  it("does not apply the method policy to GET requests", async () => {
    const res = await worker.fetch(new Request("https://testnet-rpc.boing.network/live", { method: "GET" }), env("enforce"));
    expect(res.status).toBe(200);
    expect(res.headers.get("X-Boing-Edge-Policy")).toBeNull();
  });
});
