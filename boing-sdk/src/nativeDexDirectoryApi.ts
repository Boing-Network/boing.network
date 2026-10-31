/**
 * HTTP JSON for the Cloudflare Worker `workers/native-dex-indexer` D1 directory
 * (`GET /v1/directory/meta`, `GET /v1/directory/pools`). Not Boing JSON-RPC.
 */

import { validateHex32 } from './hex.js';
import type { NativeDexIndexerPoolRow } from './nativeDexIndexerStats.js';
import type { NativeDexMaterializedPoolEvent } from './nativeDexPoolHistory.js';

/** `api` field value on successful directory responses. */
export const NATIVE_DEX_DIRECTORY_API_ID = 'boing-native-dex-directory/v1' as const;

/** Bumped when D1 schema or meta semantics change in a breaking way for clients. */
export const NATIVE_DEX_DIRECTORY_SCHEMA_VERSION = 2 as const;

export type NativeDexDirectoryMetaResponse = {
  api: string;
  schemaVersion?: number;
  poolCount: number;
  /** Row count in `directory_pool_events` when the Worker migration is applied. */
  eventCount?: number;
  latestSyncBatch: string | null;
  /** Chain height used for the last event snapshot (migration `0003+`). */
  indexedTipHeight?: number | null;
  /** Block hash at `indexedTipHeight` when the node returned it — for skew checks only; not a reorg rewind signal. */
  indexedTipBlockHash?: string | null;
  /** `header.parent_hash` at the indexed tip block when available. */
  indexedParentBlockHash?: string | null;
  nftOwnerRowCount?: number;
  /** Rows in `directory_receipt_log` when migration `0006` is applied and receipt archiving is enabled. */
  receiptLogCount?: number;
};

export type NativeDexDirectoryPoolsPageResponse = {
  api: string;
  limit: number;
  cursor: string | null;
  nextCursor: string | null;
  hasMore: boolean;
  pools: NativeDexIndexerPoolRow[];
};

export type NativeDexDirectoryPoolEventsPageResponse = {
  api: string;
  poolHex: string;
  limit: number;
  cursor: string | null;
  nextCursor: string | null;
  hasMore: boolean;
  events: NativeDexMaterializedPoolEvent[];
};

export type NativeDexDirectoryUserEventsPageResponse = {
  api: string;
  callerHex: string;
  limit: number;
  cursor: string | null;
  nextCursor: string | null;
  hasMore: boolean;
  events: NativeDexMaterializedPoolEvent[];
};

export class NativeDexDirectoryHttpError extends Error {
  constructor(
    message: string,
    readonly status: number,
    readonly url: string,
    readonly bodySnippet?: string,
  ) {
    super(message);
    this.name = 'NativeDexDirectoryHttpError';
  }
}

/** Trim and strip a trailing slash (no path segment — base is worker origin). */
export function normalizeNativeDexDirectoryWorkerBaseUrl(raw: string): string {
  return String(raw || '')
    .trim()
    .replace(/\/+$/, '');
}

function isPlainObject(v: unknown): v is Record<string, unknown> {
  return v !== null && typeof v === 'object' && !Array.isArray(v);
}

function tryParseIndexerPoolRow(o: unknown): NativeDexIndexerPoolRow | null {
  if (!isPlainObject(o)) return null;
  try {
    validateHex32(String(o.poolHex));
    validateHex32(String(o.tokenAHex));
    validateHex32(String(o.tokenBHex));
  } catch {
    return null;
  }
  return o as NativeDexIndexerPoolRow;
}

/**
 * Parse `GET /v1/directory/meta` JSON. Returns `null` if shape is invalid.
 */
