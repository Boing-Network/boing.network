/**
 * Idempotent submit for signed transactions.
 *
 * The public testnet gateway fans `boing_submitTransaction` out to every hosted node. The full node can accept
 * the tx and gossip it to the validator before the validator's own copy of the request arrives, so the
 * validator answers `Duplicate transaction` (`data.reason = "already_in_mempool"`) even though the tx was
 * accepted. Resubmitting the *same signed bytes* is harmless, so that error means "already submitted": treat it
 * as success instead of failing (and making callers think nothing happened).
 */
import type { BoingClient } from './client.js';
import type { SubmitTransactionResult } from './types.js';
/**
 * The tx id for a submit: the node's `tx_hash` when it is a real id, otherwise (older nodes answered
 * `"ok"`) derived locally from the signed bytes. Lower-case `0x` hex.
 */
export declare function submittedTxHash(result: Partial<SubmitTransactionResult> | null | undefined, signedHex: string): string;
/** Submit idempotently and return the tx id plus whether it was new or already pending. */
export declare function submitSignedTransactionWithHash(client: Pick<BoingClient, 'submitTransaction'>, signedHex: string): Promise<{
    status: 'submitted' | 'already_pending';
    tx_hash: string;
}>;
/** True for the node's duplicate-signed-payload error (`-32000`, reason `already_in_mempool`). */
export declare function isAlreadyInMempoolError(e: unknown): boolean;
/** Submit signed hex; returns `'submitted'` or `'already_pending'` (same bytes already in the mempool). */
export declare function submitSignedTransactionIdempotent(client: Pick<BoingClient, 'submitTransaction'>, signedHex: string): Promise<'submitted' | 'already_pending'>;
//# sourceMappingURL=submitIdempotent.d.ts.map