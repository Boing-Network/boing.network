import { describe, expect, it, vi } from 'vitest';
import type { BoingClient } from '../src/client.js';
import { findNativeDexFactoryPoolByTokens } from '../src/nativeDexFactoryPool.js';
import { senderHexFromSecretKey } from '../src/transactionBuilder.js';
import * as ed25519 from '@noble/ed25519';

describe('findNativeDexFactoryPoolByTokens', () => {
  it('returns null and skips simulate when count is zero', async () => {
    const secret = ed25519.utils.randomPrivateKey();
    const senderHex = await senderHexFromSecretKey(secret);
    const simulateTransaction = vi.fn();
    const client = {
      getAccount: vi.fn().mockResolvedValue({ balance: '0', nonce: 0, stake: '0' }),
      getContractStorage: vi.fn().mockResolvedValue({ value: '0x' + '00'.repeat(32) }),
      simulateTransaction,
    } as unknown as BoingClient;

    const pool = await findNativeDexFactoryPoolByTokens(
      client,
      '0x' + 'ee'.repeat(32),
      '0x' + 'aa'.repeat(32),
      '0x' + 'bb'.repeat(32),
      { secretKey32: secret, senderHex }
    );
    expect(pool).toBe(null);
    expect(simulateTransaction).not.toHaveBeenCalled();
  });
});
