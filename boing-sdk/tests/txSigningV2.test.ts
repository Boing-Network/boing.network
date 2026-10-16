/**
 * Security fix #2: v2 (chain-bound) signing domain. Golden vector shared with the Rust node
 * (`boing-primitives` `v2_golden_vector`).
 */
import { describe, expect, it, afterEach } from 'vitest';
import * as ed25519 from '@noble/ed25519';
import {
  signableTransactionHash,
  signableTransactionHashV2,
  signTransactionInput,
  setDefaultSigningChainId,
  signingHashFor,
  type TransactionInput,
} from '../src/index.js';
import { bytesToHex } from '../src/hex.js';

async function fixture(): Promise<{ tx: TransactionInput; sk: Uint8Array }> {
  const sk = new Uint8Array(32).fill(7);
  const sender = await ed25519.getPublicKeyAsync(sk);
  const tx: TransactionInput = {
    nonce: 3n,
    sender,
    payload: { kind: 'transfer', to: new Uint8Array(32).fill(9), amount: 5n },
    accessList: { read: [], write: [] },
  } as TransactionInput;
  return { tx, sk };
}

afterEach(() => setDefaultSigningChainId(null));

describe('v2 signing domain', () => {
  it('matches the Rust golden vector', async () => {
    const { tx } = await fixture();
    expect(bytesToHex(signableTransactionHashV2(tx, 6913))).toBe(
      '0xd88a23702aa0c376d9e0f3b6a2c2948058fecda14b95d26843f5f902ece87899',
    );
  });

  it('binds the chain id (no cross-chain replay) and differs from legacy', async () => {
    const { tx, sk } = await fixture();
    const h6913 = signableTransactionHashV2(tx, 6913);
    const h6914 = signableTransactionHashV2(tx, 6914);
    expect(bytesToHex(h6913)).not.toBe(bytesToHex(h6914));
    expect(bytesToHex(h6913)).not.toBe(bytesToHex(signableTransactionHash(tx)));
    const sig = await ed25519.signAsync(h6913, sk);
    expect(await ed25519.verifyAsync(sig, h6913, tx.sender)).toBe(true);
    expect(await ed25519.verifyAsync(sig, h6914, tx.sender)).toBe(false);
  });

  it('signTransactionInput uses the default chain id, explicit option, or legacy', async () => {
    const { tx, sk } = await fixture();
    const legacy = await signTransactionInput(tx, sk);
    setDefaultSigningChainId(6913);
    const v2 = await signTransactionInput(tx, sk);
    expect(v2).not.toBe(legacy);
    expect(await signTransactionInput(tx, sk, { chainId: null })).toBe(legacy);
    expect(await signTransactionInput(tx, sk, { chainId: 6913 })).toBe(v2);
    expect(bytesToHex(signingHashFor(tx))).toBe(bytesToHex(signableTransactionHashV2(tx, 6913)));
  });
});