export function parseNativeDexDirectoryMetaResponse(data: unknown): NativeDexDirectoryMetaResponse | null {
  if (!isPlainObject(data)) return null;
  const api = data.api;
  if (api !== NATIVE_DEX_DIRECTORY_API_ID) return null;
  const schemaVersion = data.schemaVersion;
  if (schemaVersion != null && (typeof schemaVersion !== 'number' || !Number.isFinite(schemaVersion) || schemaVersion < 1)) {
    return null;
  }
  const poolCount = data.poolCount;
  if (typeof poolCount !== 'number' || !Number.isFinite(poolCount) || poolCount < 0) return null;
  const latestSyncBatch = data.latestSyncBatch;
  if (latestSyncBatch != null && typeof latestSyncBatch !== 'string') return null;
  const eventCount = data.eventCount;
  if (eventCount != null && (typeof eventCount !== 'number' || !Number.isFinite(eventCount) || eventCount < 0)) {
    return null;
  }
  const indexedTipHeightRaw = data.indexedTipHeight;
  if (indexedTipHeightRaw != null) {
    if (typeof indexedTipHeightRaw !== 'number' || !Number.isFinite(indexedTipHeightRaw) || indexedTipHeightRaw < 0) {
      return null;
    }
  }
  const indexedTipBlockHashRaw = data.indexedTipBlockHash;
  if (indexedTipBlockHashRaw != null && typeof indexedTipBlockHashRaw !== 'string') return null;
  if (
    typeof indexedTipBlockHashRaw === 'string' &&
    indexedTipBlockHashRaw !== '' &&
    !/^0x[0-9a-f]{64}$/i.test(indexedTipBlockHashRaw)
  ) {
    return null;
  }

  const out: NativeDexDirectoryMetaResponse = {
    api,
    poolCount,
    latestSyncBatch: latestSyncBatch ?? null,
  };
  if (eventCount != null) out.eventCount = eventCount;
  if (data.indexedTipHeight !== undefined) {
    out.indexedTipHeight = indexedTipHeightRaw === null || indexedTipHeightRaw === undefined ? null : indexedTipHeightRaw;
  }
  if (indexedTipBlockHashRaw !== undefined) {
    out.indexedTipBlockHash =
      indexedTipBlockHashRaw == null || indexedTipBlockHashRaw === ''
        ? null
        : indexedTipBlockHashRaw.toLowerCase();
  }
  const indexedParentBlockHashRaw = data.indexedParentBlockHash;
  if (indexedParentBlockHashRaw != null && typeof indexedParentBlockHashRaw !== 'string') return null;
  if (
    typeof indexedParentBlockHashRaw === 'string' &&
    indexedParentBlockHashRaw !== '' &&
    !/^0x[0-9a-f]{64}$/i.test(indexedParentBlockHashRaw)
  ) {
    return null;
  }
  if (indexedParentBlockHashRaw !== undefined) {
    out.indexedParentBlockHash =
      indexedParentBlockHashRaw == null || indexedParentBlockHashRaw === ''
        ? null
        : indexedParentBlockHashRaw.toLowerCase();
  }
  const nftOwnerRowCount = data.nftOwnerRowCount;
  if (nftOwnerRowCount != null && (typeof nftOwnerRowCount !== 'number' || !Number.isFinite(nftOwnerRowCount) || nftOwnerRowCount < 0)) {
    return null;
  }
  if (schemaVersion != null) out.schemaVersion = schemaVersion;
  if (nftOwnerRowCount != null) out.nftOwnerRowCount = nftOwnerRowCount;
  const receiptLogCount = data.receiptLogCount;
  if (receiptLogCount != null && (typeof receiptLogCount !== 'number' || !Number.isFinite(receiptLogCount) || receiptLogCount < 0)) {
    return null;
  }
  if (receiptLogCount != null) out.receiptLogCount = receiptLogCount;
  return out;
}

/**
 * Parse `GET /v1/directory/pools` JSON. Drops pool objects that fail minimal hex validation.
 */
export function parseNativeDexDirectoryPoolsPageResponse(data: unknown): NativeDexDirectoryPoolsPageResponse | null {
  if (!isPlainObject(data)) return null;
  const api = data.api;
  if (api !== NATIVE_DEX_DIRECTORY_API_ID) return null;
  const limit = data.limit;
  if (typeof limit !== 'number' || !Number.isFinite(limit) || limit < 1) return null;
  const cursor = data.cursor;
  if (cursor != null && typeof cursor !== 'string') return null;
  const nextCursor = data.nextCursor;
  if (nextCursor != null && typeof nextCursor !== 'string') return null;
  const hasMore = data.hasMore;
  if (typeof hasMore !== 'boolean') return null;
  const rawPools = data.pools;
  if (!Array.isArray(rawPools)) return null;
  const pools: NativeDexIndexerPoolRow[] = [];
  for (const row of rawPools) {
    const p = tryParseIndexerPoolRow(row);
    if (p) pools.push(p);
  }
  return {
    api,
    limit,
    cursor: cursor ?? null,
    nextCursor: nextCursor ?? null,
    hasMore,
    pools,
  };
}

