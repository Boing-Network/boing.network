/**
 * Native constant-product pool — `contract_call` + access list for Boing Express / JSON-RPC.
 * Matches `Transaction::suggested_parallel_access_list` for `ContractCall` when only sender + pool touch state.
 *
 * Storage layout matches `boing_execution::native_amm`:
 * reserves (`reserve_a_key` / `reserve_b_key`), total LP (`total_lp_supply_key`), per-signer LP (`lp_balance_storage_key`);
 * **v3/v4:** `swap_fee_bps_key` (`k[31] == 0x07`).
 * Amounts are u128 BE in the **low 16 bytes** of each 32-byte word.
 */
import { mergeAccessListWithSimulation } from './accessList.js';
import { ensureHex, validateHex32 } from './hex.js';
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
/** `read` and `write` both include signer + pool (parallel-scheduling minimum for pool-only bytecode). */
export function buildNativeConstantProductPoolAccessList(senderHex32, poolHex32, options) {
    const s = validateHex32(senderHex32).toLowerCase();
    const p = validateHex32(poolHex32).toLowerCase();
    const extra = options?.additionalAccountsHex32 ?? [];
    if (extra.length === 0) {
        return { read: [s, p], write: [s, p] };
    }
    const seen = new Set([s, p]);
    const sortedExtras = [];
    for (const x of extra) {
        const h = validateHex32(x).toLowerCase();
        if (!seen.has(h)) {
            seen.add(h);
            sortedExtras.push(h);
        }
    }
    sortedExtras.sort();
    const combined = [s, p, ...sortedExtras];
    return { read: combined, write: [...combined] };
}
/** Params for `boing_sendTransaction` / Express `contract_call` with explicit access list. */
export function buildNativeConstantProductContractCallTx(senderHex32, poolHex32, calldataHex, options) {
    return {
        type: 'contract_call',
        contract: validateHex32(poolHex32).toLowerCase(),
        calldata: normalizeCalldataHex(calldataHex),
        access_list: buildNativeConstantProductPoolAccessList(senderHex32, poolHex32, options),
    };
}
/**
 * Widen `read`/`write` with `sim.suggested_access_list` (e.g. after `boing_simulateTransaction`).
 */
export function mergeNativePoolAccessListWithSimulation(senderHex32, poolHex32, sim, options) {
    const base = buildNativeConstantProductPoolAccessList(senderHex32, poolHex32, options);
    return mergeAccessListWithSimulation(base.read, base.write, sim);
}
/** `boing_getContractStorage` key for reserve A (`native_amm::reserve_a_key`). */
export const NATIVE_CONSTANT_PRODUCT_RESERVE_A_KEY_HEX = `0x${'00'.repeat(31)}01`;
/** `boing_getContractStorage` key for reserve B (`native_amm::reserve_b_key`). */
export const NATIVE_CONSTANT_PRODUCT_RESERVE_B_KEY_HEX = `0x${'00'.repeat(31)}02`;
/** Total LP supply key (`native_amm::total_lp_supply_key`, `k[31] == 0x03`). */
export const NATIVE_CONSTANT_PRODUCT_TOTAL_LP_KEY_HEX = `0x${'00'.repeat(31)}03`;
/** **v2:** On-chain reference-token id for reserve side A (`token_a_key`, `k[31] == 0x04`). */
export const NATIVE_CONSTANT_PRODUCT_TOKEN_A_KEY_HEX = `0x${'00'.repeat(31)}04`;
/** **v2:** Reference-token id for side B (`k[31] == 0x05`). */
export const NATIVE_CONSTANT_PRODUCT_TOKEN_B_KEY_HEX = `0x${'00'.repeat(31)}05`;
/** **v2:** Non-zero after successful `set_tokens` (`k[31] == 0x06`). */
export const NATIVE_CONSTANT_PRODUCT_TOKENS_CONFIGURED_KEY_HEX = `0x${'00'.repeat(31)}06`;
/** **v3/v4:** Swap fee bps on output (`swap_fee_bps_key`, `k[31] == 0x07`). **`0`** = unset until first `add_liquidity` (then defaults to **`NATIVE_CP_SWAP_FEE_BPS`** on-chain). */
export const NATIVE_CONSTANT_PRODUCT_SWAP_FEE_BPS_KEY_HEX = `0x${'00'.repeat(31)}07`;
const LP_BALANCE_XOR_U8 = (() => {
    const u8 = new Uint8Array(32);
    u8.set(new TextEncoder().encode('BOING_NATIVEAMM_LPRV1'));
    return u8;
})();
/**
 * `boing_getContractStorage` key for the caller's LP balance (`native_amm::lp_balance_storage_key`).
 */
export function nativeAmmLpBalanceStorageKeyHex(senderHex32) {
    const raw = validateHex32(senderHex32).slice(2).toLowerCase();
    let out = '';
    for (let i = 0; i < 32; i++) {
        const b = parseInt(raw.slice(i * 2, i * 2 + 2), 16) ^ LP_BALANCE_XOR_U8[i];
        out += b.toString(16).padStart(2, '0');
    }
    return `0x${out}`;
}
/**
 * Decode **u128** from a 32-byte Boing contract storage word (`value` from `boing_getContractStorage`):
 * big-endian integer in the **low 16 bytes** (high 16 bytes ignored), matching reference-token amount words.
 */
