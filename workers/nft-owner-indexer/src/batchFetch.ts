/**
 * Fetch blocks (with receipts) using JSON-RPC batch requests: up to `batchSize` heights per HTTP call
 * (the node caps batches at 32, BOING_RPC_MAX_BATCH). One subrequest per batch instead of per block keeps
 * a wide catch-up window under the Free-plan 50-subrequest cap and well below the public gateway's
 * per-IP request rate limit.
 *
 * Oversize handling follows `oversize.ts`: if the gateway refuses a whole batch as over its response cap,
 * that batch is retried block by block and only the individually oversize heights are skipped.
 */
import { isOversizeRejection } from './oversize.js';

export type FetchedBlock = { height: number; block: unknown; oversize: string | null };

export type RpcPost = (body: unknown) => Promise<unknown>;

type RpcEntry = { id?: unknown; result?: unknown; error?: { message?: unknown } | null };

function errMessage(e: unknown): string {
  if (e && typeof e === 'object' && typeof (e as { message?: unknown }).message === 'string') {
    return (e as { message: string }).message;
  }
  return String(e);
}

export const MAX_RPC_BATCH = 32;

export async function fetchBlocksBatched(
  post: RpcPost,
  heights: readonly number[],
  options: {
    batchSize?: number;
    omitMissing?: boolean;
    /** Single-height fetch used after a batch-level oversize rejection. */
    single: (height: number) => Promise<unknown>;
  }
): Promise<FetchedBlock[]> {
  const size = Math.max(1, Math.min(MAX_RPC_BATCH, options.batchSize ?? MAX_RPC_BATCH));
  const out: FetchedBlock[] = [];
  const missing = (h: number) => {
    if (!options.omitMissing) throw new Error(`missing block at height ${h}`);
  };

  for (let i = 0; i < heights.length; i += size) {
    const chunk = heights.slice(i, i + size);
    const body = chunk.map((h, id) => ({
      jsonrpc: '2.0',
      id,
      method: 'boing_getBlockByHeight',
      params: [h, true],
    }));
    let res: unknown;
    try {
      res = await post(body);
    } catch (e) {
      if (!isOversizeRejection(e)) throw e;
      res = { error: { message: errMessage(e) } };
    }

    if (!Array.isArray(res)) {
      const err = (res as RpcEntry | null)?.error;
      if (err && isOversizeRejection(err)) {
        // The batch as a whole is too big: go height by height, skipping only the oversize ones.
        for (const h of chunk) {
          try {
            const block = await options.single(h);
            if (block == null) missing(h);
            else out.push({ height: h, block, oversize: null });
          } catch (e) {
            if (!isOversizeRejection(e)) throw e;
            out.push({ height: h, block: null, oversize: errMessage(e).slice(0, 200) });
          }
        }
        continue;
      }
      throw new Error(`RPC batch failed: ${err ? errMessage(err) : 'non-array response'}`);
    }

    const byId = new Map<number, RpcEntry>();
    for (const entry of res as RpcEntry[]) {
      if (entry && typeof entry.id === 'number') byId.set(entry.id, entry);
    }
    chunk.forEach((h, id) => {
      const entry = byId.get(id);
      if (!entry) throw new Error(`RPC batch response missing height ${h}`);
      if (entry.error) {
        if (isOversizeRejection(entry.error)) {
          out.push({ height: h, block: null, oversize: errMessage(entry.error).slice(0, 200) });
          return;
        }
        throw new Error(`RPC error at height ${h}: ${errMessage(entry.error)}`);
      }
      if (entry.result == null) missing(h);
      else out.push({ height: h, block: entry.result, oversize: null });
    });
  }
  return out;
}

/** JSON-RPC POST helper for `fetchBlocksBatched` (throws on HTTP errors). */
export function jsonRpcPoster(rpcUrl: string, timeoutMs = 30_000): RpcPost {
  return async (body) => {
    const r = await fetch(rpcUrl, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(body),
      signal: AbortSignal.timeout(timeoutMs),
    });
    const text = await r.text();
    let parsed: unknown;
    try {
      parsed = JSON.parse(text);
    } catch {
      throw new Error(`RPC HTTP ${r.status}: non-JSON response`);
    }
    if (!r.ok && !(parsed && typeof parsed === 'object' && 'error' in (parsed as object))) {
      throw new Error(`RPC HTTP ${r.status}`);
    }
    return parsed;
  };
}