const NATIVE_AMM_LOG2_KINDS = new Set(['swap', 'addLiquidity', 'removeLiquidity']);

function tryParseMaterializedPoolEvent(o: unknown): NativeDexMaterializedPoolEvent | null {
  if (!isPlainObject(o)) return null;
  const kind = o.kind;
  if (typeof kind !== 'string' || !NATIVE_AMM_LOG2_KINDS.has(kind)) return null;
  const poolHex = String(o.poolHex || '').trim().toLowerCase();
  if (!/^0x[0-9a-f]{64}$/.test(poolHex)) return null;
  const blockHeight = o.blockHeight;
  if (typeof blockHeight !== 'number' || !Number.isFinite(blockHeight)) return null;
  const txId = o.txId;
  if (typeof txId !== 'string' || !txId.trim()) return null;
  const logIndex = o.logIndex;
  if (typeof logIndex !== 'number' || !Number.isFinite(logIndex)) return null;
  const callerHex = String(o.callerHex || '').trim().toLowerCase();
  if (!/^0x[0-9a-f]{64}$/.test(callerHex)) return null;
  const blockHashRaw = o.blockHash;
  let blockHash: string | null = null;
  if (blockHashRaw !== undefined && blockHashRaw !== null) {
    if (typeof blockHashRaw !== 'string' || (blockHashRaw !== '' && !/^0x[0-9a-f]{64}$/i.test(blockHashRaw))) {
      return null;
    }
    blockHash = blockHashRaw === '' ? null : blockHashRaw.toLowerCase();
  }
  const payload = o.payload;
  if (!isPlainObject(payload)) return null;
  const payloadOut: Record<string, string> = {};
  for (const [k, v] of Object.entries(payload)) {
    if (typeof v !== 'string') return null;
    payloadOut[k] = v;
  }
  return {
    kind: kind as NativeDexMaterializedPoolEvent['kind'],
    poolHex,
    blockHeight: Math.floor(blockHeight),
    blockHash,
    txId: txId.trim(),
    logIndex: Math.floor(logIndex),
    callerHex,
    payload: payloadOut,
  };
}

/**
 * Parse `GET /v1/history/pool/{pool}/events` JSON. Drops malformed event objects.
 */
export function parseNativeDexDirectoryPoolEventsPageResponse(
  data: unknown,
): NativeDexDirectoryPoolEventsPageResponse | null {
  if (!isPlainObject(data)) return null;
  const api = data.api;
  if (api !== NATIVE_DEX_DIRECTORY_API_ID) return null;
  const poolHex = String(data.poolHex || '').trim().toLowerCase();
  if (!/^0x[0-9a-f]{64}$/.test(poolHex)) return null;
  const limit = data.limit;
  if (typeof limit !== 'number' || !Number.isFinite(limit) || limit < 1) return null;
  const cursor = data.cursor;
  if (cursor != null && typeof cursor !== 'string') return null;
  const nextCursor = data.nextCursor;
  if (nextCursor != null && typeof nextCursor !== 'string') return null;
  const hasMore = data.hasMore;
  if (typeof hasMore !== 'boolean') return null;
  const rawEvents = data.events;
  if (!Array.isArray(rawEvents)) return null;
  const events: NativeDexMaterializedPoolEvent[] = [];
  for (const row of rawEvents) {
    const e = tryParseMaterializedPoolEvent(row);
    if (e) events.push(e);
  }
  return {
    api,
    poolHex,
    limit,
    cursor: cursor ?? null,
    nextCursor: nextCursor ?? null,
    hasMore,
    events,
  };
}

/**
 * Parse `GET /v1/history/user/{caller}/events` JSON. Drops malformed event objects.
 */
