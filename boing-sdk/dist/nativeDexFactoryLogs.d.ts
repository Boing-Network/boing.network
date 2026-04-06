/**
 * Parse pair-directory **`Log3`** after `register_pair` (`topic0` + token_a + token_b, **data** = pool).
 * Matches `native_dex_factory::NATIVE_DEX_FACTORY_TOPIC_REGISTER`.
 */
import type { ExecutionLog, ExecutionReceipt, RpcLogEntry } from './types.js';
export type NativeDexFactoryRegisterEvent = {
    tokenAHex: string;
    tokenBHex: string;
    poolHex: string;
};
export type NativeDexFactoryRegisterRpcParsed = NativeDexFactoryRegisterEvent & Pick<RpcLogEntry, 'block_height' | 'tx_index' | 'tx_id' | 'log_index' | 'address'>;
/** True if normalized `topic0` is the factory register topic. */
export declare function isNativeDexFactoryRegisterTopic0(topic: string): boolean;
/** `Log3` with known topic0; **data** must be exactly **32** bytes (pool id). */
export declare function isNativeDexFactoryRegisterLog3Shape(log: Pick<ExecutionLog, 'topics'>): boolean;
export declare function tryParseNativeDexFactoryRegisterLog3(log: Pick<ExecutionLog, 'topics' | 'data'>): NativeDexFactoryRegisterEvent | null;
export declare function tryParseNativeDexFactoryRegisterRpcLogEntry(log: RpcLogEntry): NativeDexFactoryRegisterRpcParsed | null;
export declare function collectNativeDexFactoryRegisterLogsFromReceipt(receipt: ExecutionReceipt): NativeDexFactoryRegisterEvent[];
//# sourceMappingURL=nativeDexFactoryLogs.d.ts.map