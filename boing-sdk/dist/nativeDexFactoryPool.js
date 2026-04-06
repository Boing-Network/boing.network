/**
 * Pair directory contract — access lists, `contract_call` payloads, storage reads.
 * Matches `boing_execution::native_dex_factory` + `native_dex_factory_count_key`.
 */
import { mergeAccessListWithSimulation } from './accessList.js';
import { decodeBoingStorageWordU128 } from './nativeAmmPool.js';
import { encodeNativeDexGetPairAtCalldata } from './nativeDexFactory.js';
import { ensureHex, validateHex32 } from './hex.js';
import { buildContractCallTransaction, fetchNextNonce, signTransactionInput, } from './transactionBuilder.js';
const HEX_RE = /^[0-9a-fA-F]+$/;
function normalizeCalldataHex(calldataHex) {
    const h = ensureHex(calldataHex.trim());
    const raw = h.slice(2);
    if (raw.length % 2 !== 0) {
        throw new Error('calldata must be even-length hex');
    }
    if (!HEX_RE.test(raw)) {
        throw new Error('calldata: invalid hex');
    }
    return `0x${raw.toLowerCase()}`;
}
/**
 * `boing_getContractStorage` key for pair count (`native_dex_factory::native_dex_factory_count_key`).
 * 16 zero bytes, **`BOINGDEX`** (8 bytes), 4 zero bytes, **`0xffffffff`** (big-endian).
 */
export const NATIVE_DEX_FACTORY_COUNT_KEY_HEX = `0x${'00'.repeat(16)}424f494e4744455800000000ffffffff`;
/** `read` / `write`: signer + directory contract (parallel scheduling minimum). */
export function buildNativeDexFactoryAccessList(senderHex32, factoryHex32) {
    const s = validateHex32(senderHex32).toLowerCase();
    const f = validateHex32(factoryHex32).toLowerCase();
    return { read: [s, f], write: [s, f] };
}
export function buildNativeDexFactoryContractCallTx(senderHex32, factoryHex32, calldataHex) {
    return {
        type: 'contract_call',
        contract: validateHex32(factoryHex32).toLowerCase(),
        calldata: normalizeCalldataHex(calldataHex),
        access_list: buildNativeDexFactoryAccessList(senderHex32, factoryHex32),
    };
}
export function mergeNativeDexFactoryAccessListWithSimulation(senderHex32, factoryHex32, sim) {
    const base = buildNativeDexFactoryAccessList(senderHex32, factoryHex32);
    return mergeAccessListWithSimulation(base.read, base.write, sim);
}
/** Decode `pairs_count` return data (32-byte word); count is in the low **8** bytes (matches on-chain layout). */
export function decodeNativeDexFactoryPairsCountReturnData(returnDataHex) {
    const w = decodeBoingStorageWordU128(ensureHex(returnDataHex));
    return w & 0xffffffffffffffffn;
}
/**
 * Decode `get_pair_at` return data (**96** bytes = three account words).
 */
export function decodeNativeDexFactoryGetPairAtReturnData(returnDataHex) {
    const raw = ensureHex(returnDataHex).slice(2).toLowerCase();
    if (!HEX_RE.test(raw)) {
        throw new Error('get_pair_at return data: invalid hex');
    }
    if (raw.length < 192) {
        throw new Error('get_pair_at return data: expected 96 bytes (192 hex chars)');
    }
    const word = (i) => validateHex32(`0x${raw.slice(i * 64, i * 64 + 64)}`);
    return {
        tokenAHex: word(0),
        tokenBHex: word(1),
        poolHex: word(2),
    };
}
/** Read registered pair count from contract storage (no simulation). */
export async function fetchNativeDexFactoryPairsCount(client, factoryHex32) {
    const f = validateHex32(factoryHex32);
    const w = await client.getContractStorage(f, NATIVE_DEX_FACTORY_COUNT_KEY_HEX);
    return decodeNativeDexFactoryPairsCountReturnData(w.value);
}
function sameAccountHex(a, b) {
    return validateHex32(a).toLowerCase() === validateHex32(b).toLowerCase();
}
/**
 * Find a registered pool address for **`(tokenA, tokenB)`** in either order.
 *
 * There is no on-chain reverse map; this helper reads **`pairs_count`** from storage, then simulates
 * **`get_pair_at(i)`** for each index until a matching triplet is found.
 *
 * Requires a signer because **`boing_simulateTransaction`** expects a signed tx envelope (same as submit).
 */
export async function findNativeDexFactoryPoolByTokens(client, factoryHex32, tokenAHex32, tokenBHex32, options) {
    const cap = BigInt(options.maxPairsToScan ?? 4096);
    const count = await fetchNativeDexFactoryPairsCount(client, factoryHex32);
    const n = count < cap ? count : cap;
    const factory = validateHex32(factoryHex32).toLowerCase();
    const sender = validateHex32(options.senderHex).toLowerCase();
    const nonce = await fetchNextNonce(client, sender);
    const al = buildNativeDexFactoryAccessList(sender, factory);
    for (let i = 0n; i < n; i++) {
        const calldata = encodeNativeDexGetPairAtCalldata(Number(i));
        const tx = buildContractCallTransaction({
            nonce,
            senderHex: sender,
            contractHex: factory,
            calldata,
            accessList: al,
        });
        const signedHex = await signTransactionInput(tx, options.secretKey32);
        const sim = await client.simulateTransaction(signedHex);
        if (!sim.success || sim.return_data == null || sim.return_data === '0x') {
            continue;
        }
        try {
            const row = decodeNativeDexFactoryGetPairAtReturnData(sim.return_data);
            const hitDirect = sameAccountHex(tokenAHex32, row.tokenAHex) && sameAccountHex(tokenBHex32, row.tokenBHex);
            const hitSwap = sameAccountHex(tokenAHex32, row.tokenBHex) && sameAccountHex(tokenBHex32, row.tokenAHex);
            if (hitDirect || hitSwap) {
                return row.poolHex.toLowerCase();
            }
        }
        catch {
            continue;
        }
    }
    return null;
}
