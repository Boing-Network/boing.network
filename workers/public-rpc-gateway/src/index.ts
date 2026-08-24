/**
 * Public JSON-RPC edge for `testnet-rpc.boing.network`.
 *
 * Cloudflare Worker in front of the hosted Fly cluster: health-check `/live`,
 * fail over between backends, fan-out `boing_submitTransaction` to every origin
 * (so deploys reach the validator, not only the full node), and keep the stable
 * public hostname independent of a home Cloudflare Tunnel connector.
 */

type Env = {
  RPC_BACKENDS?: string;
};

const DEFAULT_BACKENDS = [
  "https://boing-testnet-1.fly.dev",
  "https://boing-testnet-2.fly.dev",
];

const HEALTH_TTL_MS = 8_000;
const UPSTREAM_TIMEOUT_MS = 25_000;
/** Parallel write timeout: stay under the Worker ~30s wall clock when skipping health probes. */
const SUBMIT_UPSTREAM_TIMEOUT_MS = 20_000;
const MAX_BODY_BYTES = 8 * 1_048_576;
const FAILOVER_STATUSES = new Set([502, 503, 504, 521, 522, 523, 524, 525, 526, 530]);
/** Writes that must reach the validator, not only the first healthy full node. */
const FANOUT_RPC_METHODS = new Set(["boing_submitTransaction"]);

const CORS: Record<string, string> = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Methods": "GET, HEAD, POST, OPTIONS",
  "Access-Control-Allow-Headers":
    "Content-Type, Accept, Authorization, X-Boing-Operator, User-Agent",
  "Access-Control-Max-Age": "86400",
};

type HealthSnap = { at: number; live: Map<string, boolean> };
let healthCache: HealthSnap | null = null;

function normalizeOrigin(raw: string): string {
  return raw.trim().replace(/\/$/, "");
}

function backendsFromEnv(env: Env): string[] {
  const extra = (env.RPC_BACKENDS ?? "")
    .split(",")
    .map(normalizeOrigin)
    .filter(Boolean);
  const seen = new Set<string>();
  const out: string[] = [];
  for (const u of extra.length > 0 ? extra : DEFAULT_BACKENDS) {
    if (seen.has(u)) continue;
    seen.add(u);
    out.push(u);
  }
  return out;
}

function withCors(headers: Headers): Headers {
  const next = new Headers(headers);
  for (const [k, v] of Object.entries(CORS)) next.set(k, v);
  return next;
}

function json(data: unknown, status = 200): Response {
  return new Response(JSON.stringify(data), {
    status,
    headers: withCors(new Headers({ "Content-Type": "application/json" })),
  });
}

async function probeLive(origin: string): Promise<boolean> {
  try {
    const res = await fetch(`${origin}/live`, {
      method: "GET",
      headers: { Accept: "text/plain", "User-Agent": "boing-public-rpc-gateway/1" },
      signal: AbortSignal.timeout(4_000),
    });
    return res.ok;
  } catch {
    return false;
  }
}

async function liveMap(origins: string[]): Promise<Map<string, boolean>> {
  const now = Date.now();
  if (healthCache && now - healthCache.at < HEALTH_TTL_MS) {
    return healthCache.live;
  }
  const pairs = await Promise.all(origins.map(async (o) => [o, await probeLive(o)] as const));
  const live = new Map(pairs);
  healthCache = { at: now, live };
  return live;
}

function orderedBackends(origins: string[], live: Map<string, boolean>): string[] {
  const up = origins.filter((o) => live.get(o) !== false);
  const down = origins.filter((o) => live.get(o) === false);
  return up.length > 0 ? [...up, ...down] : origins;
}

function shouldFailover(status: number): boolean {
  return FAILOVER_STATUSES.has(status);
}

function hopHeaders(req: Request): Headers {
  const headers = new Headers();
  const pass = ["content-type", "accept", "authorization", "x-boing-operator", "user-agent"];
  for (const name of pass) {
    const v = req.headers.get(name);
    if (v) headers.set(name, v);
  }
  if (!headers.has("User-Agent")) headers.set("User-Agent", "boing-public-rpc-gateway/1");
  return headers;
}

async function proxyOnce(
  origin: string,
  req: Request,
  body: ArrayBuffer | null,
  timeoutMs = UPSTREAM_TIMEOUT_MS
): Promise<Response> {
  const url = new URL(req.url);
  const target = `${origin}${url.pathname}${url.search}`;
  return fetch(target, {
    method: req.method,
    headers: hopHeaders(req),
    body: body && req.method !== "GET" && req.method !== "HEAD" ? body : undefined,
    redirect: "manual",
    signal: AbortSignal.timeout(timeoutMs),
  });
}

function jsonRpcMethod(body: ArrayBuffer | null): string | null {
  if (!body || body.byteLength === 0) return null;
  try {
    const parsed = JSON.parse(new TextDecoder().decode(body)) as { method?: unknown };
    return typeof parsed.method === "string" ? parsed.method : null;
  } catch {
    return null;
  }
}

type FanoutKind = "success" | "rpc_error" | "transport";

type FanoutAttempt = {
  origin: string;
  kind: FanoutKind;
  status: number;
  text: string;
  headers: Headers;
};

function classifyJsonRpcBody(text: string): FanoutKind | "other" {
  try {
    const parsed = JSON.parse(text) as { result?: unknown; error?: unknown };
    if (!parsed || typeof parsed !== "object") return "other";
    if (parsed.error != null) return "rpc_error";
    if ("result" in parsed) return "success";
    return "other";
  } catch {
    return "other";
  }
}

