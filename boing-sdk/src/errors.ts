/**
 * RPC error with optional structured data (e.g. QA rejection rule_id and message).
 * When thrown from BoingClient, `method` is set to the RPC method that failed.
 */
export class BoingRpcError extends Error {
  constructor(
    public readonly code: number,
    message: string,
    public readonly data?: unknown,
    /** RPC method that failed (e.g. "boing_getBalance"). */
    public readonly method?: string,
    /**
     * From HTTP **`Retry-After`** when the server returned a retriable status (e.g. 429).
     * Used by `BoingClient` to wait at least this long before the next retry.
     */
    public readonly retryAfterMs?: number
  ) {
    super(message);
    this.name = 'BoingRpcError';
    Object.setPrototypeOf(this, BoingRpcError.prototype);
  }

  /** Short string for logging: "BoingRpcError(code, method): message". */
  override toString(): string {
    const method = this.method ? ` ${this.method}` : '';
    return `BoingRpcError(${this.code}${method}): ${this.message}`;
  }

  /** True if this is a QA deployment rejection (-32050). */
  get isQaRejected(): boolean {
    return this.code === -32050;
  }

  /** True if deployment was referred to governance QA pool (-32051). */
  get isQaPendingPool(): boolean {
    return this.code === -32051;
  }

  /** True if QA pool is disabled by governance (-32054). */
  get isQaPoolDisabled(): boolean {
    return this.code === -32054;
  }

  /** True if QA pool hit global max pending (-32055). */
  get isQaPoolFull(): boolean {
    return this.code === -32055;
  }

  /** True if deployer exceeded per-address pool cap (-32056). */
  get isQaPoolDeployerCap(): boolean {
    return this.code === -32056;
  }

  /** True if the node rejected the call due to HTTP JSON-RPC rate limiting (-32016). */
  get isRateLimited(): boolean {
    return this.code === -32016;
  }

  /** For -32051, `data.tx_hash` when present. */
  get pendingPoolTxHash(): string | undefined {
    if (this.code !== -32051 || !this.data || typeof this.data !== 'object') return undefined;
    const d = this.data as Record<string, unknown>;
    return typeof d.tx_hash === 'string' ? d.tx_hash : undefined;
  }

  /** QA rejection details when code is -32050. */
  get qaData(): { rule_id: string; message: string } | undefined {
    if (this.code !== -32050 || !this.data || typeof this.data !== 'object') return undefined;
    const d = this.data as Record<string, unknown>;
    if (typeof d.rule_id === 'string' && typeof d.message === 'string') {
      return { rule_id: d.rule_id, message: d.message };
    }
    return undefined;
  }
}

/**
 * JSON-RPC 2.0 **method not found** (-32601). Common when the endpoint is an older Boing node or a
 * proxy that does not implement a newer `boing_*` method (e.g. `boing_getSyncState`, `boing_getLogs`).
 */
export function isBoingRpcMethodNotFound(e: unknown): boolean {
  return e instanceof BoingRpcError && e.code === -32601;
}

/**
 * Whether a failed call is worth retrying (transient HTTP, rate limits, network).
 * Application errors (e.g. invalid nonce, QA rejection) return false.
 */
export function isRetriableBoingRpcError(e: unknown): boolean {
  if (!(e instanceof BoingRpcError)) return true;
  if (e.isRateLimited) return true;
  if (e.code === -32000) {
    const m = e.message;
    if (/\bHTTP 429\b/.test(m)) return true;
    if (/\bHTTP 502\b/.test(m)) return true;
    if (/\bHTTP 503\b/.test(m)) return true;
    if (/\bHTTP 504\b/.test(m)) return true;
    if (/Request timed out after \d+ms/.test(m)) return true;
    if (/ECONNRESET|ETIMEDOUT|ENOTFOUND|EAI_AGAIN/i.test(m)) return true;
    if (/fetch failed|Failed to fetch|Load failed|network/i.test(m)) return true;
  }
  return false;
}

/**
 * User-facing explanation for logging and UI (maps Boing JSON-RPC codes to short text).
 * See `docs/BOING-RPC-ERROR-CODES-FOR-DAPPS.md` in the boing-network repo.
 */
export function explainBoingRpcError(e: unknown): string {
  if (e instanceof BoingRpcError) {
    if (e.isQaRejected) {
      const q = e.qaData;
      return q ? `QA rejected (${q.rule_id}): ${q.message}` : `QA rejected: ${e.message}`;
    }
    if (e.isQaPendingPool) {
      const h = e.pendingPoolTxHash;
      return h
        ? `Deployment queued for QA pool (tx_hash ${h}). Vote via boing_qaPoolVote.`
        : `Deployment queued for QA pool: ${e.message}`;
    }
    if (e.isQaPoolDisabled) return `QA pool is disabled by governance: ${e.message}`;
    if (e.isQaPoolFull) return `QA pool is full (global cap): ${e.message}`;
    if (e.isQaPoolDeployerCap) return `QA pool deployer cap reached: ${e.message}`;
    if (e.code === -32057) return `Operator RPC authentication required: ${e.message}`;
    if (e.isRateLimited) {
      const hint =
        e.retryAfterMs != null && e.retryAfterMs > 0
          ? ` Wait at least ${Math.ceil(e.retryAfterMs / 1000)}s (Retry-After header).`
          : '';
      return `Rate limited: ${e.message}.${hint}`;
    }
    if (e.code === -32000 && e.message.includes('413')) {
      return `Request body too large for this RPC endpoint: ${e.message}`;
    }
    if (e.code === -32700)
      return `Invalid JSON in RPC body (parse error): ${e.message}`;
    if (e.code === -32600)
      return `Invalid JSON-RPC request (e.g. batch too large or malformed): ${e.message}`;
    if (e.code === -32601)
      return `RPC method not implemented on this endpoint (old node or filtered proxy): ${e.message}`;
    if (e.code === -32602) return `Invalid RPC params: ${e.message}`;
    if (e.code === -32000 && /duplicate transaction/i.test(e.message)) {
      return `This exact transaction is already in the mempool: ${e.message}. Wait for inclusion (or a failed-fee receipt) before deploying the same payload again.`;
    }
    if (e.code === -32000 && /insufficient balance for fee/i.test(e.message)) {
      return `Not enough native BOING to pay the execution fee: ${e.message}. Request the faucet and retry.`;
    }
    if (e.code === -32000 && e.retryAfterMs != null && e.retryAfterMs > 0) {
      return `Transient RPC error (wait ~${Math.ceil(e.retryAfterMs / 1000)}s per Retry-After): ${e.message}`;
    }
    return e.toString();
  }
  if (e instanceof Error) return e.message;
  return String(e);
}
