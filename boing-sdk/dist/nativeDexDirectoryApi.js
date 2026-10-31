/**
 * HTTP JSON for the Cloudflare Worker `workers/native-dex-indexer` D1 directory
 * (`GET /v1/directory/meta`, `GET /v1/directory/pools`). Not Boing JSON-RPC.
 */
import { validateHex32 } from './hex.js';
/** `api` field value on successful directory responses. */
export const NATIVE_DEX_DIRECTORY_API_ID = 'boing-native-dex-directory/v1';
/** Bumped when D1 schema or meta semantics change in a breaking way for clients. */
export const NATIVE_DEX_DIRECTORY_SCHEMA_VERSION = 2;
export class NativeDexDirectoryHttpError extends Error {
    constructor(message, status, url, bodySnippet) {
        super(message);
        this.status = status;
        this.url = url;
        this.bodySnippet = bodySnippet;
        this.name = 'NativeDexDirectoryHttpError';
    }
}
/** Trim and strip a trailing slash (no path segment — base is worker origin). */
export function normalizeNativeDexDirectoryWorkerBaseUrl(raw) {
    return String(raw || '')
        .trim()
        .replace(/\/+$/, '');
}
function isPlainObject(v) {
    return v !== null && typeof v === 'object' && !Array.isArray(v);
}
function tryParseIndexerPoolRow(o) {
    if (!isPlainObject(o))
        return null;
    try {
        validateHex32(String(o.poolHex));
        validateHex32(String(o.tokenAHex));
        validateHex32(String(o.tokenBHex));
    }
    catch {
        return null;
    }
    return o;
}
/**
 * Parse `GET /v1/directory/meta` JSON. Returns `null` if shape is invalid.
 */