export function parseNativeDexDirectoryUserEventsPageResponse(
  data: unknown,
): NativeDexDirectoryUserEventsPageResponse | null {
  if (!isPlainObject(data)) return null;
  const api = data.api;
  if (api !== NATIVE_DEX_DIRECTORY_API_ID) return null;
  const callerHex = String(data.callerHex || '').trim().toLowerCase();
  if (!/^0x[0-9a-f]{64}$/.test(callerHex)) return null;
  const limit = data.limit;
  if (typeof limit !== 'number' || !Number.isFinite(limit) || limit < 1) return null;
  const cursor = data.cursor;
  if (cursor != null && typeof cursor !== 'string') return null;
  const nextCursor = data.nextCursor;
  if (nextCursor != null && typeof nextCursor !== 'string') return null;
  const hasMore = data.hasMore;
  if (typeof hasMore !== 'boolean') return null;
  const rawEvents = data.events;
  if (!Array.isArray(rawEvents)) return null;
  const events: NativeDexMaterializedPoolEvent[] = [];
  for (const row of rawEvents) {
    const e = tryParseMaterializedPoolEvent(row);
    if (e) events.push(e);
  }
  return {
    api,
    callerHex,
    limit,
    cursor: cursor ?? null,
    nextCursor: nextCursor ?? null,
    hasMore,
    events,
  };
}

async function readJsonOrThrow(url: string, res: Response): Promise<unknown> {
  const text = await res.text();
  let data: unknown;
  try {
    data = JSON.parse(text) as unknown;
  } catch {
    throw new NativeDexDirectoryHttpError(`Invalid JSON from directory worker`, res.status, url, text.slice(0, 200));
  }
  if (!res.ok) {
    const errMsg =
      isPlainObject(data) && typeof data.error === 'string' ? data.error : `HTTP ${res.status}`;
    throw new NativeDexDirectoryHttpError(errMsg, res.status, url, text.slice(0, 200));
  }
  return data;
}

/**
 * `GET {baseUrl}/v1/directory/meta`
 */

/** Default fetch timeout for directory worker helpers (matches BoingClient default). */
export const DEFAULT_DIRECTORY_FETCH_TIMEOUT_MS = 30_000;

function mergeDirectoryFetchInit(init?: RequestInit, timeoutMs = DEFAULT_DIRECTORY_FETCH_TIMEOUT_MS): RequestInit {
  if (init?.signal != null || timeoutMs <= 0) {
    return { ...init, method: 'GET', headers: { Accept: 'application/json', ...init?.headers } };
  }
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  const signal = controller.signal;
  // Clear timer when aborted by caller or timeout — best-effort via signal event.
  signal.addEventListener('abort', () => clearTimeout(timer), { once: true });
  return {
    ...init,
    method: 'GET',
    signal,
    headers: { Accept: 'application/json', ...init?.headers },
  };
}

export async function fetchNativeDexDirectoryMeta(
  baseUrl: string,
  init?: RequestInit,
): Promise<NativeDexDirectoryMetaResponse> {
  const root = normalizeNativeDexDirectoryWorkerBaseUrl(baseUrl);
  const url = `${root}/v1/directory/meta`;
  const res = await fetch(url, mergeDirectoryFetchInit(init));
  const data = await readJsonOrThrow(url, res);
  const parsed = parseNativeDexDirectoryMetaResponse(data);
  if (!parsed) {
    throw new NativeDexDirectoryHttpError('Unexpected /v1/directory/meta JSON shape', res.status, url);
  }
  return parsed;
}

export type FetchNativeDexDirectoryPoolsPageQuery = {
  /** 1–100; default 20 on server if omitted — we pass explicit default 20 when unset. */
  limit?: number;
  cursor?: string | null;
};

/**
 * `GET {baseUrl}/v1/directory/pools?limit=&cursor=`
 */
export async function fetchNativeDexDirectoryPoolsPage(
  baseUrl: string,
  query: FetchNativeDexDirectoryPoolsPageQuery = {},
  init?: RequestInit,
): Promise<NativeDexDirectoryPoolsPageResponse> {
  const root = normalizeNativeDexDirectoryWorkerBaseUrl(baseUrl);
  const u = new URL(`${root}/v1/directory/pools`);
  if (query.limit != null) u.searchParams.set('limit', String(query.limit));
  if (query.cursor != null && query.cursor !== '') u.searchParams.set('cursor', query.cursor);
  const url = u.toString();
  const res = await fetch(url, mergeDirectoryFetchInit(init));
  const data = await readJsonOrThrow(url, res);
  const parsed = parseNativeDexDirectoryPoolsPageResponse(data);
  if (!parsed) {
    throw new NativeDexDirectoryHttpError('Unexpected /v1/directory/pools JSON shape', res.status, url);
  }
  return parsed;
}

