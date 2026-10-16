import { BoingRpcError } from './errors.js';
/** True for the node's duplicate-signed-payload error (`-32000`, reason `already_in_mempool`). */
export function isAlreadyInMempoolError(e) {
    if (!(e instanceof BoingRpcError))
        return false;
    const d = e.data;
    if (d && typeof d === 'object' && d.reason === 'already_in_mempool')
        return true;
    return e.code === -32000 && /duplicate transaction/i.test(e.message);
}
/** Submit signed hex; returns `'submitted'` or `'already_pending'` (same bytes already in the mempool). */
export async function submitSignedTransactionIdempotent(client, signedHex) {
    try {
        await client.submitTransaction(signedHex);
        return 'submitted';
    }
    catch (e) {
        if (isAlreadyInMempoolError(e))
            return 'already_pending';
        throw e;
    }
}
