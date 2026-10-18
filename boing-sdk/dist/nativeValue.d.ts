/**
 * Native BOING on contract calls (native-value rules; testnet 6913 from height 1150), wrapped BOING
 * (wBOING) and one-signature native BOING swaps against v10 custody pools.
 *
 * - `contractCallWithValue` payloads move `value` base units to the contract before it runs; any failure
 *   refunds it (see `docs/NATIVE-VALUE-AND-WBOING.md`).
 * - wBOING is token-v2 compatible (balances / allowances / transfer_from), 18 decimals, 1:1 with BOING.
 * - {@link planBoingNativeSwap} builds a single atomic batch: wrap + approve + swap, or approve + swap + unwrap.
 */
import type { TransactionInput } from './bincode.js';
import { type BoingPoolVersion } from './boingPool.js';
import { WRAPPED_BOING_BYTECODE_HEX } from './defaultWrappedBoingBytecodeHex.js';
export { WRAPPED_BOING_BYTECODE_HEX };
/** Testnet (6913) activation height of the native-value rules (`BOING_TESTNET_NATIVE_VALUE_FROM`). */
export declare const BOING_TESTNET_NATIVE_VALUE_FROM = 1150;
/**
 * `true` when value-carrying calls are accepted in the block at `height` (pass tip + 1 for a new tx).
 * Testnet: from 1150. Other chains (devnets, chains without history) have the rules from genesis.
 */
export declare function nativeValueActiveAt(chainId: number | bigint | null | undefined, height: number | bigint): boolean;
export declare const WBOING_SELECTOR_DEPOSIT = 32;
export declare const WBOING_SELECTOR_WITHDRAW = 33;
export declare const WBOING_SELECTOR_WITHDRAW_TO = 34;
export declare const WBOING_SELECTOR_WITHDRAW_ALL = 35;
export declare const WBOING_SELECTOR_TOTAL_SUPPLY = 36;
export declare const WBOING_SELECTOR_BALANCE_OF = 37;
export declare const WBOING_SELECTOR_DECIMALS = 38;
export declare const WBOING_DECIMALS = 18;
/** Log topic0 of a deposit (`"BOING_WBOING_DEPOSIT"` right-padded to 32 bytes). */
export declare const WBOING_TOPIC_DEPOSIT_HEX: string;
/** Log topic0 of a withdrawal. */
export declare const WBOING_TOPIC_WITHDRAW_HEX: string;
/** Storage key of the wBOING total supply. */
export declare const WBOING_TOTAL_SUPPLY_KEY_HEX: string;
export declare const encodeWboingDepositCalldata: () => Uint8Array;
export declare const encodeWboingWithdrawCalldata: (amount: bigint) => Uint8Array;
export declare const encodeWboingWithdrawToCalldata: (amount: bigint, recipientHex: string) => Uint8Array;
export declare const encodeWboingWithdrawAllCalldata: () => Uint8Array;
export declare const encodeWboingTotalSupplyCalldata: () => Uint8Array;
export declare const encodeWboingBalanceOfCalldata: (holderHex: string) => Uint8Array;
/** `deposit` with `amount` native BOING: mints `amount` wBOING to the sender (one tx). */
export declare function buildWrapBoingTransaction(input: {
    nonce: bigint;
    senderHex: string;
    wboingHex: string;
    amount: bigint;
}): TransactionInput;
/** `withdraw(amount)` (or `withdraw_all` when `amount` is `'all'`): burns wBOING and pays native BOING. */
export declare function buildUnwrapBoingTransaction(input: {
    nonce: bigint;
    senderHex: string;
    wboingHex: string;
    amount: bigint | 'all';
}): TransactionInput;
/** `'native'` = BOING itself (wrapped / unwrapped automatically); otherwise a token id. */
export type BoingSwapAsset = 'native' | string;
export interface BoingNativeSwapPlan {
    direction: 0 | 1;
    amountIn: bigint;
    amountOut: bigint;
    minOut: bigint;
    feeBps: bigint;
    /** `'wrap'`: BOING in (deposit + approve + swap); `'unwrap'`: BOING out (approve + swap + withdraw_all). */
    mode: 'wrap' | 'unwrap';
    /** Native BOING sent with the batch (= amountIn when wrapping, else 0). */
    value: bigint;
    /** One atomic batch, one signature. */
    tx: TransactionInput;
    /** Human-readable steps (for progress UI / approval previews). */
    steps: string[];
}
/**
 * Plan a native BOING swap against a v10 custody pool whose one side is wBOING. Pure (no RPC): pass the
 * pool state (reserves, tokens, stored fee) and the sender's next nonce. The batch either wraps the input
 * (`deposit` with value, `approve(pool)`, `swap`) or unwraps the output (`approve(pool)`, `swap`,
 * `withdraw_all`). `withdraw_all` also unwraps wBOING the sender already held.
 */
export declare function planBoingNativeSwap(input: {
    poolHex: string;
    wboingHex: string;
    state: {
        reserveA: bigint;
        reserveB: bigint;
        storedFeeBps: bigint;
        tokenAHex: string;
        tokenBHex: string;
    };
    assetIn: BoingSwapAsset;
    assetOut: BoingSwapAsset;
    amountIn: bigint;
    slippageBps: bigint;
    senderHex: string;
    nonce: bigint;
    deadlineHeight?: bigint;
    poolVersion?: BoingPoolVersion;
}): BoingNativeSwapPlan;
/** Batch step of a Boing Express `batch` request (`value` in base units when non-zero). */
export type BoingExpressBatchStep = {
    type: 'transfer';
    to: string;
    amount: string;
} | {
    type: 'contract_call';
    contract: string;
    calldata: string;
    value?: string;
};
/**
 * Boing Express request object for a `contractCall`, `contractCallWithValue` or `batch` `TransactionInput`
 * (hex fields, snake_case). Value-carrying steps get `value` (decimal base units).
 */
export declare function boingTxToExpressRequest(tx: TransactionInput): {
    type: 'contract_call';
    contract: string;
    calldata: string;
    value?: string;
    access_list: {
        read: string[];
        write: string[];
    };
} | {
    type: 'batch';
    calls: BoingExpressBatchStep[];
    access_list: {
        read: string[];
        write: string[];
    };
};
//# sourceMappingURL=nativeValue.d.ts.map