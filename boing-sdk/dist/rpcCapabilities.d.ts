/**
 * Optional `boing_*` JSON-RPC methods: probe availability without assuming a full node build.
 * Uses JSON-RPC **-32601** (method not found) to detect missing handlers — same as integration fallbacks.
 */
import type { BoingClient } from './client.js';
/** Result of probing a single RPC method. */
export interface BoingRpcMethodProbe {
    /** `true` if the call completed without throwing. */
    available: boolean;
    /** Present when `available` is false. */
    code?: number;
    message?: string;
}
/** Named probes useful for indexers, wallets, and debugging version skew. */
export interface BoingRpcCapabilities {
    boing_chainHeight: BoingRpcMethodProbe;
    boing_getSyncState: BoingRpcMethodProbe;
    boing_getBlockByHeight: BoingRpcMethodProbe;
    boing_getLogs: BoingRpcMethodProbe;
    boing_getTransactionReceipt: BoingRpcMethodProbe;
    boing_getNetworkInfo: BoingRpcMethodProbe;
}
/** Result of {@link probeBoingRpcCapabilities}: core probes plus optional discovery fields. */
export interface BoingRpcProbeBundle {
    /** From `boing_clientVersion` when implemented; **`null`** if **-32601** or unset. */
    clientVersion: string | null;
    /** From `boing_rpcSupportedMethods` when implemented; **`null`** if **-32601** or unset. */
    supportedMethods: string[] | null;
    methods: BoingRpcCapabilities;
}
/**
 * Call discovery plus a small set of read-only RPCs with minimal parameters.
 * A method is **available** if it returns (including JSON-RPC `result: null` for unknown tx/block).
 */
export declare function probeBoingRpcCapabilities(client: BoingClient): Promise<BoingRpcProbeBundle>;
/** Count of core probed methods with `available: true`. */
export declare function countAvailableBoingRpcMethods(bundleOrMethods: BoingRpcProbeBundle | BoingRpcCapabilities): number;
/**
 * Human-readable diagnosis when **`probeBoingRpcCapabilities`** shows gaps.
 * **`undefined`** when all probed methods are available, or when there is nothing actionable to say.
 */
export declare function explainBoingRpcProbeGaps(bundleOrMethods: BoingRpcProbeBundle | BoingRpcCapabilities): string | undefined;
//# sourceMappingURL=rpcCapabilities.d.ts.map