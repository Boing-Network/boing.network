/**
 * One-call defaults for native Boing DEX wiring: merge **`boing_getNetworkInfo.end_user`**
 * hints with embedded testnet fallbacks and app overrides.
 *
 * See [BOING-DAPP-INTEGRATION.md](../../docs/BOING-DAPP-INTEGRATION.md) § **Seamless native DEX defaults**.
 */
import type { BoingClient } from './client.js';
import { type NativeDexFactoryRegisterRpcParsed } from './nativeDexFactoryLogs.js';
import type { NetworkInfo } from './types.js';
export type NativeDexDefaultSource = 'rpc_end_user' | 'sdk_testnet_embedded' | 'override' | 'none';
/** Resolved pool / factory accounts for native DEX UIs and calldata builders. */
export type NativeDexIntegrationDefaults = {
    nativeCpPoolAccountHex: `0x${string}` | null;
    nativeDexFactoryAccountHex: `0x${string}` | null;
    poolSource: NativeDexDefaultSource;
    factorySource: NativeDexDefaultSource;
    /** From `boing_getNetworkInfo.end_user.explorer_url` when set (https URL). */
    endUserExplorerUrl: string | null;
};
export type NativeDexIntegrationOverrides = {
    nativeCpPoolAccountHex?: string;
    nativeDexFactoryAccountHex?: string;
};
/**
 * Merge RPC **`end_user`** canonical addresses, optional app overrides, and embedded **6913** pool fallback.
 * Order: overrides → node hints → testnet embedded pool (factory has no embedded default).
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