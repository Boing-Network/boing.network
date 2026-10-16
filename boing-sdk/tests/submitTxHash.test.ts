import { describe, expect, it, vi } from 'vitest';
import { BoingRpcError } from '../src/errors.js';
import { submittedTxHash, submitSignedTransactionWithHash } from '../src/submitIdempotent.js';
import { transactionIdFromSignedTransactionHex } from '../src/bincode.js';
import { buildTransferTransaction, senderHexFromSecretKey, signTransactionInput } from '../src/transactionBuilder.js';

const KEY = new Uint8Array(32).fill(7);

async function signed(): Promise<{ hex: string; id: string }> {
  const sender = await senderHexFromSecretKey(KEY);
  const hex = await signTransactionInput(buildTransferTransaction({ nonce: 0n, senderHex: sender, toHex: '0x' + '22'.repeat(32), amount: 5n }), KEY);
  return { hex, id: transactionIdFromSignedTransactionHex(hex).toLowerCase() };
}

describe('submittedTxHash (real hash, old-node fallback)', () => {
  it('uses the node hash when it is a real id', async () => {
    const { hex } = await signed();
    const real = '0x' + 'AB'.repeat(32);
    expect(submittedTxHash({ tx_hash: real }, hex)).toBe(real.toLowerCase());
    expect(submittedTxHash({ tx_hash: 'x', tx_id: real }, hex)).toBe(real.toLowerCase());
  });

  it('derives the id locally for legacy "ok" answers', async () => {
    const { hex, id } = await signed();
    expect(submittedTxHash({ tx_hash: 'ok' }, hex)).toBe(id);
    expect(submittedTxHash(null, hex)).toBe(id);
    expect(id).toMatch(/^0x[0-9a-f]{64}$/);
  });

  it('submitSignedTransactionWithHash: new, legacy and duplicate', async () => {
    const { hex, id } = await signed();
    const fresh = { submitTransaction: vi.fn(async () => ({ tx_hash: id, tx_id: id })) };
    expect(await submitSignedTransactionWithHash(fresh as never, hex)).toEqual({ status: 'submitted', tx_hash: id });
    const legacy = { submitTransaction: vi.fn(async () => ({ tx_hash: 'ok' })) };
    expect(await submitSignedTransactionWithHash(legacy as never, hex)).toEqual({ status: 'submitted', tx_hash: id });
    const dup = {
      submitTransaction: vi.fn(async () => {
        throw new BoingRpcError(-32000, 'Duplicate transaction', { reason: 'already_in_mempool' }, 'boing_submitTransaction');
      }),
    };
    expect(await submitSignedTransactionWithHash(dup as never, hex)).toEqual({ status: 'already_pending', tx_hash: id });
  });
});