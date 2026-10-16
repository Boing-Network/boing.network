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
import { BoingRpcError } from './errors.js';
import { transactionIdFromSignedTransactionHex } from './bincode.js';
import type { SubmitTransactionResult } from './types.js';

const TX_ID_RE = /^0x[0-9a-fA-F]{64}$/;

/**
 * The tx id for a submit: the node's `tx_hash` when it is a real id, otherwise (older nodes answered
 * `"ok"`) derived locally from the signed bytes. Lower-case `0x` hex.
 */
export function submittedTxHash(result: Partial<SubmitTransactionResult> | null | undefined, signedHex: string): string {
  const h = result?.tx_id ?? result?.tx_hash;
  if (typeof h === 'string' && TX_ID_RE.test(h)) return h.toLowerCase();
  return transactionIdFromSignedTransactionHex(signedHex).toLowerCase();
}

/** Submit idempotently and return the tx id plus whether it was new or already pending. */
export async function submitSignedTransactionWithHash(
  client: Pick<BoingClient, 'submitTransaction'>,
  signedHex: string,
): Promise<{ status: 'submitted' | 'already_pending'; tx_hash: string }> {
  try {
    const r = await client.submitTransaction(signedHex);
    return { status: 'submitted', tx_hash: submittedTxHash(r, signedHex) };
  } catch (e) {
    if (isAlreadyInMempoolError(e)) {
      const d = (e as BoingRpcError).data as { tx_hash?: string } | undefined;
      return { status: 'already_pending', tx_hash: submittedTxHash({ tx_hash: d?.tx_hash ?? '' }, signedHex) };
    }
    throw e;
  }
}

/** True for the node's duplicate-signed-payload error (`-32000`, reason `already_in_mempool`). */
export function isAlreadyInMempoolError(e: unknown): boolean {
  if (!(e instanceof BoingRpcError)) return false;
  const d = e.data as { reason?: unknown } | null | undefined;
  if (d && typeof d === 'object' && d.reason === 'already_in_mempool') return true;
  return e.code === -32000 && /duplicate transaction/i.test(e.message);
}

/** Submit signed hex; returns `'submitted'` or `'already_pending'` (same bytes already in the mempool). */
export async function submitSignedTransactionIdempotent(
  client: Pick<BoingClient, 'submitTransaction'>,
  signedHex: string,
): Promise<'submitted' | 'already_pending'> {
  try {
    await client.submitTransaction(signedHex);
    return 'submitted';
  } catch (e) {
    if (isAlreadyInMempoolError(e)) return 'already_pending';
    throw e;
  }
}