function corsJsonResponse(attempt: FanoutAttempt): Response {
  const headers = withCors(attempt.headers);
  headers.delete("content-encoding");
  headers.delete("content-length");
  if (!headers.has("Content-Type")) headers.set("Content-Type", "application/json");
  return new Response(attempt.text, { status: attempt.status, headers });
}

/**
 * POST `boing_submitTransaction` to every configured backend (validator first in
 * `RPC_BACKENDS`). Sequential failover to the full node can mempool-accept a deploy
 * that the single validator never sees, so finance reports success and explorer stays empty.
 *
 * Prefer the validator's JSON-RPC success or application error; only use another
 * backend when the validator times out or returns HTTP 5xx.
 */
async function proxySubmitFanout(origins: string[], req: Request, body: ArrayBuffer | null): Promise<Response> {
  const settled = await Promise.all(
    origins.map(async (origin): Promise<FanoutAttempt> => {
      try {
        const upstream = await proxyOnce(origin, req, body, SUBMIT_UPSTREAM_TIMEOUT_MS);
        const text = await upstream.text();
        const headers = new Headers(upstream.headers);
        if (upstream.status >= 200 && upstream.status < 300) {
          const kind = classifyJsonRpcBody(text);
          if (kind === "success" || kind === "rpc_error") {
            return { origin, kind, status: upstream.status, text, headers };
          }
        }
        return { origin, kind: "transport", status: upstream.status, text, headers };
      } catch (e) {
        const msg = e instanceof Error ? `${e.name}: ${e.message}` : "network error";
        return {
          origin,
          kind: "transport",
          status: 502,
          text: JSON.stringify({
            jsonrpc: "2.0",
            id: null,
            error: { code: -32000, message: `${origin}: ${msg}` },
          }),
          headers: new Headers({ "Content-Type": "application/json" }),
        };
      }
    })
  );

  const validator = origins[0];
  const fromValidator = settled.find((a) => a.origin === validator);
  if (fromValidator && (fromValidator.kind === "success" || fromValidator.kind === "rpc_error")) {
    return corsJsonResponse(fromValidator);
  }
  const success = settled.find((a) => a.kind === "success");
  if (success) return corsJsonResponse(success);
  const rpcError = settled.find((a) => a.kind === "rpc_error");
  if (rpcError) return corsJsonResponse(rpcError);
  return corsJsonResponse(settled[0] ?? {
    origin: validator ?? "none",
    kind: "transport",
    status: 502,
    text: JSON.stringify({
      jsonrpc: "2.0",
      id: null,
      error: { code: -32000, message: "Public RPC gateway could not reach a hosted Boing node." },
    }),
    headers: new Headers({ "Content-Type": "application/json" }),
  });
}

export default {
  async fetch(req: Request, env: Env): Promise<Response> {
    if (req.method === "OPTIONS") {
      return new Response(null, { status: 204, headers: withCors(new Headers()) });
    }

    const origins = backendsFromEnv(env);
    const url = new URL(req.url);

    if (url.pathname === "/__gateway/health") {
      const live = await liveMap(origins);
      return json({
        ok: [...live.values()].some(Boolean),
        backends: origins.map((origin) => ({ origin, live: live.get(origin) === true })),
        public_hostname: "https://testnet-rpc.boing.network/",
      });
    }

    if (req.method !== "GET" && req.method !== "HEAD" && req.method !== "POST") {
      return json({ error: "Method not allowed" }, 405);
    }

    const length = Number(req.headers.get("content-length") ?? "0");
    if (Number.isFinite(length) && length > MAX_BODY_BYTES) {
      return json({ error: "Request body too large" }, 413);
    }

    let body: ArrayBuffer | null = null;
    if (req.method === "POST") {
      body = await req.arrayBuffer();
      if (body.byteLength > MAX_BODY_BYTES) {
        return json({ error: "Request body too large" }, 413);
      }
    }

    if (req.method === "POST" && FANOUT_RPC_METHODS.has(jsonRpcMethod(body) ?? "")) {
      return proxySubmitFanout(origins, req, body);
    }

    const live = await liveMap(origins);
    const order = orderedBackends(origins, live);
    let lastStatus = 502;
    let lastText = "no healthy RPC backend";

    for (let i = 0; i < order.length; i += 1) {
      const origin = order[i];
      try {
        const upstream = await proxyOnce(origin, req, body);
        if (shouldFailover(upstream.status) && i < order.length - 1) {
          lastStatus = upstream.status;
          lastText = `backend ${origin} returned HTTP ${upstream.status}`;
          healthCache = null;
          continue;
        }
        const headers = withCors(upstream.headers);
        headers.delete("content-encoding");
        headers.delete("content-length");
        return new Response(upstream.body, { status: upstream.status, headers });
      } catch (e) {
        lastStatus = 502;
        lastText =
          e instanceof Error ? `${origin}: ${e.name} ${e.message}` : `${origin}: network error`;
        healthCache = null;
        if (i === order.length - 1) break;
      }
    }

    return json(
      {
        jsonrpc: "2.0",
        id: null,
        error: {
          code: -32000,
          message: `Public RPC gateway could not reach a hosted Boing node (${lastText}).`,
        },
      },
      lastStatus >= 400 ? lastStatus : 502
    );
  },
};
