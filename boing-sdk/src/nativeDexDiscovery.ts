/**
 * Single entry for native DEX discovery: L1 RPC → HTTP Worker directory → on-chain log snapshot.
 *
 * Prefer {@link createNativeDexDiscovery} over calling the layered helpers directly.
 */

import type { BoingClient } from './client.js';
import {
  buildNativeDexIntegrationOverridesFromProcessEnv,
  fetchNativeDexIntegrationDefaults,
  type NativeDexIntegrationDefaults,
  type NativeDexIntegrationOverrides,
} from './dexIntegration.js';
import {
  fetchNativeDexDirectorySnapshot,
  resolveNativeDexPoolForTokens,
  type FetchNativeDexDirectorySnapshotOptions,
  type NativeDexDirectorySnapshot,
  type ResolveNativeDexPoolForTokensOptions,
  type ResolveNativeDexPoolForTokensResult,
} from './nativeDexDirectory.js';
import {
  collectAllNativeDexDirectoryPools,
  fetchNativeDexDirectoryMeta,
  fetchNativeDexDirectoryPoolsPage,
  normalizeNativeDexDirectoryWorkerBaseUrl,
  type CollectNativeDexDirectoryPoolsOptions,
  type FetchNativeDexDirectoryPoolsPageQuery,
  type NativeDexDirectoryMetaResponse,
  type NativeDexDirectoryPoolsPageResponse,
} from './nativeDexDirectoryApi.js';
import type { NativeDexIndexerPoolRow } from './nativeDexIndexerStats.js';
import type {
  DexPoolListPage,
  DexPoolListRow,
  DexTokenListPage,
  DexTokenListRow,
  NetworkInfo,
} from './types.js';

function getProcessEnvRecord(): Record<string, string | undefined> | undefined {
  if (typeof globalThis === 'undefined') return undefined;
  const proc = (globalThis as unknown as { process?: { env?: Record<string, string | undefined> } })
    .process;
  return proc?.env;
}

function readFirstProcessEnv(keys: readonly string[]): string | undefined {
  const env = getProcessEnvRecord();
  if (env == null) return undefined;
  for (const k of keys) {
    const v = env[k];
    if (v != null && String(v).trim()) return String(v).trim();
  }
  return undefined;
}

/**
 * Directory Worker base URL from **`process.env`** (Node / Vite / CRA).
 * Keys: `REACT_APP_BOING_NATIVE_DEX_DIRECTORY_BASE_URL`, `VITE_…`, `BOING_…`.
 */
export function readNativeDexDirectoryBaseUrlFromProcessEnv(): string | undefined {
  return readFirstProcessEnv([
    'REACT_APP_BOING_NATIVE_DEX_DIRECTORY_BASE_URL',
    'VITE_BOING_NATIVE_DEX_DIRECTORY_BASE_URL',
    'BOING_NATIVE_DEX_DIRECTORY_BASE_URL',
  ]);
}

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
  getToken: (
    idHex32: string,
    options?: { factory?: string; light?: boolean; enrich?: boolean; includeDiagnostics?: boolean },
  ) => Promise<DexTokenListRow | null>;
  directorySnapshot: (
    options?: Omit<FetchNativeDexDirectorySnapshotOptions, 'overrides'>,
  ) => Promise<NativeDexDirectorySnapshot>;
  resolvePool: (
    tokenAHex32: string,
    tokenBHex32: string,
    options: Omit<ResolveNativeDexPoolForTokensOptions, 'overrides'>,
  ) => Promise<ResolveNativeDexPoolForTokensResult>;
  /** HTTP Worker meta when `directoryBaseUrl` is set. */
  directoryMeta: () => Promise<NativeDexDirectoryMetaResponse>;
  /** HTTP Worker pools page when `directoryBaseUrl` is set. */
  directoryPoolsPage: (
    query?: FetchNativeDexDirectoryPoolsPageQuery,
  ) => Promise<NativeDexDirectoryPoolsPageResponse>;
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

function preferredSource(
  rpcPools: boolean,
  directoryBaseUrl: string | null,
): NativeDexDiscoveryPoolSource {
  if (rpcPools) return 'rpc';
  if (directoryBaseUrl) return 'http';
  return 'logs';
}

