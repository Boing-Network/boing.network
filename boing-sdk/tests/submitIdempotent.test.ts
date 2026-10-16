import { describe, expect, it, vi } from 'vitest';
import { BoingRpcError } from '../src/errors.js';
import { isAlreadyInMempoolError, submitSignedTransactionIdempotent } from '../src/submitIdempotent.js';
import { addBoingPoolLiquidity, createBoingPool, swapBoingPool } from '../src/boingPool.js';
import { deployBoingToken } from '../src/boingToken.js';
import { routeBoingSwap } from '../src/boingRouter.js';

const dup = () => new BoingRpcError(-32000, 'Duplicate transaction', { reason: 'already_in_mempool' }, 'boing_submitTransaction');
const KEY = new Uint8Array(32).fill(7);
const T1 = '0x' + '0a'.repeat(32);
const T2 = '0x' + '0b'.repeat(32);
const POOL = '0x' + '11'.repeat(32);

/** A client whose submit must never be called; reads return harmless values. */
function spyClient() {
  const submitTransaction = vi.fn(async () => ({ tx_hash: 'ok' }));
  const request = vi.fn(async () => null);
  const client = new Proxy({ submitTransaction, request } as Record<string, unknown>, {
    get(t, p) {
      if (p in t) return t[p as string];
      return vi.fn(async () => { throw new Error('unexpected RPC ' + String(p)); });
    },
  });
  return { client: client as never, submitTransaction };
}

describe('submitIdempotent', () => {
  it('detects the duplicate error by reason or message', () => {
    expect(isAlreadyInMempoolError(dup())).toBe(true);
    expect(isAlreadyInMempoolError(new BoingRpcError(-32000, 'Duplicate transaction'))).toBe(true);
    expect(isAlreadyInMempoolError(new BoingRpcError(-32000, 'insufficient balance for fee'))).toBe(false);
    expect(isAlreadyInMempoolError(new Error('Duplicate transaction'))).toBe(false);
  });
  it('treats an already-pending duplicate as submitted and rethrows other errors', async () => {
    await expect(submitSignedTransactionIdempotent({ submitTransaction: async () => { throw dup(); } } as never, '0x00')).resolves.toBe('already_pending');
    await expect(submitSignedTransactionIdempotent({ submitTransaction: async () => ({ tx_hash: 'ok' }) } as never, '0x00')).resolves.toBe('submitted');
    await expect(submitSignedTransactionIdempotent({ submitTransaction: async () => { throw new BoingRpcError(-32000, 'insufficient balance for fee'); } } as never, '0x00')).rejects.toThrow(/insufficient/);
  });
});

describe('dry runs never call boing_submitTransaction', () => {
  it('token deploy (offline nonce)', async () => {
    const { client, submitTransaction } = spyClient();
    const r = await deployBoingToken(client, KEY, { name: 'T', symbol: 'T', initialSupply: 5n, nonce: 1n, dryRun: true } as never);
    expect(submitTransaction).not.toHaveBeenCalled();
    expect(JSON.stringify(r)).toContain('signedHex');
  });
  it('pool create / add / swap and router route (offline nonce + explicit amounts)', async () => {
    const { client, submitTransaction } = spyClient();
    await createBoingPool(client, KEY, { version: 9, tokenAHex: T1, tokenBHex: T2, nonce: 3n, dryRun: true });
    await addBoingPoolLiquidity(client, KEY, { poolHex: POOL, amountA: 10n, amountB: 10n, minLiquidity: 1n, version: 9, tokenAHex: T1, tokenBHex: T2, nonce: 4n, dryRun: true });
    await swapBoingPool(client, KEY, { poolHex: POOL, direction: 'a-to-b', amountIn: 5n, minOut: 1n, version: 9, tokenAHex: T1, tokenBHex: T2, nonce: 7n, dryRun: true });
    await routeBoingSwap(client, KEY, { routerHex: '0x' + '22'.repeat(32), hops: [{ poolHex: POOL, direction: 0, tokenInHex: T1, tokenOutHex: T2 }], amountIn: 5n, minOut: 1n, nonce: 9n, dryRun: true } as never);
    expect(submitTransaction).not.toHaveBeenCalled();
  });
});
