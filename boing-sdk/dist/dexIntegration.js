/**
 * One-call defaults for native Boing DEX wiring: merge **`boing_getNetworkInfo.end_user`**
 * hints with embedded testnet fallbacks and app overrides.
 *
 * See [BOING-DAPP-INTEGRATION.md](../../docs/BOING-DAPP-INTEGRATION.md) § **Seamless native DEX defaults**.
 */
import { CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX } from './canonicalTestnet.js';
import { isBoingTestnetChainId } from './chainIds.js';
import { validateHex32 } from './hex.js';
import { getLogsChunked } from './indexerBatch.js';
import { NATIVE_DEX_FACTORY_TOPIC_REGISTER_HEX } from './nativeDexFactory.js';
import { tryParseNativeDexFactoryRegisterRpcLogEntry, } from './nativeDexFactoryLogs.js';
function parseOptionalHex32(v) {
    if (v == null || typeof v !== 'string')
        return null;
    const t = v.trim();
    if (!t)
        return null;
    try {
        return validateHex32(t);
    }
    catch {
        return null;
    }
}
/**
 * Merge RPC **`end_user`** canonical addresses, optional app overrides, and embedded **6913** pool fallback.
 * Order: overrides → node hints → testnet embedded pool (factory has no embedded default).
 */
export function mergeNativeDexIntegrationDefaults(info, overrides) {
    const chainId = info?.chain_id ?? null;
    const eu = info?.end_user;
    let nativeCpPoolAccountHex = null;
    let poolSource = 'none';
    const oPool = overrides?.nativeCpPoolAccountHex;
    if (oPool?.trim()) {
        try {
            nativeCpPoolAccountHex = validateHex32(oPool);
            poolSource = 'override';
        }
        catch {
            poolSource = 'none';
        }
    }
    else {
        const rpcPool = parseOptionalHex32(eu?.canonical_native_cp_pool ?? null);
        if (rpcPool) {
            nativeCpPoolAccountHex = rpcPool;
            poolSource = 'rpc_end_user';
        }
        else if (chainId != null && isBoingTestnetChainId(chainId)) {
            nativeCpPoolAccountHex = CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX;
            poolSource = 'sdk_testnet_embedded';
        }
    }
    let nativeDexFactoryAccountHex = null;
    let factorySource = 'none';
    const oFac = overrides?.nativeDexFactoryAccountHex;
    if (oFac?.trim()) {
        try {
            nativeDexFactoryAccountHex = validateHex32(oFac);
            factorySource = 'override';
        }
        catch {
            factorySource = 'none';
        }
    }
    else {
        const rpcFac = parseOptionalHex32(eu?.canonical_native_dex_factory ?? null);
        if (rpcFac) {
            nativeDexFactoryAccountHex = rpcFac;
            factorySource = 'rpc_end_user';
        }
    }
    let endUserExplorerUrl = null;
    const ex = eu?.explorer_url;
    if (typeof ex === 'string') {
        const t = ex.trim();
        if (t && /^https?:\/\//i.test(t)) {
            endUserExplorerUrl = t.replace(/\/+$/, '');
        }
    }
    return {
        nativeCpPoolAccountHex,
        nativeDexFactoryAccountHex,
        poolSource,
        factorySource,
        endUserExplorerUrl,
    };
}
/** Fetch **`boing_getNetworkInfo`** and {@link mergeNativeDexIntegrationDefaults}. */
export async function fetchNativeDexIntegrationDefaults(client, overrides) {
    const info = await client.getNetworkInfo();
    return mergeNativeDexIntegrationDefaults(info, overrides);
}
/**
 * Stream **`register_pair`** **`Log3`** rows for a factory (chunked **`boing_getLogs`**).
 * Requires a known factory **`AccountId`** (from {@link NativeDexIntegrationDefaults} or CREATE2 prediction).
 */
export async function fetchNativeDexFactoryRegisterLogs(client, opts) {
    const factoryAccountHex = validateHex32(opts.factoryAccountHex);
    const raw = await getLogsChunked(client, {
        fromBlock: opts.fromBlock,
        toBlock: opts.toBlock,
        address: factoryAccountHex,
        topics: [NATIVE_DEX_FACTORY_TOPIC_REGISTER_HEX, null, null],
    }, {});
    const out = [];
    for (const row of raw) {
        const p = tryParseNativeDexFactoryRegisterRpcLogEntry(row);
        if (p)
            out.push(p);
    }
    return out;
}
