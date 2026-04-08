/**
 * One-call defaults for native Boing DEX wiring: merge **`boing_getNetworkInfo.end_user`**
 * hints with embedded testnet fallbacks and app overrides.
 *
 * See [BOING-DAPP-INTEGRATION.md](../../docs/BOING-DAPP-INTEGRATION.md) § **Seamless native DEX defaults**.
 */
import { CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX } from './canonicalTestnet.js';
import { CANONICAL_BOING_TESTNET_NATIVE_AMM_LP_VAULT_HEX, CANONICAL_BOING_TESTNET_NATIVE_DEX_FACTORY_HEX, CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V2_HEX, CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V3_HEX, CANONICAL_BOING_TESTNET_NATIVE_DEX_MULTIHOP_SWAP_ROUTER_HEX, CANONICAL_BOING_TESTNET_NATIVE_LP_SHARE_TOKEN_HEX, } from './canonicalTestnetDex.js';
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
function mergeOptionalAccountHex(chainId, override, rpcField, embeddedWhenTestnet) {
    const o = override;
    if (o?.trim()) {
        try {
            return { hex: validateHex32(o), source: 'override' };
        }
        catch {
            return { hex: null, source: 'none' };
        }
    }
    const rpc = parseOptionalHex32(rpcField ?? null);
    if (rpc)
        return { hex: rpc, source: 'rpc_end_user' };
    if (chainId != null && isBoingTestnetChainId(chainId) && embeddedWhenTestnet != null) {
        return { hex: embeddedWhenTestnet, source: 'sdk_testnet_embedded' };
    }
    return { hex: null, source: 'none' };
}
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
 * Build {@link NativeDexIntegrationOverrides} from **`process.env`** (Node / Vite / CRA).
 * First non-empty value wins per key group. Safe to call from browser bundles if env is injected at build time.
 */
export function buildNativeDexIntegrationOverridesFromProcessEnv() {
    const o = {};
    const pool = readFirstProcessEnv([
        'REACT_APP_BOING_NATIVE_AMM_POOL',
        'VITE_BOING_NATIVE_AMM_POOL',
        'BOING_NATIVE_AMM_POOL',
    ]);
    const fac = readFirstProcessEnv([
        'REACT_APP_BOING_NATIVE_VM_DEX_FACTORY',
        'VITE_BOING_NATIVE_VM_DEX_FACTORY',
        'BOING_NATIVE_VM_DEX_FACTORY',
        'BOING_DEX_FACTORY_HEX',
    ]);
    const hop = readFirstProcessEnv([
        'REACT_APP_BOING_NATIVE_VM_SWAP_ROUTER',
        'VITE_BOING_NATIVE_VM_SWAP_ROUTER',
        'BOING_NATIVE_VM_SWAP_ROUTER',
        'BOING_NATIVE_DEX_MULTIHOP_SWAP_ROUTER',
    ]);
    const l2 = readFirstProcessEnv([
        'REACT_APP_BOING_NATIVE_DEX_LEDGER_ROUTER_V2',
        'VITE_BOING_NATIVE_DEX_LEDGER_ROUTER_V2',
        'BOING_NATIVE_DEX_LEDGER_ROUTER_V2',
    ]);
    const l3 = readFirstProcessEnv([
        'REACT_APP_BOING_NATIVE_DEX_LEDGER_ROUTER_V3',
        'VITE_BOING_NATIVE_DEX_LEDGER_ROUTER_V3',
        'BOING_NATIVE_DEX_LEDGER_ROUTER_V3',
    ]);
    const vault = readFirstProcessEnv([
        'REACT_APP_BOING_NATIVE_AMM_LP_VAULT',
        'VITE_BOING_NATIVE_AMM_LP_VAULT',
        'BOING_NATIVE_AMM_LP_VAULT',
    ]);
    const share = readFirstProcessEnv([
        'REACT_APP_BOING_NATIVE_AMM_LP_SHARE_TOKEN',
        'VITE_BOING_NATIVE_AMM_LP_SHARE_TOKEN',
        'BOING_NATIVE_AMM_LP_SHARE_TOKEN',
    ]);
    if (pool)
        o.nativeCpPoolAccountHex = pool;
    if (fac)
        o.nativeDexFactoryAccountHex = fac;
    if (hop)
        o.nativeDexMultihopSwapRouterAccountHex = hop;
    if (l2)
        o.nativeDexLedgerRouterV2AccountHex = l2;
    if (l3)
        o.nativeDexLedgerRouterV3AccountHex = l3;
    if (vault)
        o.nativeAmmLpVaultAccountHex = vault;
    if (share)
        o.nativeLpShareTokenAccountHex = share;
    return o;
}
/**
 * Merge RPC **`end_user`** canonical addresses, optional app overrides, and embedded **6913** fallbacks
 * (see [`canonicalTestnetDex.ts`](./canonicalTestnetDex.ts)).
 * Order per field: overrides → node hints → testnet embedded constants.
 */
