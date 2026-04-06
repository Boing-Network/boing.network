import { describe, expect, it, vi } from 'vitest';
import {
  BOING_WALLET_RPC_METHODS_NATIVE_DAPP,
  boingSendTransaction,
  connectInjectedBoingWallet,
  explainEthSendTransactionInsufficientForBoingNativeCall,
  mapInjectedProviderErrorToUiMessage,
  providerSupportsBoingNativeRpc,
  readChainIdHex,
  requestAccounts,
} from '../src/walletProvider.js';
import type { Eip1193Requester } from '../src/walletProvider.js';

describe('walletProvider', () => {
  it('BOING_WALLET_RPC_METHODS_NATIVE_DAPP lists boing_sendTransaction', () => {
    expect(BOING_WALLET_RPC_METHODS_NATIVE_DAPP).toContain('boing_sendTransaction');
  });

  it('explainEthSendTransactionInsufficientForBoingNativeCall mentions contract_call', () => {
    expect(explainEthSendTransactionInsufficientForBoingNativeCall()).toContain('contract_call');
  });

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

  it('connectInjectedBoingWallet aggregates accounts, chain id, boing probe', async () => {
    const provider: Eip1193Requester = {
      request: vi.fn(async (args: { method: string }) => {
        const m = args.method;
        if (m === 'boing_chainId') return '0x1b01';
        if (m === 'boing_requestAccounts') throw new Error('no');
        if (m === 'eth_requestAccounts') return ['0x' + 'ee'.repeat(32)];
        if (m === 'eth_chainId') return '0x1b01';
        throw new Error(`unexpected ${m}`);
      }),
    };
    const r = await connectInjectedBoingWallet(provider);
    expect(r.chainIdHex).toBe('0x1b01');
    expect(r.accounts.length).toBe(1);
    expect(r.supportsBoingNativeRpc).toBe(true);
  });

  it('mapInjectedProviderErrorToUiMessage handles rejection and method not found', () => {
    expect(mapInjectedProviderErrorToUiMessage({ code: 4001, message: 'user rejected' })).toContain('cancelled');
    expect(mapInjectedProviderErrorToUiMessage({ message: 'method not found' })).toContain('Boing');
  });
});
