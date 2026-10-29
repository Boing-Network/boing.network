/**
 * Method allow-list and per-IP, per-class rate limits for the public RPC edge.
 *
 * Pure logic (no Cloudflare types) so it can be unit-tested from boing-sdk/tests/publicRpcPolicy.test.ts.
 * The method table and numbers are GENERATED from ops/cloudflare/public-rpc/policy.json
 * (see methodPolicy.generated.ts); never hand-edit them.
 *
 * Modes (EDGE_POLICY_MODE var): "log" (default) never blocks, it only tags the response with
 * `X-Boing-Edge-Policy` and logs; "enforce" rejects; "off" skips everything.
 */
import {
  CLASS_LIMITS,
  MAX_BATCH_SIZE,
  METHOD_CLASS,
  type MethodClass,
  type RateClass,
} from "./methodPolicy.generated";

export type EdgeMode = "off" | "log" | "enforce";

export function parseMode(raw: string | undefined): EdgeMode {
  const v = (raw ?? "").trim().toLowerCase();
  return v === "off" || v === "enforce" ? v : "log";
}

export type Call = { method: string | null; id: unknown };
export type Inspection = { calls: Call[]; batch: boolean };

/** Parse a JSON-RPC body. Returns null when it is not JSON-RPC-shaped; the node then answers the error itself. */
export function inspectBody(text: string): Inspection | null {
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    return null;
  }
  const one = (v: unknown): Call | null => {
    if (!v || typeof v !== "object" || Array.isArray(v)) return null;
    const o = v as { method?: unknown; id?: unknown };
    return { method: typeof o.method === "string" ? o.method : null, id: o.id ?? null };
  };
  if (Array.isArray(parsed)) {
    const calls: Call[] = [];
    for (const item of parsed) {
      const c = one(item);
      if (c) calls.push(c);
    }
    return { calls, batch: true };
  }
  const c = one(parsed);
  return c ? { calls: [c], batch: false } : null;
}

export type Reason = "method_not_public" | "unknown_method" | "batch_too_large" | "rate_limited";
export type Violation = { reason: Reason; method?: string; class?: MethodClass; id: unknown };

export type Plan = { violations: Violation[]; counts: Partial<Record<RateClass, number>> };

export function classify(method: string): MethodClass | "unknown" {
  return Object.prototype.hasOwnProperty.call(METHOD_CLASS, method) ? METHOD_CLASS[method] : "unknown";
}

/** Static checks (allow-list, batch size) plus how many calls each rate class has to pay for. */
export function planRequest(inspection: Inspection): Plan {
  const violations: Violation[] = [];
  const counts: Plan["counts"] = {};
  if (inspection.calls.length > MAX_BATCH_SIZE) {
    violations.push({ reason: "batch_too_large", id: null });
  }
  for (const call of inspection.calls) {
    if (call.method === null) continue; // malformed call: the node returns -32600
    const cls = classify(call.method);
    if (cls === "unknown") {
      violations.push({ reason: "unknown_method", method: call.method, id: call.id });
    } else if (cls === "blocked") {
      violations.push({ reason: "method_not_public", method: call.method, class: cls, id: call.id });
    } else {
      counts[cls] = (counts[cls] ?? 0) + 1;
    }
  }
  return { violations, counts };
}

/** Minimal shape of the Workers Rate Limiting binding. */
export type RateLimitBinding = { limit(opts: { key: string }): Promise<{ success: boolean }> };

/**
 * Charge `counts` against the per-class bindings for `key` (the client IP). Returns the first class that
 * is over its limit, or null. A missing binding (local dev, not yet deployed) is treated as "no limit".
 */
export async function checkRates(
  env: Record<string, unknown>,
  key: string,
  counts: Plan["counts"]
): Promise<RateClass | null> {
  for (const cls of Object.keys(counts) as RateClass[]) {
    const binding = env[CLASS_LIMITS[cls].binding] as RateLimitBinding | undefined;
    if (!binding || typeof binding.limit !== "function") continue;
    const n = Math.min(counts[cls] ?? 0, MAX_BATCH_SIZE);
    for (let i = 0; i < n; i += 1) {
      const { success } = await binding.limit({ key: `${cls}:${key}` });
      if (!success) return cls;
    }
  }
  return null;
}

/** Common names that do not exist on a Boing node (seen in edge logs) and what to call instead. */
export const METHOD_HINTS: Readonly<Record<string, string>> = {
  boing_chainId: "Wallet-provider method (Boing Express), not a node RPC. On the node use boing_getNetworkInfo (chain_id).",
  eth_chainId: "Wallet-provider method, not a node RPC. On the node use boing_getNetworkInfo (chain_id).",
  boing_getTransactionByHash: "Use boing_getTransactionReceipt([tx_id]) and then boing_getBlockByHeight for the full transaction.",
  boing_getBlock: "Use boing_getBlockByHeight([height, include_receipts]) or boing_getBlockByHash([hash]).",
  boing_getBlockByNumber: "Use boing_getBlockByHeight([height, include_receipts]).",
  eth_getBlockByNumber: "Boing is not EVM. Use boing_getBlockByHeight([height, include_receipts]).",
  eth_blockNumber: "Boing is not EVM. Use boing_chainHeight.",
  boing_blockNumber: "Use boing_chainHeight.",
  boing_getMempool: "The mempool is not exposed on the public RPC. Use boing_getSyncState or boing_health.",
  boing_mempoolSize: "The mempool is not exposed on the public RPC. Use boing_getSyncState or boing_health.",
  boing_pendingTransactions: "The mempool is not exposed on the public RPC.",
};

export type Denial = { status: number; body: unknown; retryAfter?: number; note: string };

/** JSON-RPC error for a violation. -32601 for non-public methods, -32016 ("Rate limit exceeded", see RPC-API-SPEC) for limits. */
export function denial(v: Violation, batch: boolean): Denial {
  const id = batch ? null : v.id;
  if (v.reason === "rate_limited") {
    const period = v.class && v.class !== "blocked" ? CLASS_LIMITS[v.class].period : 60;
    return {
      status: 429,
      retryAfter: period,
      note: `would-deny:rate_limited:${v.class}`,
      body: {
        jsonrpc: "2.0",
        id,
        error: {
          code: -32016,
          message: `Rate limit exceeded for ${v.class} methods on the public RPC. Retry in ${period}s, or run your own node.`,
          data: { reason: "rate_limited", class: v.class, retry_after_secs: period },
        },
      },
    };
  }
  if (v.reason === "batch_too_large") {
    return {
      status: 400,
      note: "would-deny:batch_too_large",
      body: {
        jsonrpc: "2.0",
        id: null,
        error: { code: -32600, message: `Batch too large (max ${MAX_BATCH_SIZE} calls).`, data: { reason: "batch_too_large" } },
      },
    };
  }
  return {
    status: 403,
    note: `would-deny:${v.reason}:${v.method}`,
    body: {
      jsonrpc: "2.0",
      id,
      error: {
        code: -32601,
        message: `${v.method} is not available on the public RPC endpoint.`,
        data: v.method && METHOD_HINTS[v.method] ? { reason: v.reason, hint: METHOD_HINTS[v.method] } : { reason: v.reason },
      },
    },
  };
}

/** Constant-time-ish string comparison for the trusted-caller token. */
export function sameToken(a: string, b: string): boolean {
  if (a.length !== b.length || a.length === 0) return false;
  let diff = 0;
  for (let i = 0; i < a.length; i += 1) diff |= a.charCodeAt(i) ^ b.charCodeAt(i);
  return diff === 0;
}