export type FetchNativeDexDirectoryPoolEventsPageQuery = {
  limit?: number;
  /** Row `id` cursor from prior `nextCursor` (newest-first pages). */
  cursor?: string | null;
};

/**
 * `GET {baseUrl}/v1/history/pool/{poolHex}/events?limit=&cursor=`
 */
export async function fetchNativeDexDirectoryPoolEventsPage(
  baseUrl: string,
  poolHex32: string,
  query: FetchNativeDexDirectoryPoolEventsPageQuery = {},
  init?: RequestInit,
): Promise<NativeDexDirectoryPoolEventsPageResponse> {
  const root = normalizeNativeDexDirectoryWorkerBaseUrl(baseUrl);
  const pool = validateHex32(String(poolHex32).trim()).toLowerCase();
  const u = new URL(`${root}/v1/history/pool/${pool}/events`);
  if (query.limit != null) u.searchParams.set('limit', String(query.limit));
  if (query.cursor != null && query.cursor !== '') u.searchParams.set('cursor', query.cursor);
  const url = u.toString();
  const res = await fetch(url, mergeDirectoryFetchInit(init));
  const data = await readJsonOrThrow(url, res);
  const parsed = parseNativeDexDirectoryPoolEventsPageResponse(data);
  if (!parsed) {
    throw new NativeDexDirectoryHttpError('Unexpected pool events JSON shape', res.status, url);
  }
  return parsed;
}

export type FetchNativeDexDirectoryUserEventsPageQuery = {
  limit?: number;
  cursor?: string | null;
};

/**
 * `GET {baseUrl}/v1/history/user/{callerHex}/events?limit=&cursor=`
 */
export async function fetchNativeDexDirectoryUserEventsPage(
  baseUrl: string,
  callerHex32: string,
  query: FetchNativeDexDirectoryUserEventsPageQuery = {},
  init?: RequestInit,
): Promise<NativeDexDirectoryUserEventsPageResponse> {
  const root = normalizeNativeDexDirectoryWorkerBaseUrl(baseUrl);
  const caller = validateHex32(String(callerHex32).trim()).toLowerCase();
  const u = new URL(`${root}/v1/history/user/${caller}/events`);
  if (query.limit != null) u.searchParams.set('limit', String(query.limit));
  if (query.cursor != null && query.cursor !== '') u.searchParams.set('cursor', query.cursor);
  const url = u.toString();
  const res = await fetch(url, mergeDirectoryFetchInit(init));
  const data = await readJsonOrThrow(url, res);
  const parsed = parseNativeDexDirectoryUserEventsPageResponse(data);
  if (!parsed) {
    throw new NativeDexDirectoryHttpError('Unexpected user pool events JSON shape', res.status, url);
  }
  return parsed;
}

export type CollectNativeDexDirectoryPoolsOptions = {
  /** Per request `limit` (1–100). Default 100. */
  pageLimit?: number;
  /** Stop after this many pools (truncates last page). Default unlimited. */
  maxPools?: number;
  /** Safety cap on HTTP pages. Default 10_000. */
  maxPages?: number;
  init?: RequestInit;
};

/**
 * Walk cursor pages until `hasMore` is false or limits hit.
 */
export async function collectAllNativeDexDirectoryPools(
  baseUrl: string,
  opts: CollectNativeDexDirectoryPoolsOptions = {},
): Promise<NativeDexIndexerPoolRow[]> {
  const pageLimit = Math.min(100, Math.max(1, opts.pageLimit ?? 100));
  const maxPools = opts.maxPools ?? Number.POSITIVE_INFINITY;
  const maxPages = opts.maxPages ?? 10_000;
  const out: NativeDexIndexerPoolRow[] = [];
  let cursor: string | null | undefined;

  for (let page = 0; page < maxPages; page++) {
    const pageRes = await fetchNativeDexDirectoryPoolsPage(
      baseUrl,
      { limit: pageLimit, cursor: cursor ?? null },
      opts.init,
    );
    for (const p of pageRes.pools) {
      out.push(p);
      if (out.length >= maxPools) return out.slice(0, maxPools);
    }
    if (!pageRes.hasMore || pageRes.nextCursor == null || pageRes.nextCursor === '') break;
    cursor = pageRes.nextCursor;
  }
  return out;
}
