/**
 * Native DEX pair directory — single Boing RPC surface (no external chains).
 *
 * Composes {@link fetchNativeDexIntegrationDefaults}, factory storage reads, and optional
 * **`register_pair`** log backfill into one snapshot; helpers resolve **`(tokenA, tokenB)` → pool**.
 */
import type { BoingClient } from './client.js';
import { type NativeDexIntegrationDefaults, type NativeDexIntegrationOverrides } from './dexIntegration.js';
import { type FindNativeDexFactoryPoolOptions } from './nativeDexFactoryPool.js';
import type { NativeDexFactoryRegisterRpcParsed } from './nativeDexFactoryLogs.js';
/** Snapshot of operator hints + on-chain directory state (Boing RPC only). */
export type NativeDexDirectorySnapshot = {
    chainId: number | null;
    headHeight: number;
    defaults: NativeDexIntegrationDefaults;
    /** Factory storage **`pairs_count`** when factory address is known; otherwise **`null`**. */
    pairsCount: bigint | null;
    /**
     * Parsed **`register_pair`** logs when {@link FetchNativeDexDirectorySnapshotOptions.registerLogs} was set
     * and a factory address was resolved; otherwise **`null`** (not fetched).
     */
    registerLogs: NativeDexFactoryRegisterRpcParsed[] | null;
};
export type FetchNativeDexDirectorySnapshotOptions = {
    overrides?: NativeDexIntegrationOverrides;
    /**
     * Inclusive block range for **`boing_getLogs`** (**`register_pair`** on the factory).
     * **`toBlock`** defaults to the chain head from the same **`getNetworkInfo`** snapshot.
     */
    registerLogs?: {
        fromBlock: number;
        toBlock?: number;
    };
};
/** Canonical map key for an unordered token pair (lowercased **32-byte** hex ids). */
export declare function nativeDexPairKey(tokenAHex32: string, tokenBHex32: string): string;
/**
 * Build **`pairKey → poolHex`** from register logs. Later log entries win (on-chain re-register / ordering).
 */
export declare function buildNativeDexRegisterLogPoolIndex(logs: readonly NativeDexFactoryRegisterRpcParsed[]): ReadonlyMap<string, `0x${string}`>;
/** Resolve pool for **`(tokenA, tokenB)`** in either order using a register-log index. */
export declare function pickNativeDexPoolFromRegisterLogs(logs: readonly NativeDexFactoryRegisterRpcParsed[], tokenAHex32: string, tokenBHex32: string): `0x${string}` | null;
/**
 * Fetch network hints, optional factory pair count, and optional **`register_pair`** logs — **Boing RPC only**.
 */
export declare function fetchNativeDexDirectorySnapshot(client: BoingClient, options?: FetchNativeDexDirectorySnapshotOptions): Promise<NativeDexDirectorySnapshot>;
export type ResolveNativeDexPoolForTokensResult = {
    poolHex: `0x${string}` | null;
    factoryHex: `0x${string}` | null;
    via: 'logs' | 'simulate' | 'none';
};
export type ResolveNativeDexPoolForTokensOptions = {
    kind: 'logs';
    overrides?: NativeDexIntegrationOverrides;
    fromBlock: number;
    toBlock?: number;
} | {
    kind: 'simulate';
    overrides?: NativeDexIntegrationOverrides;
    find: FindNativeDexFactoryPoolOptions;
} | {
    kind: 'auto';
    overrides?: NativeDexIntegrationOverrides;
    fromBlock: number;
    toBlock?: number;
    find: FindNativeDexFactoryPoolOptions;
};
/**
 * Resolve **`tokenA` / `tokenB` → pool** using logs and/or directory simulation (**Boing-only**).
 */
export declare function resolveNativeDexPoolForTokens(client: BoingClient, tokenAHex32: string, tokenBHex32: string, options: ResolveNativeDexPoolForTokensOptions): Promise<ResolveNativeDexPoolForTokensResult>;
//# sourceMappingURL=nativeDexDirectory.d.ts.map