async function collectAllListDexPools(
  client: BoingClient,
  params?: { factory?: string; light?: boolean; limit?: number },
): Promise<DexPoolListRow[]> {
  const out: DexPoolListRow[] = [];
  let cursor: string | null | undefined = undefined;
  const limit = params?.limit ?? 200;
  for (let i = 0; i < 10_000; i++) {
    const page = await client.listDexPoolsPage({
      cursor: cursor ?? null,
      limit,
      factory: params?.factory,
      light: params?.light,
    });
    out.push(...page.pools);
    if (!page.nextCursor) break;
    cursor = page.nextCursor;
  }
  return out;
}

/**
 * Build a discovery handle: loads network info + integration defaults, then exposes
 * RPC / Worker / log helpers with a clear preferred source.
 */
export async function createNativeDexDiscovery(
  client: BoingClient,
  options?: CreateNativeDexDiscoveryOptions,
): Promise<NativeDexDiscovery> {
  const envOverrides =
    options?.useProcessEnvOverrides === false
      ? {}
      : buildNativeDexIntegrationOverridesFromProcessEnv();
  const overrides: NativeDexIntegrationOverrides = {
    ...envOverrides,
    ...options?.overrides,
  };

  const networkInfo = await client.getNetworkInfo();
  const defaults = await fetchNativeDexIntegrationDefaults(client, overrides);

  const methods = networkInfo.developer?.dex_discovery_methods ?? [];
  const rpcPoolsSupported = methods.includes('boing_listDexPools');
  const rpcTokensSupported = methods.includes('boing_listDexTokens');

  const rawBase =
    options?.directoryBaseUrl?.trim() || readNativeDexDirectoryBaseUrlFromProcessEnv() || '';
  const directoryBaseUrl = rawBase
    ? normalizeNativeDexDirectoryWorkerBaseUrl(rawBase)
    : null;

  const preferredPoolSource = preferredSource(rpcPoolsSupported, directoryBaseUrl);

  const factoryDefault = defaults.nativeDexFactoryAccountHex ?? undefined;

  return {
    defaults,
    networkInfo,
    rpcPoolsSupported,
    rpcTokensSupported,
    directoryBaseUrl,
    preferredPoolSource,
    listPoolsPage: (params) =>
      client.listDexPoolsPage({
        ...params,
        factory: params?.factory ?? factoryDefault,
      }),
    listTokensPage: (params) =>
      client.listDexTokensPage({
        ...params,
        factory: params?.factory ?? factoryDefault,
      }),
    getToken: (id, opts) =>
      client.getDexToken(id, {
        ...opts,
        factory: opts?.factory ?? factoryDefault,
      }),
    directorySnapshot: (snapOpts) =>
      fetchNativeDexDirectorySnapshot(client, {
        ...snapOpts,
        overrides,
      }),
    resolvePool: (a, b, resolveOpts) =>
      resolveNativeDexPoolForTokens(client, a, b, {
        ...resolveOpts,
        overrides,
      } as ResolveNativeDexPoolForTokensOptions),
    directoryMeta: async () => {
      if (!directoryBaseUrl) {
        throw new Error(
          'Native DEX directory base URL not configured (set BOING_NATIVE_DEX_DIRECTORY_BASE_URL or options.directoryBaseUrl)',
        );
      }
      return fetchNativeDexDirectoryMeta(directoryBaseUrl);
    },
    directoryPoolsPage: async (query) => {
      if (!directoryBaseUrl) {
        throw new Error(
          'Native DEX directory base URL not configured (set BOING_NATIVE_DEX_DIRECTORY_BASE_URL or options.directoryBaseUrl)',
        );
      }
      return fetchNativeDexDirectoryPoolsPage(directoryBaseUrl, query);
    },
    collectAllPools: async (collectOpts) => {
      if (rpcPoolsSupported) {
        const pools = await collectAllListDexPools(client, {
          factory: collectOpts?.factory ?? factoryDefault,
          light: collectOpts?.light,
        });
        return { source: 'rpc', pools };
      }
      if (directoryBaseUrl) {
        const pools = await collectAllNativeDexDirectoryPools(
          directoryBaseUrl,
          collectOpts?.http,
        );
        return { source: 'http', pools };
      }
      return { source: 'none', pools: [] };
    },
  };
}