export function parseNativeDexDirectoryMetaResponse(data) {
    if (!isPlainObject(data))
        return null;
    const api = data.api;
    if (api !== NATIVE_DEX_DIRECTORY_API_ID)
        return null;
    const schemaVersion = data.schemaVersion;
    if (schemaVersion != null && (typeof schemaVersion !== 'number' || !Number.isFinite(schemaVersion) || schemaVersion < 1)) {
        return null;
    }
    const poolCount = data.poolCount;
    if (typeof poolCount !== 'number' || !Number.isFinite(poolCount) || poolCount < 0)
        return null;
    const latestSyncBatch = data.latestSyncBatch;
    if (latestSyncBatch != null && typeof latestSyncBatch !== 'string')
        return null;
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
    if (indexedTipBlockHashRaw != null && typeof indexedTipBlockHashRaw !== 'string')
        return null;
    if (typeof indexedTipBlockHashRaw === 'string' &&
        indexedTipBlockHashRaw !== '' &&
        !/^0x[0-9a-f]{64}$/i.test(indexedTipBlockHashRaw)) {
        return null;
    }
    const out = {
        api,
        poolCount,
        latestSyncBatch: latestSyncBatch ?? null,
    };
    if (eventCount != null)
        out.eventCount = eventCount;
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
    if (indexedParentBlockHashRaw != null && typeof indexedParentBlockHashRaw !== 'string')
        return null;
    if (typeof indexedParentBlockHashRaw === 'string' &&
        indexedParentBlockHashRaw !== '' &&
        !/^0x[0-9a-f]{64}$/i.test(indexedParentBlockHashRaw)) {
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
    if (schemaVersion != null)
        out.schemaVersion = schemaVersion;
    if (nftOwnerRowCount != null)
        out.nftOwnerRowCount = nftOwnerRowCount;
    const receiptLogCount = data.receiptLogCount;
    if (receiptLogCount != null && (typeof receiptLogCount !== 'number' || !Number.isFinite(receiptLogCount) || receiptLogCount < 0)) {
        return null;
    }
    if (receiptLogCount != null)
        out.receiptLogCount = receiptLogCount;
    return out;
}
/**
 * Parse `GET /v1/directory/pools` JSON. Drops pool objects that fail minimal hex validation.
 */
export function parseNativeDexDirectoryPoolsPageResponse(data) {
    if (!isPlainObject(data))
        return null;
    const api = data.api;
    if (api !== NATIVE_DEX_DIRECTORY_API_ID)
        return null;
    const limit = data.limit;
    if (typeof limit !== 'number' || !Number.isFinite(limit) || limit < 1)
        return null;
    const cursor = data.cursor;
    if (cursor != null && typeof cursor !== 'string')
        return null;
    const nextCursor = data.nextCursor;
    if (nextCursor != null && typeof nextCursor !== 'string')
        return null;
    const hasMore = data.hasMore;
    if (typeof hasMore !== 'boolean')
        return null;
    const rawPools = data.pools;
    if (!Array.isArray(rawPools))
        return null;
    const pools = [];
    for (const row of rawPools) {
        const p = tryParseIndexerPoolRow(row);
        if (p)
            pools.push(p);
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
function tryParseMaterializedPoolEvent(o) {
    if (!isPlainObject(o))
        return null;
    const kind = o.kind;
    if (typeof kind !== 'string' || !NATIVE_AMM_LOG2_KINDS.has(kind))
        return null;
    const poolHex = String(o.poolHex || '').trim().toLowerCase();
    if (!/^0x[0-9a-f]{64}$/.test(poolHex))
        return null;
    const blockHeight = o.blockHeight;
    if (typeof blockHeight !== 'number' || !Number.isFinite(blockHeight))
        return null;
    const txId = o.txId;
    if (typeof txId !== 'string' || !txId.trim())
        return null;
    const logIndex = o.logIndex;
    if (typeof logIndex !== 'number' || !Number.isFinite(logIndex))
        return null;
    const callerHex = String(o.callerHex || '').trim().toLowerCase();
    if (!/^0x[0-9a-f]{64}$/.test(callerHex))
        return null;
    const blockHashRaw = o.blockHash;
    let blockHash = null;
    if (blockHashRaw !== undefined && blockHashRaw !== null) {
        if (typeof blockHashRaw !== 'string' || (blockHashRaw !== '' && !/^0x[0-9a-f]{64}$/i.test(blockHashRaw))) {
            return null;
        }
        blockHash = blockHashRaw === '' ? null : blockHashRaw.toLowerCase();
    }
    const payload = o.payload;
    if (!isPlainObject(payload))
        return null;
    const payloadOut = {};
    for (const [k, v] of Object.entries(payload)) {
        if (typeof v !== 'string')
            return null;
        payloadOut[k] = v;
    }
    return {
        kind: kind,
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
export function parseNativeDexDirectoryPoolEventsPageResponse(data) {
    if (!isPlainObject(data))
        return null;
    const api = data.api;
    if (api !== NATIVE_DEX_DIRECTORY_API_ID)
        return null;
    const poolHex = String(data.poolHex || '').trim().toLowerCase();
    if (!/^0x[0-9a-f]{64}$/.test(poolHex))
        return null;
    const limit = data.limit;
    if (typeof limit !== 'number' || !Number.isFinite(limit) || limit < 1)
        return null;
    const cursor = data.cursor;
    if (cursor != null && typeof cursor !== 'string')
        return null;
    const nextCursor = data.nextCursor;
    if (nextCursor != null && typeof nextCursor !== 'string')
        return null;
    const hasMore = data.hasMore;
    if (typeof hasMore !== 'boolean')
        return null;
    const rawEvents = data.events;
    if (!Array.isArray(rawEvents))
        return null;
    const events = [];
    for (const row of rawEvents) {
        const e = tryParseMaterializedPoolEvent(row);
        if (e)
            events.push(e);
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
export function parseNativeDexDirectoryUserEventsPageResponse(data) {
    if (!isPlainObject(data))
        return null;
    const api = data.api;
    if (api !== NATIVE_DEX_DIRECTORY_API_ID)
        return null;
    const callerHex = String(data.callerHex || '').trim().toLowerCase();
    if (!/^0x[0-9a-f]{64}$/.test(callerHex))
        return null;
    const limit = data.limit;
    if (typeof limit !== 'number' || !Number.isFinite(limit) || limit < 1)
        return null;
    const cursor = data.cursor;
    if (cursor != null && typeof cursor !== 'string')
        return null;
    const nextCursor = data.nextCursor;
    if (nextCursor != null && typeof nextCursor !== 'string')
        return null;
    const hasMore = data.hasMore;
    if (typeof hasMore !== 'boolean')
        return null;
    const rawEvents = data.events;
    if (!Array.isArray(rawEvents))
        return null;
    const events = [];
    for (const row of rawEvents) {
        const e = tryParseMaterializedPoolEvent(row);
        if (e)
            events.push(e);
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
async function readJsonOrThrow(url, res) {
    const text = await res.text();
    let data;
    try {
        data = JSON.parse(text);
    }
    catch {
        throw new NativeDexDirectoryHttpError(`Invalid JSON from directory worker`, res.status, url, text.slice(0, 200));
    }
    if (!res.ok) {
        const errMsg = isPlainObject(data) && typeof data.error === 'string' ? data.error : `HTTP ${res.status}`;
        throw new NativeDexDirectoryHttpError(errMsg, res.status, url, text.slice(0, 200));
    }
    return data;
}
/**
 * `GET {baseUrl}/v1/directory/meta`
 */
/** Default fetch timeout for directory worker helpers (matches BoingClient default). */
export const DEFAULT_DIRECTORY_FETCH_TIMEOUT_MS = 30000;
function mergeDirectoryFetchInit(init, timeoutMs = DEFAULT_DIRECTORY_FETCH_TIMEOUT_MS) {
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
export async function fetchNativeDexDirectoryMeta(baseUrl, init) {
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
/**
 * `GET {baseUrl}/v1/directory/pools?limit=&cursor=`
 */
export async function fetchNativeDexDirectoryPoolsPage(baseUrl, query = {}, init) {
    const root = normalizeNativeDexDirectoryWorkerBaseUrl(baseUrl);
    const u = new URL(`${root}/v1/directory/pools`);
    if (query.limit != null)
        u.searchParams.set('limit', String(query.limit));
    if (query.cursor != null && query.cursor !== '')
        u.searchParams.set('cursor', query.cursor);
    const url = u.toString();
    const res = await fetch(url, mergeDirectoryFetchInit(init));
    const data = await readJsonOrThrow(url, res);
    const parsed = parseNativeDexDirectoryPoolsPageResponse(data);
    if (!parsed) {
        throw new NativeDexDirectoryHttpError('Unexpected /v1/directory/pools JSON shape', res.status, url);
    }
    return parsed;
}
/**
 * `GET {baseUrl}/v1/history/pool/{poolHex}/events?limit=&cursor=`
 */
export async function fetchNativeDexDirectoryPoolEventsPage(baseUrl, poolHex32, query = {}, init) {
    const root = normalizeNativeDexDirectoryWorkerBaseUrl(baseUrl);
    const pool = validateHex32(String(poolHex32).trim()).toLowerCase();
    const u = new URL(`${root}/v1/history/pool/${pool}/events`);
    if (query.limit != null)
        u.searchParams.set('limit', String(query.limit));
    if (query.cursor != null && query.cursor !== '')
        u.searchParams.set('cursor', query.cursor);
    const url = u.toString();
    const res = await fetch(url, mergeDirectoryFetchInit(init));
    const data = await readJsonOrThrow(url, res);
    const parsed = parseNativeDexDirectoryPoolEventsPageResponse(data);
    if (!parsed) {
        throw new NativeDexDirectoryHttpError('Unexpected pool events JSON shape', res.status, url);
    }
    return parsed;
}
/**
 * `GET {baseUrl}/v1/history/user/{callerHex}/events?limit=&cursor=`
 */
export async function fetchNativeDexDirectoryUserEventsPage(baseUrl, callerHex32, query = {}, init) {
    const root = normalizeNativeDexDirectoryWorkerBaseUrl(baseUrl);
    const caller = validateHex32(String(callerHex32).trim()).toLowerCase();
    const u = new URL(`${root}/v1/history/user/${caller}/events`);
    if (query.limit != null)
        u.searchParams.set('limit', String(query.limit));
    if (query.cursor != null && query.cursor !== '')
        u.searchParams.set('cursor', query.cursor);
    const url = u.toString();
    const res = await fetch(url, mergeDirectoryFetchInit(init));
    const data = await readJsonOrThrow(url, res);
    const parsed = parseNativeDexDirectoryUserEventsPageResponse(data);
    if (!parsed) {
        throw new NativeDexDirectoryHttpError('Unexpected user pool events JSON shape', res.status, url);
    }
    return parsed;
}
/**
 * Walk cursor pages until `hasMore` is false or limits hit.
 */
export async function collectAllNativeDexDirectoryPools(baseUrl, opts = {}) {
    const pageLimit = Math.min(100, Math.max(1, opts.pageLimit ?? 100));
    const maxPools = opts.maxPools ?? Number.POSITIVE_INFINITY;
    const maxPages = opts.maxPages ?? 10000;
    const out = [];
    let cursor;
    for (let page = 0; page < maxPages; page++) {
        const pageRes = await fetchNativeDexDirectoryPoolsPage(baseUrl, { limit: pageLimit, cursor: cursor ?? null }, opts.init);
        for (const p of pageRes.pools) {
            out.push(p);
            if (out.length >= maxPools)
                return out.slice(0, maxPools);
        }
        if (!pageRes.hasMore || pageRes.nextCursor == null || pageRes.nextCursor === '')
            break;
        cursor = pageRes.nextCursor;
    }
    return out;
}
