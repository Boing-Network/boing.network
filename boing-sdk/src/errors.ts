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
    public readonly method?: string
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
    return e.toString();
  }
  if (e instanceof Error) return e.message;
  return String(e);
}
