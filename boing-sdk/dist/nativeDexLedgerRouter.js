/**
 * Ledger router: forwards native CP calldata to a pool via `Call`.
 * **v1:** **128-byte** inner. **v2:** **160-byte** inner (v5 `swap_to`). **v3:** **192-byte** inner (v5 `remove_liquidity_to`).
 * Matches `boing_execution::native_dex_ledger_router`. See `docs/NATIVE-DEX-LEDGER-ROUTER.md`.
 */
import { bytesToHex, hexToBytes, validateHex32 } from './hex.js';
import { mergeAccessListWithSimulation } from './accessList.js';
/** v1 `forward_pool_call` selector (low byte of word0). */
export const SELECTOR_NATIVE_DEX_LEDGER_ROUTER_FORWARD = 0xe0;
/** v2 forward selector — **224-byte** outer calldata, **160-byte** inner. */
export const SELECTOR_NATIVE_DEX_LEDGER_ROUTER_FORWARD_V2 = 0xe1;
/** v3 forward selector — **256-byte** outer calldata, **192-byte** inner. */
export const SELECTOR_NATIVE_DEX_LEDGER_ROUTER_FORWARD_V3 = 0xe2;
function selectorWord(selector) {
    const w = new Uint8Array(32);
    w[31] = selector & 0xff;
    return w;
}
/**
 * Build **192** bytes: router selector + pool + inner pool calldata (**128** bytes).
 * `innerPoolCalldata128` must be exactly **128** bytes (same layout as native CP pool swap / add / remove).
 */
export function encodeNativeDexLedgerRouterForwardCalldata(poolHex32, innerPoolCalldata128) {
    if (innerPoolCalldata128.length !== 128) {
        throw new Error(`inner pool calldata must be 128 bytes, got ${innerPoolCalldata128.length}`);
    }
    const pool = hexToBytes(validateHex32(poolHex32));
    const out = new Uint8Array(192);
    out.set(selectorWord(SELECTOR_NATIVE_DEX_LEDGER_ROUTER_FORWARD));
    out.set(pool, 32);
    out.set(innerPoolCalldata128, 64);
    return out;
}
export function encodeNativeDexLedgerRouterForwardCalldataHex(poolHex32, innerPoolCalldata128) {
    return bytesToHex(encodeNativeDexLedgerRouterForwardCalldata(poolHex32, innerPoolCalldata128));
}
/**
 * Build **224** bytes: router selector **`0xE1`** + pool + inner pool calldata (**160** bytes, e.g. v5 `swap_to` from `encodeNativeAmmSwapToCalldata`).
 */
export function encodeNativeDexLedgerRouterForwardCalldataV2(poolHex32, innerPoolCalldata160) {
    if (innerPoolCalldata160.length !== 160) {
        throw new Error(`inner pool calldata must be 160 bytes, got ${innerPoolCalldata160.length}`);
    }
    const pool = hexToBytes(validateHex32(poolHex32));
    const out = new Uint8Array(224);
    out.set(selectorWord(SELECTOR_NATIVE_DEX_LEDGER_ROUTER_FORWARD_V2));
    out.set(pool, 32);
    out.set(innerPoolCalldata160, 64);
    return out;
}
export function encodeNativeDexLedgerRouterForwardCalldataV2Hex(poolHex32, innerPoolCalldata160) {
    return bytesToHex(encodeNativeDexLedgerRouterForwardCalldataV2(poolHex32, innerPoolCalldata160));
}
/**
 * Build **256** bytes: router selector **`0xE2`** + pool + inner pool calldata (**192** bytes, e.g. `encodeNativeAmmRemoveLiquidityToCalldata`).
 */
export function encodeNativeDexLedgerRouterForwardCalldataV3(poolHex32, innerPoolCalldata192) {
    if (innerPoolCalldata192.length !== 192) {
        throw new Error(`inner pool calldata must be 192 bytes, got ${innerPoolCalldata192.length}`);
    }
    const pool = hexToBytes(validateHex32(poolHex32));
    const out = new Uint8Array(256);
    out.set(selectorWord(SELECTOR_NATIVE_DEX_LEDGER_ROUTER_FORWARD_V3));
    out.set(pool, 32);
    out.set(innerPoolCalldata192, 64);
    return out;
}
export function encodeNativeDexLedgerRouterForwardCalldataV3Hex(poolHex32, innerPoolCalldata192) {
    return bytesToHex(encodeNativeDexLedgerRouterForwardCalldataV3(poolHex32, innerPoolCalldata192));
}
/**
 * Access list: signer, router, and **pool** (router `Call`s pool state).
 */
export function buildNativeDexLedgerRouterAccessList(senderHex32, routerHex32, poolHex32) {
    const s = validateHex32(senderHex32).toLowerCase();
    const r = validateHex32(routerHex32).toLowerCase();
    const p = validateHex32(poolHex32).toLowerCase();
    const xs = [s, r, p].sort();
    return { read: xs, write: xs };
}
export function buildNativeDexLedgerRouterContractCallTx(senderHex32, routerHex32, poolHex32, innerPoolCalldata128) {
    return {
        type: 'contract_call',
        contract: validateHex32(routerHex32).toLowerCase(),
        calldata: encodeNativeDexLedgerRouterForwardCalldataHex(poolHex32, innerPoolCalldata128),
        access_list: buildNativeDexLedgerRouterAccessList(senderHex32, routerHex32, poolHex32),
    };
}
export function mergeNativeDexLedgerRouterAccessListWithSimulation(senderHex32, routerHex32, poolHex32, sim) {
    const base = buildNativeDexLedgerRouterAccessList(senderHex32, routerHex32, poolHex32);
    return mergeAccessListWithSimulation(base.read, base.write, sim);
}
export function buildNativeDexLedgerRouterV2ContractCallTx(senderHex32, routerHex32, poolHex32, innerPoolCalldata160) {
    return {
        type: 'contract_call',
        contract: validateHex32(routerHex32).toLowerCase(),
        calldata: encodeNativeDexLedgerRouterForwardCalldataV2Hex(poolHex32, innerPoolCalldata160),
        access_list: buildNativeDexLedgerRouterAccessList(senderHex32, routerHex32, poolHex32),
    };
}
export function buildNativeDexLedgerRouterV3ContractCallTx(senderHex32, routerHex32, poolHex32, innerPoolCalldata192) {
    return {
        type: 'contract_call',
        contract: validateHex32(routerHex32).toLowerCase(),
        calldata: encodeNativeDexLedgerRouterForwardCalldataV3Hex(poolHex32, innerPoolCalldata192),
        access_list: buildNativeDexLedgerRouterAccessList(senderHex32, routerHex32, poolHex32),
    };
}
