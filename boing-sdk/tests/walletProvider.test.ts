import { describe, expect, it, vi } from 'vitest';
import {
  boingSendTransaction,
  providerSupportsBoingNativeRpc,
  readChainIdHex,
  requestAccounts,
} from '../src/walletProvider.js';
import type { Eip1193Requester } from '../src/walletProvider.js';

describe('walletProvider', () => {
  it('providerSupportsBoingNativeRpc true when boing_chainId returns hex', async () => {
    const provider: Eip1193Requester = {
      request: vi.fn().mockResolvedValue('0x1b01'),
    };
    await expect(providerSupportsBoingNativeRpc(provider)).resolves.toBe(true);
  });

  it('providerSupportsBoingNativeRpc false on method error', async () => {
    const provider: Eip1193Requester = {
      request: vi.fn().mockRejectedValue(new Error('Method not found')),
    };
    await expect(providerSupportsBoingNativeRpc(provider)).resolves.toBe(false);
  });

  it('boingSendTransaction returns string hash', async () => {
    const provider: Eip1193Requester = {
      request: vi.fn().mockResolvedValue('0xabc'),
    };
    await expect(boingSendTransaction(provider, { type: 'transfer' })).resolves.toBe('0xabc');
  });

  it('requestAccounts falls back to eth_requestAccounts', async () => {
    const provider: Eip1193Requester = {
      request: vi
        .fn()
        .mockRejectedValueOnce(new Error('no boing'))
        .mockResolvedValueOnce(['0x' + '11'.repeat(32)]),
    };
    const a = await requestAccounts(provider);
    expect(a).toEqual(['0x' + '11'.repeat(32)]);
  });

  it('readChainIdHex uses eth_chainId when boing_chainId fails', async () => {
    const provider: Eip1193Requester = {
      request: vi
        .fn()
        .mockRejectedValueOnce(new Error('no'))
        .mockResolvedValueOnce('0x1B01'),
    };
    await expect(readChainIdHex(provider)).resolves.toBe('0x1b01');
  });
});
