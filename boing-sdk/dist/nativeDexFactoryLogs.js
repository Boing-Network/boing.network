/**
 * Parse pair-directory **`Log3`** after `register_pair` (`topic0` + token_a + token_b, **data** = pool).
 * Matches `native_dex_factory::NATIVE_DEX_FACTORY_TOPIC_REGISTER`.
 */
import { NATIVE_DEX_FACTORY_TOPIC_REGISTER_HEX } from './nativeDexFactory.js';
import { iterReceiptLogs, normalizeTopicWord } from './receiptLogs.js';
import { validateHex32 } from './hex.js';
const T0_REG = normalizeTopicWord(NATIVE_DEX_FACTORY_TOPIC_REGISTER_HEX);
/** True if normalized `topic0` is the factory register topic. */
export function isNativeDexFactoryRegisterTopic0(topic) {
    try {
        return normalizeTopicWord(topic) === T0_REG;
    }
    catch {
        return false;
    }
}
/** `Log3` with known topic0; **data** must be exactly **32** bytes (pool id). */
export function isNativeDexFactoryRegisterLog3Shape(log) {
    if (log.topics.length !== 3)
        return false;
    return isNativeDexFactoryRegisterTopic0(log.topics[0]);
}
export function tryParseNativeDexFactoryRegisterLog3(log) {
    try {
        if (!isNativeDexFactoryRegisterLog3Shape(log))
            return null;
        const raw = log.data.trim().replace(/^0x/i, '');
        if (raw.length < 64)
            return null;
        const poolHex = validateHex32(`0x${raw.slice(0, 64)}`);
        const tokenAHex = validateHex32(log.topics[1]);
        const tokenBHex = validateHex32(log.topics[2]);
        return { tokenAHex, tokenBHex, poolHex };
    }
    catch {
        return null;
    }
}
export function tryParseNativeDexFactoryRegisterRpcLogEntry(log) {
    const p = tryParseNativeDexFactoryRegisterLog3(log);
    if (!p)
        return null;
    return {
        ...p,
        block_height: log.block_height,
        tx_index: log.tx_index,
        tx_id: log.tx_id,
        log_index: log.log_index,
        address: log.address,
    };
}
export function collectNativeDexFactoryRegisterLogsFromReceipt(receipt) {
    const out = [];
    for (const { log } of iterReceiptLogs(receipt)) {
        const p = tryParseNativeDexFactoryRegisterLog3(log);
        if (p)
            out.push(p);
    }
    return out;
}
