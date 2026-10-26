/**
 * Public JSON-RPC edge for `testnet-rpc.boing.network`.
 *
 * Cloudflare Worker in front of the hosted Fly validator: health-check `/live`,
 * proxy JSON-RPC, fan-out `boing_submitTransaction` to every configured origin,
 * and keep the stable public hostname independent of a home Cloudflare Tunnel.
 */

import {
  checkRates,
  denial,
  inspectBody,
  parseMode,
  planRequest,
  sameToken,
  type Denial,
  type Violation,
} from "./edgePolicy";

type Env = {
  RPC_BACKENDS?: string;
  /** "log" (default) | "enforce" | "off": see edgePolicy.ts and docs/PUBLIC-RPC-WAF-GUIDANCE.md. */
  EDGE_POLICY_MODE?: string;
  /** Optional secret: callers presenting it in `X-Boing-Edge-Trusted` skip rate limits (never the blocklist). */
  EDGE_TRUSTED_TOKEN?: string;
  [binding: string]: unknown;
};

const DEFAULT_BACKENDS = ["https://boing-testnet-1.fly.dev"];

const HEALTH_TTL_MS = 8_000;
const UPSTREAM_TIMEOUT_MS = 25_000;
/** Parallel write timeout: stay under the Worker ~30s wall clock when skipping health probes. */
const SUBMIT_UPSTREAM_TIMEOUT_MS = 20_000;
const MAX_BODY_BYTES = 8 * 1_048_576;
/**
 * Cap JSON-RPC response bodies forwarded to clients. Fly can return 15–300 MiB for tip
 * blocks whose ContractDeploy bytecode is serde'd as JSON number arrays; streaming those
 * through the Worker often truncates mid-array (HTTP 200 + invalid JSON), which breaks
 * explorers (`Expected ',' or ']' after array element…`).
 */
const MAX_RESPONSE_BYTES = 6 * 1_048_576;
const FAILOVER_STATUSES = new Set([502, 503, 504, 521, 522, 523, 524, 525, 526, 530]);
/** Writes that must reach the validator, not only the first healthy full node. */
const FANOUT_RPC_METHODS = new Set(["boing_submitTransaction"]);

const CORS: Record<string, string> = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Methods": "GET, HEAD, POST, OPTIONS",
  "Access-Control-Allow-Headers":
    "Content-Type, Accept, Authorization, X-Boing-Operator, X-Boing-Blob-Download, X-Boing-Edge-Trusted, User-Agent",
  "Access-Control-Expose-Headers": "X-Boing-Edge-Policy, Retry-After",
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

/**
 * Read upstream body up to `maxBytes`; return null if the cap would be exceeded.
 * Copies each stream chunk immediately — Workers may reuse the underlying buffer across
 * reads, and holding views until the end intermittently corrupted even small JSON-RPC
 * bodies (false “invalid JSON” rejects for methods like boing_getSyncState).
 */
async function readBodyCapped(res: Response, maxBytes: number): Promise<Uint8Array | null> {
  const declared = Number(res.headers.get("content-length") ?? "NaN");
  if (Number.isFinite(declared) && declared > maxBytes) {
    await res.body?.cancel().catch(() => undefined);
    return null;
  }

  // Fast path: known small Content-Length — one-shot arrayBuffer (no stream view reuse).
  if (Number.isFinite(declared) && declared >= 0 && declared <= maxBytes) {
    const buf = new Uint8Array(await res.arrayBuffer());
    if (buf.byteLength > maxBytes) return null;
    return buf;
  }

  if (!res.body) return new Uint8Array(0);
  const reader = res.body.getReader();
  const chunks: Uint8Array[] = [];
  let total = 0;
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      if (!value || value.byteLength === 0) continue;
      total += value.byteLength;
      if (total > maxBytes) {
        await reader.cancel().catch(() => undefined);
        return null;
      }
      // Must copy: the stream may reuse `value`'s backing memory on the next read.
      chunks.push(value.slice());
    }
  } catch {
    await reader.cancel().catch(() => undefined);
    throw new Error("upstream body read failed");
  }
  const out = new Uint8Array(total);
  let offset = 0;
  for (const c of chunks) {
    out.set(c, offset);
    offset += c.byteLength;
  }
  return out;
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
 * `RPC_BACKENDS`). With multiple backends, sequential read failover alone can
 * mempool-accept a deploy that the validator never sees.
 *
 * Prefer the validator's JSON-RPC success or application error; only use another
 * backend when the validator times out or returns HTTP 5xx.
 */
