/**
 * RPC error with optional structured data (e.g. QA rejection rule_id and message).
 * When thrown from BoingClient, `method` is set to the RPC method that failed.
 */
export declare class BoingRpcError extends Error {
    readonly code: number;
    readonly data?: unknown | undefined;
    /** RPC method that failed (e.g. "boing_getBalance"). */
    readonly method?: string | undefined;
    /**
     * From HTTP **`Retry-After`** when the server returned a retriable status (e.g. 429).
     * Used by `BoingClient` to wait at least this long before the next retry.
     */
    readonly retryAfterMs?: number | undefined;
    constructor(code: number, message: string, data?: unknown | undefined, 
    /** RPC method that failed (e.g. "boing_getBalance"). */
    method?: string | undefined, 
    /**
     * From HTTP **`Retry-After`** when the server returned a retriable status (e.g. 429).
     * Used by `BoingClient` to wait at least this long before the next retry.
     */
    retryAfterMs?: number | undefined);
    /** Short string for logging: "BoingRpcError(code, method): message". */
    toString(): string;
    /** True if this is a QA deployment rejection (-32050). */
    get isQaRejected(): boolean;
    /** True if deployment was referred to governance QA pool (-32051). */
    get isQaPendingPool(): boolean;
    /** True if QA pool is disabled by governance (-32054). */
    get isQaPoolDisabled(): boolean;
    /** True if QA pool hit global max pending (-32055). */
    get isQaPoolFull(): boolean;
    /** True if deployer exceeded per-address pool cap (-32056). */
    get isQaPoolDeployerCap(): boolean;
    /** True if the node rejected the call due to HTTP JSON-RPC rate limiting (-32016). */
    get isRateLimited(): boolean;
    /** For -32051, `data.tx_hash` when present. */
    get pendingPoolTxHash(): string | undefined;
    /** QA rejection details when code is -32050. */
    get qaData(): {
        rule_id: string;
        message: string;
    } | undefined;
}
/**
 * JSON-RPC 2.0 **method not found** (-32601). Common when the endpoint is an older Boing node or a
 * proxy that does not implement a newer `boing_*` method (e.g. `boing_getSyncState`, `boing_getLogs`).
 */
export declare function isBoingRpcMethodNotFound(e: unknown): boolean;
/**
 * Whether a failed call is worth retrying (transient HTTP, rate limits, network).
 * Application errors (e.g. invalid nonce, QA rejection) return false.
 */
export declare function isRetriableBoingRpcError(e: unknown): boolean;
/**
 * User-facing explanation for logging and UI (maps Boing JSON-RPC codes to short text).
 * See `docs/BOING-RPC-ERROR-CODES-FOR-DAPPS.md` in the boing-network repo.
 */
export declare function explainBoingRpcError(e: unknown): string;
//# sourceMappingURL=errors.d.ts.map