/**
 * LP share token calldata (minter-gated `mint` + `transfer`) + Express / JSON-RPC access lists.
 * Matches `boing_execution::native_lp_share_token`.
 * See `docs/NATIVE-LP-SHARE-TOKEN.md`.
 */
import type { SimulateResult } from './types.js';
/** `transfer(to, amount)` — **96** bytes. */
export declare const SELECTOR_LP_SHARE_TRANSFER = 1;
/** `mint(to, amount)` — **96** bytes; only minter may call. */
export declare const SELECTOR_LP_SHARE_MINT = 6;
/** `set_minter_once(minter)` — **64** bytes. */
export declare const SELECTOR_LP_SHARE_SET_MINTER_ONCE = 7;
/** Storage key for minter slot (`k[31] == 0xb1`), 32-byte word. */
export declare const LP_SHARE_MINTER_KEY_U8: Uint8Array;
export declare function encodeLpShareTransferCalldata(toHex32: string, amount: bigint): Uint8Array;
export declare function encodeLpShareTransferCalldataHex(toHex32: string, amount: bigint): string;
export declare function encodeLpShareMintCalldata(toHex32: string, amount: bigint): Uint8Array;
export declare function encodeLpShareMintCalldataHex(toHex32: string, amount: bigint): string;
export declare function encodeLpShareSetMinterOnceCalldata(minterHex32: string): Uint8Array;
export declare function encodeLpShareSetMinterOnceCalldataHex(minterHex32: string): string;
/** `read` / `write`: signer + share token contract (parallel-scheduling minimum). */
export declare function buildLpShareTokenAccessList(senderHex32: string, shareTokenHex32: string): {
    read: string[];
    write: string[];
};
export declare function buildLpShareTokenContractCallTx(senderHex32: string, shareTokenHex32: string, calldataHex: string): {
    type: 'contract_call';
    contract: string;
    calldata: string;
    access_list: {
        read: string[];
        write: string[];
    };
};
export declare function mergeLpShareTokenAccessListWithSimulation(senderHex32: string, shareTokenHex32: string, sim: SimulateResult): {
    read: string[];
    write: string[];
};
//# sourceMappingURL=nativeLpShareToken.d.ts.map