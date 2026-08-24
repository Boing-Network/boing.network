/**
 * One-call defaults for native Boing DEX wiring: merge **`boing_getNetworkInfo.end_user`**
 * hints with app overrides. Historical embedded 6913 ids are used only when network info
 * is omitted (offline). Live RPC nulls mean the hosted chain has not published those contracts.
 */
import type { BoingClient } from './client.js';
import { type NativeDexFactoryRegisterRpcParsed } from './nativeDexFactoryLogs.js';
import type { NetworkInfo } from './types.js';
export type NativeDexDefaultSource = 'rpc_end_user' | 'sdk_testnet_embedded' | 'override' | 'none';
/** Resolved pool / factory / router / LP helper accounts for native DEX UIs and calldata builders. */
export type NativeDexIntegrationDefaults = {
    nativeCpPoolAccountHex: `0x${string}` | null;
    nativeDexFactoryAccountHex: `0x${string}` | null;
    poolSource: NativeDexDefaultSource;
    factorySource: NativeDexDefaultSource;
    nativeDexMultihopSwapRouterAccountHex: `0x${string}` | null;
    nativeDexMultihopSwapRouterSource: NativeDexDefaultSource;
    nativeDexLedgerRouterV2AccountHex: `0x${string}` | null;
    nativeDexLedgerRouterV2Source: NativeDexDefaultSource;
    nativeDexLedgerRouterV3AccountHex: `0x${string}` | null;
    nativeDexLedgerRouterV3Source: NativeDexDefaultSource;
    nativeAmmLpVaultAccountHex: `0x${string}` | null;
    nativeAmmLpVaultSource: NativeDexDefaultSource;
    nativeLpShareTokenAccountHex: `0x${string}` | null;
    nativeLpShareTokenSource: NativeDexDefaultSource;
    /** From `boing_getNetworkInfo.end_user.explorer_url` when set (https URL). */
    endUserExplorerUrl: string | null;
};
export type NativeDexIntegrationOverrides = {
    nativeCpPoolAccountHex?: string;
    nativeDexFactoryAccountHex?: string;
    nativeDexMultihopSwapRouterAccountHex?: string;
    nativeDexLedgerRouterV2AccountHex?: string;
    nativeDexLedgerRouterV3AccountHex?: string;
    nativeAmmLpVaultAccountHex?: string;
    nativeLpShareTokenAccountHex?: string;
};
/**
 * Build {@link NativeDexIntegrationOverrides} from **`process.env`** (Node / Vite / CRA).
 * First non-empty value wins per key group. Safe to call from browser bundles if env is injected at build time.
 */
export declare function buildNativeDexIntegrationOverridesFromProcessEnv(): NativeDexIntegrationOverrides;
/**
 * Merge RPC **`end_user`** canonical addresses with optional app overrides.
 * Order per field: overrides → node hints. Embedded **6913** constants (see
 * [`canonicalTestnetDex.ts`](./canonicalTestnetDex.ts)) apply **only** when `info` is omitted
 * (offline / unit tests). A live `boing_getNetworkInfo` snapshot with null canonical fields
 * means the hosted chain has not published those contracts — do not substitute historical ids.
 */
export declare function mergeNativeDexIntegrationDefaults(info: NetworkInfo | null | undefined, overrides?: NativeDexIntegrationOverrides): NativeDexIntegrationDefaults;
/** Fetch **`boing_getNetworkInfo`** and {@link mergeNativeDexIntegrationDefaults}. */
export declare function fetchNativeDexIntegrationDefaults(client: BoingClient, overrides?: NativeDexIntegrationOverrides): Promise<NativeDexIntegrationDefaults>;
/**
 * Stream **`register_pair`** **`Log3`** rows for a factory (chunked **`boing_getLogs`**).
 * Requires a known factory **`AccountId`** (from {@link NativeDexIntegrationDefaults} or CREATE2 prediction).
 */
export declare function fetchNativeDexFactoryRegisterLogs(client: BoingClient, opts: {
    factoryAccountHex: string;
    fromBlock: number;
    toBlock: number;
}): Promise<NativeDexFactoryRegisterRpcParsed[]>;
//# sourceMappingURL=dexIntegration.d.ts.map