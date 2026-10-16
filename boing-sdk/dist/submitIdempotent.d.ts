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
/** True for the node's duplicate-signed-payload error (`-32000`, reason `already_in_mempool`). */
export declare function isAlreadyInMempoolError(e: unknown): boolean;
/** Submit signed hex; returns `'submitted'` or `'already_pending'` (same bytes already in the mempool). */
export declare function submitSignedTransactionIdempotent(client: Pick<BoingClient, 'submitTransaction'>, signedHex: string): Promise<'submitted' | 'already_pending'>;
//# sourceMappingURL=submitIdempotent.d.ts.map