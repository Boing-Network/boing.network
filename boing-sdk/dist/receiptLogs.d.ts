/**
 * Helpers for execution logs on receipts and blocks (no `address` field — use `boing_getLogs` / {@link BoingClient.getLogs} when you need attributed contract ids).
 */
import type { Block, ExecutionLog, ExecutionReceipt } from './types.js';
/** Normalize one 32-byte topic to lowercase `0x` + 64 hex (throws if invalid). */
export declare function normalizeTopicWord(topic: string): string;
/** Normalize `ExecutionLog` topics and data to lowercase hex with `0x`. */
export declare function normalizeExecutionLog(log: ExecutionLog): ExecutionLog;
/** `topics[0]` after normalize, or `undefined`. */
export declare function logTopic0(log: ExecutionLog): string | undefined;
export interface ReceiptLogRef {
    receipt: ExecutionReceipt;
    log: ExecutionLog;
    logIndex: number;
}
/** Yield each log in a receipt (empty if none). */
export declare function iterReceiptLogs(receipt: ExecutionReceipt): Generator<ReceiptLogRef>;
/**
 * Topic filter semantics (common log-RPC shape): `filter[i]` null/undefined = wildcard; otherwise exact match on `topics[i]` (normalized).
 */
export declare function logMatchesTopicFilter(log: ExecutionLog, filter: (string | null | undefined)[]): boolean;
/** Optional `topic0` filter on receipt logs. */
export declare function filterReceiptLogsByTopic0(receipt: ExecutionReceipt, topic0: string): ExecutionLog[];
/**
 * Walk `block.receipts` (same order as `transactions`) and emit every log with receipt context.
 * Skips `null` receipt slots.
 */
export declare function iterBlockReceiptLogs(block: Block): Generator<ReceiptLogRef>;
//# sourceMappingURL=receiptLogs.d.ts.map