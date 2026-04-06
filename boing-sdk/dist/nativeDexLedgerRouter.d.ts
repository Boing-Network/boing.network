/**
 * Ledger router: forwards native CP calldata to a pool via `Call`.
 * **v1:** **128-byte** inner. **v2:** **160-byte** inner (v5 `swap_to`). **v3:** **192-byte** inner (v5 `remove_liquidity_to`).
 * Matches `boing_execution::native_dex_ledger_router`. See `docs/NATIVE-DEX-LEDGER-ROUTER.md`.
 */
import type { SimulateResult } from './types.js';
/** v1 `forward_pool_call` selector (low byte of word0). */
export declare const SELECTOR_NATIVE_DEX_LEDGER_ROUTER_FORWARD = 224;
/** v2 forward selector — **224-byte** outer calldata, **160-byte** inner. */
export declare const SELECTOR_NATIVE_DEX_LEDGER_ROUTER_FORWARD_V2 = 225;
/** v3 forward selector — **256-byte** outer calldata, **192-byte** inner. */
export declare const SELECTOR_NATIVE_DEX_LEDGER_ROUTER_FORWARD_V3 = 226;
/**
 * Build **192** bytes: router selector + pool + inner pool calldata (**128** bytes).
 * `innerPoolCalldata128` must be exactly **128** bytes (same layout as native CP pool swap / add / remove).
 */
export declare function encodeNativeDexLedgerRouterForwardCalldata(poolHex32: string, innerPoolCalldata128: Uint8Array): Uint8Array;
export declare function encodeNativeDexLedgerRouterForwardCalldataHex(poolHex32: string, innerPoolCalldata128: Uint8Array): string;
/**
 * Build **224** bytes: router selector **`0xE1`** + pool + inner pool calldata (**160** bytes, e.g. v5 `swap_to` from `encodeNativeAmmSwapToCalldata`).
 */
export declare function encodeNativeDexLedgerRouterForwardCalldataV2(poolHex32: string, innerPoolCalldata160: Uint8Array): Uint8Array;
export declare function encodeNativeDexLedgerRouterForwardCalldataV2Hex(poolHex32: string, innerPoolCalldata160: Uint8Array): string;
/**
 * Build **256** bytes: router selector **`0xE2`** + pool + inner pool calldata (**192** bytes, e.g. `encodeNativeAmmRemoveLiquidityToCalldata`).
 */
export declare function encodeNativeDexLedgerRouterForwardCalldataV3(poolHex32: string, innerPoolCalldata192: Uint8Array): Uint8Array;
export declare function encodeNativeDexLedgerRouterForwardCalldataV3Hex(poolHex32: string, innerPoolCalldata192: Uint8Array): string;
/**
 * Access list: signer, router, and **pool** (router `Call`s pool state).
 */
export declare function buildNativeDexLedgerRouterAccessList(senderHex32: string, routerHex32: string, poolHex32: string): {
    read: string[];
    write: string[];
};
export declare function buildNativeDexLedgerRouterContractCallTx(senderHex32: string, routerHex32: string, poolHex32: string, innerPoolCalldata128: Uint8Array): {
    type: 'contract_call';
    contract: string;
    calldata: string;
    access_list: {
        read: string[];
        write: string[];
    };
};
export declare function mergeNativeDexLedgerRouterAccessListWithSimulation(senderHex32: string, routerHex32: string, poolHex32: string, sim: SimulateResult): {
    read: string[];
    write: string[];
};
export declare function buildNativeDexLedgerRouterV2ContractCallTx(senderHex32: string, routerHex32: string, poolHex32: string, innerPoolCalldata160: Uint8Array): {
    type: 'contract_call';
    contract: string;
    calldata: string;
    access_list: {
        read: string[];
        write: string[];
    };
};
export declare function buildNativeDexLedgerRouterV3ContractCallTx(senderHex32: string, routerHex32: string, poolHex32: string, innerPoolCalldata192: Uint8Array): {
    type: 'contract_call';
    contract: string;
    calldata: string;
    access_list: {
        read: string[];
        write: string[];
    };
};
//# sourceMappingURL=nativeDexLedgerRouter.d.ts.map