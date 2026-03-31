import { describe, expect, it, vi } from 'vitest';
import * as ed25519 from '@noble/ed25519';
import type { BoingClient } from '../src/client.js';
import { bytesToHex, hexToBytes } from '../src/hex.js';
import { submitTransferWithSimulationRetry } from '../src/submitFlow.js';

const GOLDEN_SEED = hexToBytes(
  '0x9d61b19deffd5a60ba844af492ec2cc44449c5697b326091a2b8c7304ee93770',
);

describe('submitTransferWithSimulationRetry', () => {
  it('widens access list and retries when simulation reports uncovered suggestion', async () => {
    const senderPub = await ed25519.getPublicKeyAsync(GOLDEN_SEED);
    const senderHex = bytesToHex(senderPub);
    const toHex = '0x' + '04'.repeat(32);

    const extra = '0x' + '11'.repeat(32);

    const simulate = vi
      .fn()
      .mockResolvedValueOnce({
        success: false,
        gas_used: 0,
        error: 'execution failed (stub)',
        access_list_covers_suggestion: false,
        suggested_access_list: { read: [senderHex, toHex, extra], write: [senderHex, toHex] },
      })
      .mockResolvedValueOnce({
        success: true,
        gas_used: 42,
        access_list_covers_suggestion: true,
        return_data: '0x',
      });

    const submit = vi.fn().mockResolvedValue({ tx_hash: '0x' + 'aa'.repeat(32) });
    const getAccount = vi.fn().mockResolvedValue({ nonce: 0, balance: '1000000', stake: '0' });

    const client = {
      simulateTransaction: simulate,
      submitTransaction: submit,
      getAccount,
    } as unknown as BoingClient;

    const out = await submitTransferWithSimulationRetry({
      client,
      secretKey32: GOLDEN_SEED,
      senderHex,
      toHex,
      amount: 3n,
      accessList: { read: [senderHex], write: [senderHex] },
    });

    expect(simulate).toHaveBeenCalledTimes(2);
    expect(submit).toHaveBeenCalledTimes(1);
    expect(out.attempts).toBe(2);
    expect(out.tx_hash).toMatch(/^0x/);
    expect(out.lastSimulation.success).toBe(true);
  });
});
