/**
 * Single entry for native DEX discovery: L1 RPC → HTTP Worker directory → on-chain log snapshot.
 *
 * Prefer {@link createNativeDexDiscovery} over calling the layered helpers directly.
 */
import type { BoingClient } from './client.js';
import { type NativeDexIntegrationDefaults, type NativeDexIntegrationOverrides } from './dexIntegration.js';
import { type FetchNativeDexDirectorySnapshotOptions, type NativeDexDirectorySnapshot, type ResolveNativeDexPoolForTokensOptions, type ResolveNativeDexPoolForTokensResult } from './nativeDexDirectory.js';
import { type CollectNativeDexDirectoryPoolsOptions, type FetchNativeDexDirectoryPoolsPageQuery, type NativeDexDirectoryMetaResponse, type NativeDexDirectoryPoolsPageResponse } from './nativeDexDirectoryApi.js';
import type { NativeDexIndexerPoolRow } from './nativeDexIndexerStats.js';
import type { DexPoolListPage, DexPoolListRow, DexTokenListPage, DexTokenListRow, NetworkInfo } from './types.js';
/**
 * Directory Worker base URL from **`process.env`** (Node / Vite / CRA).
 * Keys: `REACT_APP_BOING_NATIVE_DEX_DIRECTORY_BASE_URL`, `VITE_…`, `BOING_…`.
 */
export declare function readNativeDexDirectoryBaseUrlFromProcessEnv(): string | undefined;
export type NativeDexDiscoveryPoolSource = 'rpc' | 'http' | 'logs' | 'none';
export type CreateNativeDexDiscoveryOptions = {
    overrides?: NativeDexIntegrationOverrides;
    /** Prefer env when omitted. Normalized via {@link normalizeNativeDexDirectoryWorkerBaseUrl}. */
    directoryBaseUrl?: string;
    /** When true, merge {@link buildNativeDexIntegrationOverridesFromProcessEnv} under explicit overrides. */
    useProcessEnvOverrides?: boolean;
};
export type NativeDexDiscovery = {
    defaults: NativeDexIntegrationDefaults;
    networkInfo: NetworkInfo;
    /** True when `boing_listDexPools` is advertised on `getNetworkInfo.developer.dex_discovery_methods`. */
    rpcPoolsSupported: boolean;
    /** True when `boing_listDexTokens` is advertised. */
    rpcTokensSupported: boolean;
    /** Normalized Worker base URL when configured; otherwise null. */
    directoryBaseUrl: string | null;
    /** Preferred pool listing source given current capabilities. */
    preferredPoolSource: NativeDexDiscoveryPoolSource;
    listPoolsPage: (params?: {
        cursor?: string | null;
        limit?: number;
        factory?: string;
        light?: boolean;
        enrich?: boolean;
        includeDiagnostics?: boolean;
    }) => Promise<DexPoolListPage>;
    listTokensPage: (params?: {
        cursor?: string | null;
        limit?: number;
        factory?: string;
        light?: boolean;
        enrich?: boolean;
        minReserveProduct?: string;
        minLiquidityWei?: string;
        includeDiagnostics?: boolean;
    }) => Promise<DexTokenListPage>;
    getToken: (idHex32: string, options?: {
        factory?: string;
        light?: boolean;
        enrich?: boolean;
        includeDiagnostics?: boolean;
    }) => Promise<DexTokenListRow | null>;
    directorySnapshot: (options?: Omit<FetchNativeDexDirectorySnapshotOptions, 'overrides'>) => Promise<NativeDexDirectorySnapshot>;
    resolvePool: (tokenAHex32: string, tokenBHex32: string, options: Omit<ResolveNativeDexPoolForTokensOptions, 'overrides'>) => Promise<ResolveNativeDexPoolForTokensResult>;
    /** HTTP Worker meta when `directoryBaseUrl` is set. */
    directoryMeta: () => Promise<NativeDexDirectoryMetaResponse>;
    /** HTTP Worker pools page when `directoryBaseUrl` is set. */
    directoryPoolsPage: (query?: FetchNativeDexDirectoryPoolsPageQuery) => Promise<NativeDexDirectoryPoolsPageResponse>;
    /**
     * Collect all pools: L1 RPC cursor walk when supported; else HTTP Worker collect-all;
     * else empty with `source: 'none'`.
     */
    collectAllPools: (options?: {
        factory?: string;
        light?: boolean;
        http?: CollectNativeDexDirectoryPoolsOptions;
    }) => Promise<{
        source: NativeDexDiscoveryPoolSource;
        pools: DexPoolListRow[] | NativeDexIndexerPoolRow[];
    }>;
};
/**
 * Build a discovery handle: loads network info + integration defaults, then exposes
 * RPC / Worker / log helpers with a clear preferred source.
 */
export declare function createNativeDexDiscovery(client: BoingClient, options?: CreateNativeDexDiscoveryOptions): Promise<NativeDexDiscovery>;
//# sourceMappingURL=nativeDexDiscovery.d.ts.map