export function decodeBoingStorageWordU128(valueHex) {
    const raw = ensureHex(valueHex).slice(2).toLowerCase();
    if (raw.length === 0)
        return 0n;
    if (raw.length % 2 !== 0) {
        throw new Error('storage word: hex length must be even');
    }
    if (!HEX_RE.test(raw)) {
        throw new Error('storage word: invalid hex');
    }
    const word64 = raw.length > 64 ? raw.slice(-64) : raw.padStart(64, '0');
    const low16BytesHex = word64.slice(32);
    return BigInt(`0x${low16BytesHex}`);
}
/**
 * Decode successful native pool **`add_liquidity`** contract return data: **exactly 32 bytes** (64 hex
 * chars), **u128** LP minted in the **low 16 bytes**; high 16 bytes must be zero (canonical amount word).
 */
export function decodeNativeAmmAddLiquidityReturnLpMinted(returnDataHex) {
    const raw = ensureHex(returnDataHex).slice(2).toLowerCase();
    if (raw.length % 2 !== 0) {
        throw new Error('add_liquidity return: hex length must be even');
    }
    if (raw.length !== 64) {
        throw new Error('add_liquidity return: expected exactly 32 bytes (64 hex chars)');
    }
    if (!HEX_RE.test(raw)) {
        throw new Error('add_liquidity return: invalid hex');
    }
    if (!raw.slice(0, 32).match(/^0+$/)) {
        throw new Error('add_liquidity return: high 16 bytes must be zero');
    }
    return decodeBoingStorageWordU128(`0x${raw}`);
}
/**
 * Decode native AMM `Log2` **data** (96 bytes): three u128 words (low 16 bytes of each 32-byte word), per `NATIVE-AMM-CALLDATA.md` § Logs.
 */
export function decodeNativeAmmLogDataU128Triple(dataHex) {
    const raw = ensureHex(dataHex).slice(2).toLowerCase();
    if (!HEX_RE.test(raw)) {
        throw new Error('native AMM log data: invalid hex');
    }
    if (raw.length < 192) {
        throw new Error('native AMM log data: expected at least 96 bytes (192 hex chars)');
    }
    const w = (start) => decodeBoingStorageWordU128(`0x${raw.slice(start, start + 64)}`);
    return [w(0), w(64), w(128)];
}
/**
 * Read both in-ledger reserves for the MVP constant-product pool (`Promise.all` of two storage reads).
 */
export async function fetchNativeConstantProductReserves(client, poolHex32) {
    const pool = validateHex32(poolHex32);
    const [wa, wb] = await Promise.all([
        client.getContractStorage(pool, NATIVE_CONSTANT_PRODUCT_RESERVE_A_KEY_HEX),
        client.getContractStorage(pool, NATIVE_CONSTANT_PRODUCT_RESERVE_B_KEY_HEX),
    ]);
    return {
        reserveA: decodeBoingStorageWordU128(wa.value),
        reserveB: decodeBoingStorageWordU128(wb.value),
    };
}
/** Single `boing_getContractStorage` read for **`total_lp_supply_key`**. */
export async function fetchNativeConstantProductTotalLpSupply(client, poolHex32) {
    const pool = validateHex32(poolHex32);
    const w = await client.getContractStorage(pool, NATIVE_CONSTANT_PRODUCT_TOTAL_LP_KEY_HEX);
    return decodeBoingStorageWordU128(w.value);
}
/** **v3/v4:** Raw u128 at **`swap_fee_bps_key`**. Use **`0n`** → default fee **`NATIVE_CP_SWAP_FEE_BPS`** when quoting swaps. */
export async function fetchNativeConstantProductSwapFeeBps(client, poolHex32) {
    const pool = validateHex32(poolHex32);
    const w = await client.getContractStorage(pool, NATIVE_CONSTANT_PRODUCT_SWAP_FEE_BPS_KEY_HEX);
    return decodeBoingStorageWordU128(w.value);
}
/** LP balance for **`signerHex32`** in **`poolHex32`** (XOR-derived storage key). */
export async function fetchNativeAmmSignerLpBalance(client, poolHex32, signerHex32) {
    const pool = validateHex32(poolHex32);
    const key = nativeAmmLpBalanceStorageKeyHex(signerHex32);
    const w = await client.getContractStorage(pool, key);
    return decodeBoingStorageWordU128(w.value);
}
/**
 * One round-trip batch: reserves + total LP, and optionally the given signer's LP balance (**3** or **4** parallel storage reads).
 */
export async function fetchNativeConstantProductPoolSnapshot(client, poolHex32, options) {
    const pool = validateHex32(poolHex32);
    const signer = options?.signerHex32?.trim();
    const reads = [
        client.getContractStorage(pool, NATIVE_CONSTANT_PRODUCT_RESERVE_A_KEY_HEX),
        client.getContractStorage(pool, NATIVE_CONSTANT_PRODUCT_RESERVE_B_KEY_HEX),
        client.getContractStorage(pool, NATIVE_CONSTANT_PRODUCT_TOTAL_LP_KEY_HEX),
    ];
    if (signer) {
        reads.push(client.getContractStorage(pool, nativeAmmLpBalanceStorageKeyHex(signer)));
    }
    const out = await Promise.all(reads);
    const snap = {
        reserveA: decodeBoingStorageWordU128(out[0].value),
        reserveB: decodeBoingStorageWordU128(out[1].value),
        totalLpSupply: decodeBoingStorageWordU128(out[2].value),
    };
    if (signer && out[3]) {
        snap.signerLpBalance = decodeBoingStorageWordU128(out[3].value);
    }
    return snap;
}
