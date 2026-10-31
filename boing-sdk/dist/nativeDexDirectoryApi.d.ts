/**
 * HTTP JSON for the Cloudflare Worker `workers/native-dex-indexer` D1 directory
 * (`GET /v1/directory/meta`, `GET /v1/directory/pools`). Not Boing JSON-RPC.
 */
import type { NativeDexIndexerPoolRow } from './nativeDexIndexerStats.js';
import type { NativeDexMaterializedPoolEvent } from './nativeDexPoolHistory.js';
/** `api` field value on successful directory responses. */
export declare const NATIVE_DEX_DIRECTORY_API_ID: "boing-native-dex-directory/v1";
/** Bumped when D1 schema or meta semantics change in a breaking way for clients. */
export declare const NATIVE_DEX_DIRECTORY_SCHEMA_VERSION: 2;
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
export declare class NativeDexDirectoryHttpError extends Error {
    readonly status: number;
    readonly url: string;
    readonly bodySnippet?: string | undefined;
    constructor(message: string, status: number, url: string, bodySnippet?: string | undefined);
}
/** Trim and strip a trailing slash (no path segment — base is worker origin). */
export declare function normalizeNativeDexDirectoryWorkerBaseUrl(raw: string): string;
/**
 * Parse `GET /v1/directory/meta` JSON. Returns `null` if shape is invalid.
 */
export declare function parseNativeDexDirectoryMetaResponse(data: unknown): NativeDexDirectoryMetaResponse | null;
/**
 * Parse `GET /v1/directory/pools` JSON. Drops pool objects that fail minimal hex validation.
 */
export declare function parseNativeDexDirectoryPoolsPageResponse(data: unknown): NativeDexDirectoryPoolsPageResponse | null;
/**
 * Parse `GET /v1/history/pool/{pool}/events` JSON. Drops malformed event objects.
 */
export declare function parseNativeDexDirectoryPoolEventsPageResponse(data: unknown): NativeDexDirectoryPoolEventsPageResponse | null;
/**
 * Parse `GET /v1/history/user/{caller}/events` JSON. Drops malformed event objects.
 */
export declare function parseNativeDexDirectoryUserEventsPageResponse(data: unknown): NativeDexDirectoryUserEventsPageResponse | null;
/**
 * `GET {baseUrl}/v1/directory/meta`
 */
/** Default fetch timeout for directory worker helpers (matches BoingClient default). */
export declare const DEFAULT_DIRECTORY_FETCH_TIMEOUT_MS = 30000;
export declare function fetchNativeDexDirectoryMeta(baseUrl: string, init?: RequestInit): Promise<NativeDexDirectoryMetaResponse>;
export type FetchNativeDexDirectoryPoolsPageQuery = {
    /** 1–100; default 20 on server if omitted — we pass explicit default 20 when unset. */
    limit?: number;
    cursor?: string | null;
};
/**
 * `GET {baseUrl}/v1/directory/pools?limit=&cursor=`
 */
export declare function fetchNativeDexDirectoryPoolsPage(baseUrl: string, query?: FetchNativeDexDirectoryPoolsPageQuery, init?: RequestInit): Promise<NativeDexDirectoryPoolsPageResponse>;
export type FetchNativeDexDirectoryPoolEventsPageQuery = {
    limit?: number;
    /** Row `id` cursor from prior `nextCursor` (newest-first pages). */
    cursor?: string | null;
};
/**
 * `GET {baseUrl}/v1/history/pool/{poolHex}/events?limit=&cursor=`
 */
export declare function fetchNativeDexDirectoryPoolEventsPage(baseUrl: string, poolHex32: string, query?: FetchNativeDexDirectoryPoolEventsPageQuery, init?: RequestInit): Promise<NativeDexDirectoryPoolEventsPageResponse>;
export type FetchNativeDexDirectoryUserEventsPageQuery = {
    limit?: number;
    cursor?: string | null;
};
/**
 * `GET {baseUrl}/v1/history/user/{callerHex}/events?limit=&cursor=`
 */
export declare function fetchNativeDexDirectoryUserEventsPage(baseUrl: string, callerHex32: string, query?: FetchNativeDexDirectoryUserEventsPageQuery, init?: RequestInit): Promise<NativeDexDirectoryUserEventsPageResponse>;
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
export declare function collectAllNativeDexDirectoryPools(baseUrl: string, opts?: CollectNativeDexDirectoryPoolsOptions): Promise<NativeDexIndexerPoolRow[]>;
//# sourceMappingURL=nativeDexDirectoryApi.d.ts.map