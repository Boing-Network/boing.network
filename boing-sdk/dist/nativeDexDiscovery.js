/**
 * Single entry for native DEX discovery: L1 RPC → HTTP Worker directory → on-chain log snapshot.
 *
 * Prefer {@link createNativeDexDiscovery} over calling the layered helpers directly.
 */
import { buildNativeDexIntegrationOverridesFromProcessEnv, fetchNativeDexIntegrationDefaults, } from './dexIntegration.js';
import { fetchNativeDexDirectorySnapshot, resolveNativeDexPoolForTokens, } from './nativeDexDirectory.js';
import { collectAllNativeDexDirectoryPools, fetchNativeDexDirectoryMeta, fetchNativeDexDirectoryPoolsPage, normalizeNativeDexDirectoryWorkerBaseUrl, } from './nativeDexDirectoryApi.js';
function getProcessEnvRecord() {
    if (typeof globalThis === 'undefined')
        return undefined;
    const proc = globalThis
        .process;
    return proc?.env;
}
function readFirstProcessEnv(keys) {
    const env = getProcessEnvRecord();
    if (env == null)
        return undefined;
    for (const k of keys) {
        const v = env[k];
        if (v != null && String(v).trim())
            return String(v).trim();
    }
    return undefined;
}
/**
 * Directory Worker base URL from **`process.env`** (Node / Vite / CRA).
 * Keys: `REACT_APP_BOING_NATIVE_DEX_DIRECTORY_BASE_URL`, `VITE_…`, `BOING_…`.
 */
export function readNativeDexDirectoryBaseUrlFromProcessEnv() {
    return readFirstProcessEnv([
        'REACT_APP_BOING_NATIVE_DEX_DIRECTORY_BASE_URL',
        'VITE_BOING_NATIVE_DEX_DIRECTORY_BASE_URL',
        'BOING_NATIVE_DEX_DIRECTORY_BASE_URL',
    ]);
}
function preferredSource(rpcPools, directoryBaseUrl) {
    if (rpcPools)
        return 'rpc';
    if (directoryBaseUrl)
        return 'http';
    return 'logs';
}
async function collectAllListDexPools(client, params) {
    const out = [];
    let cursor = undefined;
    const limit = params?.limit ?? 200;
    for (let i = 0; i < 10000; i++) {
        const page = await client.listDexPoolsPage({
            cursor: cursor ?? null,
            limit,
            factory: params?.factory,
            light: params?.light,
        });
        out.push(...page.pools);
        if (!page.nextCursor)
            break;
        cursor = page.nextCursor;
    }
    return out;
}
/**
 * Build a discovery handle: loads network info + integration defaults, then exposes
 * RPC / Worker / log helpers with a clear preferred source.
 */
export async function createNativeDexDiscovery(client, options) {
    const envOverrides = options?.useProcessEnvOverrides === false
        ? {}
        : buildNativeDexIntegrationOverridesFromProcessEnv();
    const overrides = {
        ...envOverrides,
        ...options?.overrides,
    };
    const networkInfo = await client.getNetworkInfo();
    const defaults = await fetchNativeDexIntegrationDefaults(client, overrides);
    const methods = networkInfo.developer?.dex_discovery_methods ?? [];
    const rpcPoolsSupported = methods.includes('boing_listDexPools');
    const rpcTokensSupported = methods.includes('boing_listDexTokens');
    const rawBase = options?.directoryBaseUrl?.trim() || readNativeDexDirectoryBaseUrlFromProcessEnv() || '';
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
        listPoolsPage: (params) => client.listDexPoolsPage({
            ...params,
            factory: params?.factory ?? factoryDefault,
        }),
        listTokensPage: (params) => client.listDexTokensPage({
            ...params,
            factory: params?.factory ?? factoryDefault,
        }),
        getToken: (id, opts) => client.getDexToken(id, {
            ...opts,
            factory: opts?.factory ?? factoryDefault,
        }),
        directorySnapshot: (snapOpts) => fetchNativeDexDirectorySnapshot(client, {
            ...snapOpts,
            overrides,
        }),
        resolvePool: (a, b, resolveOpts) => resolveNativeDexPoolForTokens(client, a, b, {
            ...resolveOpts,
            overrides,
        }),
        directoryMeta: async () => {
            if (!directoryBaseUrl) {
                throw new Error('Native DEX directory base URL not configured (set BOING_NATIVE_DEX_DIRECTORY_BASE_URL or options.directoryBaseUrl)');
            }
            return fetchNativeDexDirectoryMeta(directoryBaseUrl);
        },
        directoryPoolsPage: async (query) => {
            if (!directoryBaseUrl) {
                throw new Error('Native DEX directory base URL not configured (set BOING_NATIVE_DEX_DIRECTORY_BASE_URL or options.directoryBaseUrl)');
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
                const pools = await collectAllNativeDexDirectoryPools(directoryBaseUrl, collectOpts?.http);
                return { source: 'http', pools };
            }
            return { source: 'none', pools: [] };
        },
    };
}