export function mergeNativeDexIntegrationDefaults(info, overrides) {
    const chainId = info?.chain_id ?? null;
    const eu = info?.end_user;
    const poolEmb = CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX;
    const facEmb = CANONICAL_BOING_TESTNET_NATIVE_DEX_FACTORY_HEX;
    const hopEmb = CANONICAL_BOING_TESTNET_NATIVE_DEX_MULTIHOP_SWAP_ROUTER_HEX;
    const l2Emb = CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V2_HEX;
    const l3Emb = CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V3_HEX;
    const vaultEmb = CANONICAL_BOING_TESTNET_NATIVE_AMM_LP_VAULT_HEX;
    const shareEmb = CANONICAL_BOING_TESTNET_NATIVE_LP_SHARE_TOKEN_HEX;
    const pool = mergeOptionalAccountHex(chainId, overrides?.nativeCpPoolAccountHex, eu?.canonical_native_cp_pool ?? null, poolEmb);
    const factory = mergeOptionalAccountHex(chainId, overrides?.nativeDexFactoryAccountHex, eu?.canonical_native_dex_factory ?? null, facEmb);
    const multihop = mergeOptionalAccountHex(chainId, overrides?.nativeDexMultihopSwapRouterAccountHex, eu?.canonical_native_dex_multihop_swap_router ?? null, hopEmb);
    const ledgerV2 = mergeOptionalAccountHex(chainId, overrides?.nativeDexLedgerRouterV2AccountHex, eu?.canonical_native_dex_ledger_router_v2 ?? null, l2Emb);
    const ledgerV3 = mergeOptionalAccountHex(chainId, overrides?.nativeDexLedgerRouterV3AccountHex, eu?.canonical_native_dex_ledger_router_v3 ?? null, l3Emb);
    const vault = mergeOptionalAccountHex(chainId, overrides?.nativeAmmLpVaultAccountHex, eu?.canonical_native_amm_lp_vault ?? null, vaultEmb);
    const share = mergeOptionalAccountHex(chainId, overrides?.nativeLpShareTokenAccountHex, eu?.canonical_native_lp_share_token ?? null, shareEmb);
    let endUserExplorerUrl = null;
    const ex = eu?.explorer_url;
    if (typeof ex === 'string') {
        const t = ex.trim();
        if (t && /^https?:\/\//i.test(t)) {
            endUserExplorerUrl = t.replace(/\/+$/, '');
        }
    }
    return {
        nativeCpPoolAccountHex: pool.hex,
        nativeDexFactoryAccountHex: factory.hex,
        poolSource: pool.source,
        factorySource: factory.source,
        nativeDexMultihopSwapRouterAccountHex: multihop.hex,
        nativeDexMultihopSwapRouterSource: multihop.source,
        nativeDexLedgerRouterV2AccountHex: ledgerV2.hex,
        nativeDexLedgerRouterV2Source: ledgerV2.source,
        nativeDexLedgerRouterV3AccountHex: ledgerV3.hex,
        nativeDexLedgerRouterV3Source: ledgerV3.source,
        nativeAmmLpVaultAccountHex: vault.hex,
        nativeAmmLpVaultSource: vault.source,
        nativeLpShareTokenAccountHex: share.hex,
        nativeLpShareTokenSource: share.source,
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
