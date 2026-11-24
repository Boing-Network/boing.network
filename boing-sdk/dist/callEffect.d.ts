/**
 * Confirm that a submitted transaction actually did what it was meant to.
 *
 * On Boing a contract that ends with `STOP` commits, so a receipt can say `success: true` although the call
 * changed nothing (reference NFT templates v1–v3 end unauthorized transfers this way). A fault reverts and
 * reports `success: false`. Apps that act on receipts should therefore check both: the receipt succeeded
 * **and** the state they expect is now on chain. {@link assertCallEffect} does that in one call.
 */
import type { ExecutionReceipt } from './types.js';
export type CallEffectFailure = 'timeout' | 'reverted' | 'no_effect';
export declare class CallEffectError extends Error {
    readonly kind: CallEffectFailure;
    readonly receipt: ExecutionReceipt | null;
    constructor(kind: CallEffectFailure, message: string, receipt: ExecutionReceipt | null);
}
type ReceiptReader = {
    getTransactionReceipt(txIdHex: string): Promise<ExecutionReceipt | null>;
};
type StorageReader = {
    getContractStorage(contractHex: string, keyHex: string): Promise<unknown>;
};
export type WaitForReceiptOptions = {
    /** Give up after this long (default 60 s). */
    timeoutMs?: number;
    /** Delay between polls (default 1 s). */
    intervalMs?: number;
    signal?: AbortSignal;
    /** Injected for tests. */
    sleep?: (ms: number) => Promise<void>;
    now?: () => number;
};
/** Poll `boing_getTransactionReceipt` until the tx is included. Throws `CallEffectError('timeout')`. */
export declare function waitForReceipt(client: ReceiptReader, txIdHex: string, opts?: WaitForReceiptOptions): Promise<ExecutionReceipt>;
/**
 * Wait for the receipt, require `success: true`, then run `check` against current state. Returns the receipt.
 * Throws `CallEffectError` with kind `timeout`, `reverted` (receipt says failure) or `no_effect` (receipt
 * says success but `check` returned false: the call was a silent no-op).
 */
export declare function assertCallEffect(client: ReceiptReader, txIdHex: string, check: (receipt: ExecutionReceipt) => boolean | Promise<boolean>, opts?: WaitForReceiptOptions): Promise<ExecutionReceipt>;
/**
 * `check` for {@link assertCallEffect}: the reference NFT (any template version) `tokenId` is now owned by
 * `expectedOwner`. Use after `transfer_nft` or `mint_batch`.
 */
export declare function referenceNftOwnedBy(client: StorageReader, collectionHex32: string, tokenIdHex32: string, expectedOwnerHex32: string): () => Promise<boolean>;
export {};
//# sourceMappingURL=callEffect.d.ts.map