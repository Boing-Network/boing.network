import { describe, expect, it } from 'vitest';
import { CallEffectError, assertCallEffect, referenceNftOwnedBy, waitForReceipt } from '../src/callEffect.js';
import { referenceNftOwnerStorageKey, referenceNftTokenIdWordFromU64 } from '../src/referenceNft.js';
import type { ExecutionReceipt } from '../src/types.js';

const TX = '0x' + '11'.repeat(32);
const NFT = '0x' + '4e'.repeat(32);
const ALICE = '0x' + 'a1'.repeat(32);
const BOB = '0x' + 'b2'.repeat(32);
const receipt = (success: boolean): ExecutionReceipt => ({
  tx_id: TX,
  block_height: 7,
  tx_index: 0,
  success,
  gas_used: 1,
  return_data: '0x',
  logs: [],
  error: success ? null : 'InvalidJump',
});

function clock() {
  let t = 0;
  return { now: () => t, sleep: async (ms: number) => void (t += ms) };
}

describe('waitForReceipt / assertCallEffect', () => {
  it('polls until the receipt appears', async () => {
    let calls = 0;
    const client = { getTransactionReceipt: async () => (++calls < 3 ? null : receipt(true)) };
    expect((await waitForReceipt(client, TX, { ...clock(), intervalMs: 10 })).success).toBe(true);
    expect(calls).toBe(3);
  });

  it('times out', async () => {
    const client = { getTransactionReceipt: async () => null };
    await expect(waitForReceipt(client, TX, { ...clock(), timeoutMs: 50, intervalMs: 10 })).rejects.toMatchObject({
      kind: 'timeout',
    });
  });

  it('reports reverted and silent no-op calls separately', async () => {
    const failed = { getTransactionReceipt: async () => receipt(false) };
    await expect(assertCallEffect(failed, TX, () => true)).rejects.toMatchObject({ kind: 'reverted' });
    const ok = { getTransactionReceipt: async () => receipt(true) };
    const err = await assertCallEffect(ok, TX, () => false).catch((e) => e);
    expect(err).toBeInstanceOf(CallEffectError);
    expect(err.kind).toBe('no_effect');
    expect((await assertCallEffect(ok, TX, async () => true)).tx_id).toBe(TX);
  });

  it('checks reference NFT ownership through storage', async () => {
    const id = referenceNftTokenIdWordFromU64(5);
    const storage = {
      async getContractStorage(_c: string, k: string) {
        return { value: k === referenceNftOwnerStorageKey(id) ? ALICE : '0x' + '00'.repeat(32) };
      },
    };
    expect(await referenceNftOwnedBy(storage, NFT, id, ALICE)()).toBe(true);
    expect(await referenceNftOwnedBy(storage, NFT, id, BOB)()).toBe(false);
  });
});
