/**
 * Parse native constant-product pool **`Log2`** rows (`topic0` + caller `topic1` + 96-byte `data`).
 * Matches `native_amm` emission and `docs/NATIVE-AMM-CALLDATA.md` § Logs.
 */
import type { ExecutionLog, ExecutionReceipt, RpcLogEntry } from './types.js';
/** Discriminant for a parsed native AMM `Log2` event. */
export type NativeAmmLog2Kind = 'swap' | 'addLiquidity' | 'removeLiquidity';
export type NativeAmmLog2Event = {
    kind: 'swap';
    callerHex: string;
    /** Calldata-style direction: `0` = A→B, `1` = B→A. */
    direction: bigint;
    amountIn: bigint;
    amountOutAfterFee: bigint;
} | {
    kind: 'addLiquidity';
    callerHex: string;
    amountA: bigint;
    amountB: bigint;
    lpMinted: bigint;
} | {
    kind: 'removeLiquidity';
    callerHex: string;
    liquidityBurned: bigint;
    amountAOut: bigint;
    amountBOut: bigint;
};
/** `tryParseNativeAmmRpcLogEntry` result: parsed fields plus RPC placement. */
export type NativeAmmRpcLogParsed = NativeAmmLog2Event & Pick<RpcLogEntry, 'block_height' | 'tx_index' | 'tx_id' | 'log_index' | 'address'>;
/** True if `topic` (any casing / optional `0x`) is one of the three native AMM `Log2` topic0 values. */
export declare function isNativeAmmLog2Topic0(topic: string): boolean;
/**
 * True if `log` looks like a native AMM `Log2` (two topics, known topic0). Does not validate `data`.
 */
export declare function isNativeAmmLog2Shape(log: Pick<ExecutionLog, 'topics'>): boolean;
/**
 * Parse one receipt / RPC log into a typed event, or `null` if shape or `data` does not match.
 */
export declare function tryParseNativeAmmLog2(log: Pick<ExecutionLog, 'topics' | 'data'>): NativeAmmLog2Event | null;
export declare function tryParseNativeAmmRpcLogEntry(entry: RpcLogEntry): NativeAmmRpcLogParsed | null;
/** Keep only `RpcLogEntry` rows that parse as native AMM `Log2`, with placement fields attached. */
export declare function filterMapNativeAmmRpcLogs(entries: readonly RpcLogEntry[]): NativeAmmRpcLogParsed[];
/** All successfully parsed native AMM logs in one receipt (in log order). */
export declare function collectNativeAmmLog2FromReceipt(receipt: ExecutionReceipt): Array<{
    logIndex: number;
    event: NativeAmmLog2Event;
}>;
//# sourceMappingURL=nativeAmmLogs.d.ts.map