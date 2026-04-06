/**
 * Helpers for execution logs on receipts and blocks (no `address` field — use `boing_getLogs` / {@link BoingClient.getLogs} when you need attributed contract ids).
 */
import { ensureHex } from './hex.js';
const TOPIC_HEX_RE = /^[0-9a-fA-F]{64}$/;
/** Normalize one 32-byte topic to lowercase `0x` + 64 hex (throws if invalid). */
export function normalizeTopicWord(topic) {
    const raw = topic.trim().replace(/^0x/i, '');
    if (raw.length !== 64 || !TOPIC_HEX_RE.test(raw)) {
        throw new Error(`Topic must be 32 bytes (64 hex chars), got ${raw.length}`);
    }
    return '0x' + raw.toLowerCase();
}
/** Normalize `ExecutionLog` topics and data to lowercase hex with `0x`. */
export function normalizeExecutionLog(log) {
    return {
        topics: log.topics.map((t) => normalizeTopicWord(t)),
        data: ensureHex(log.data.trim().replace(/^0X/i, '0x')).toLowerCase(),
    };
}
/** `topics[0]` after normalize, or `undefined`. */
export function logTopic0(log) {
    const t0 = log.topics[0];
    return t0 != null ? normalizeTopicWord(t0) : undefined;
}
/** Yield each log in a receipt (empty if none). */
export function* iterReceiptLogs(receipt) {
    for (let logIndex = 0; logIndex < receipt.logs.length; logIndex++) {
        yield { receipt, log: receipt.logs[logIndex], logIndex };
    }
}
/**
 * Topic filter semantics (common log-RPC shape): `filter[i]` null/undefined = wildcard; otherwise exact match on `topics[i]` (normalized).
 */
export function logMatchesTopicFilter(log, filter) {
    for (let i = 0; i < filter.length; i++) {
        const want = filter[i];
        if (want == null)
            continue;
        const a = log.topics[i];
        if (a == null)
            return false;
        if (normalizeTopicWord(a) !== normalizeTopicWord(want))
            return false;
    }
    return true;
}
/** Optional `topic0` filter on receipt logs. */
export function filterReceiptLogsByTopic0(receipt, topic0) {
    const want = normalizeTopicWord(topic0);
    return receipt.logs.filter((log) => logTopic0(log) === want);
}
/**
 * Walk `block.receipts` (same order as `transactions`) and emit every log with receipt context.
 * Skips `null` receipt slots.
 */
export function* iterBlockReceiptLogs(block) {
    const receipts = block.receipts;
    if (receipts == null)
        return;
    for (const receipt of receipts) {
        if (receipt == null)
            continue;
        yield* iterReceiptLogs(receipt);
    }
}