/** JSON-RPC error body for the node's duplicate-signed-payload rejection (`data.reason = already_in_mempool`). */
export function isAlreadyInMempoolBody(text: string): boolean {
  try {
    const v = JSON.parse(text) as { error?: { message?: unknown; data?: { reason?: unknown } } };
    const err = v?.error;
    if (!err) return false;
    if (err.data && err.data.reason === "already_in_mempool") return true;
    return typeof err.message === "string" && /duplicate transaction/i.test(err.message);
  } catch {
    return false;
  }
}

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
  // Fan-out race: a full node accepts the tx and gossips it to the validator before the validator's own copy
  // arrives, so the validator says "Duplicate transaction" (already_in_mempool) although the tx WAS accepted.
  // Prefer any backend's success in that case instead of surfacing a misleading failure.
  if (fromValidator && fromValidator.kind === "rpc_error" && isAlreadyInMempoolBody(fromValidator.text)) {
    const ok = settled.find((a) => a.kind === "success");
    if (ok) return corsJsonResponse(ok);
  }
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

type Note = { value: string | null };

/**
 * Method allow-list and per-IP class limits. Returns a Response only in enforce mode; in log mode it
 * records what would have been denied in `note` and lets the request through.
 */
async function edgeGate(req: Request, env: Env, body: ArrayBuffer | null, note: Note): Promise<Response | null> {
  const mode = parseMode(env.EDGE_POLICY_MODE);
  if (mode === "off" || req.method !== "POST" || !body) return null;
  const inspection = inspectBody(new TextDecoder().decode(body));
  if (!inspection) return null;
  const plan = planRequest(inspection);
  let first: Violation | undefined = plan.violations[0];
  if (!first) {
    const trusted = env.EDGE_TRUSTED_TOKEN ? sameToken(req.headers.get("x-boing-edge-trusted") ?? "", env.EDGE_TRUSTED_TOKEN) : false;
    if (!trusted) {
      const ip = req.headers.get("cf-connecting-ip") ?? "unknown";
      const over = await checkRates(env, ip, plan.counts);
      if (over) first = { reason: "rate_limited", class: over, id: inspection.calls[0]?.id ?? null };
    }
  }
  if (!first) return null;
  const d: Denial = denial(first, inspection.batch);
  console.warn(JSON.stringify({ edge_policy: mode, denial: d.note, ip: req.headers.get("cf-connecting-ip") }));
  if (mode === "log") {
    note.value = d.note;
    return null;
  }
  const headers = withCors(new Headers({ "Content-Type": "application/json", "X-Boing-Edge-Policy": d.note.replace("would-deny", "denied") }));
  if (d.retryAfter) headers.set("Retry-After", String(d.retryAfter));
  return new Response(JSON.stringify(d.body), { status: d.status, headers });
}

export default {
  async fetch(req: Request, env: Env): Promise<Response> {
    const note: Note = { value: null };
    const res = await handle(req, env, note);
    if (!note.value) return res;
    const headers = new Headers(res.headers);
    headers.set("X-Boing-Edge-Policy", note.value);
    return new Response(res.body, { status: res.status, headers });
  },
};

async function handle(req: Request, env: Env, note: Note): Promise<Response> {
  {
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

    const denied = await edgeGate(req, env, body, note);
    if (denied) return denied;

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

        const declaredLength = Number(upstream.headers.get("content-length") ?? "NaN");
        if (Number.isFinite(declaredLength) && declaredLength > MAX_RESPONSE_BYTES) {
          await upstream.body?.cancel().catch(() => undefined);
          return json(
            {
              jsonrpc: "2.0",
              id: null,
              error: {
                code: -32000,
                message: `Upstream RPC response too large (${declaredLength} bytes; gateway limit ${MAX_RESPONSE_BYTES}).`,
              },
            },
            200
          );
        }

        // Buffer (with cap) so we never forward a mid-stream truncation as HTTP 200 JSON.
        const buf = await readBodyCapped(upstream, MAX_RESPONSE_BYTES);
        if (buf === null) {
          return json(
            {
              jsonrpc: "2.0",
              id: null,
              error: {
                code: -32000,
                message: `Upstream RPC response exceeded gateway limit (${MAX_RESPONSE_BYTES} bytes).`,
              },
            },
            200
          );
        }
        if (
          Number.isFinite(declaredLength) &&
          declaredLength > 0 &&
          buf.byteLength < declaredLength
        ) {
          return json(
            {
              jsonrpc: "2.0",
              id: null,
              error: {
                code: -32000,
                message: `Upstream RPC response truncated (${buf.byteLength} of ${declaredLength} bytes).`,
              },
            },
            200
          );
        }

        // Only reject when the body is non-empty yet not JSON. Empty 502/530 pages from
        // the edge still failover above; valid small JSON-RPC must never be false-rejected.
        if (req.method === "POST" && buf.byteLength > 0) {
          const text = new TextDecoder().decode(buf);
          try {
            JSON.parse(text);
          } catch {
            return json(
              {
                jsonrpc: "2.0",
                id: null,
                error: {
                  code: -32000,
                  message:
                    "Upstream RPC returned truncated or invalid JSON (not forwarding corrupt body).",
                },
              },
              200
            );
          }
        }

        const headers = withCors(upstream.headers);
        headers.delete("content-encoding");
        headers.delete("content-length");
        if (!headers.has("Content-Type")) headers.set("Content-Type", "application/json");
        return new Response(buf, { status: upstream.status, headers });
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
  }
}
