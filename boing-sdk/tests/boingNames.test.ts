import { describe, expect, it } from 'vitest';
import { encodeTransactionPayload } from '../src/bincode.js';
import { bytesToHex } from '../src/hex.js';
import {
  boingNameCommitment,
  boingNameRentPerYear,
  buildNameCommitTransaction,
  buildNameRegisterTransaction,
  looksLikeBoingName,
  normalizeBoingName,
} from '../src/boingNames.js';

const A = '0x' + '11'.repeat(32);
const b = (n: number) => new Uint8Array(32).fill(n);
const hex = (u: Uint8Array) => bytesToHex(u).replace(/^0x/, '');

// Pinned in Rust: boing_primitives::names::tests::{commitment_vector, payload_bincode_vectors}
const VECTORS = [
  '0d0000003333333333333333333333333333333333333333333333333333333333333333',
  '0e0000000500000000000000616c6963652222222222222222222222222222222222222222222222222222222222222222111111111111111111111111111111111111111111111111111111111111111102',
  '0f0000000500000000000000616c69636503',
  '100000000300000000000000626f621111111111111111111111111111111111111111111111111111111111111111',
  '110000000300000000000000626f621111111111111111111111111111111111111111111111111111111111111111',
  '12000000010300000000000000626f62',
  '1200000000',
  '130000000300000000000000626f62',
];

describe('boing names', () => {
  it('normalizes like the node', () => {
    expect(normalizeBoingName(' Alice.BOING ')).toEqual({ ok: true, label: 'alice' });
    for (const bad of ['ab', 'a'.repeat(33), 'al_ice', '-ab', 'ab-', 'a--b', '0xabc', '12345', 'ab'.repeat(16), '\u0430lice']) {
      expect(normalizeBoingName(bad).ok, bad).toBe(false);
    }
    expect(looksLikeBoingName('alice.boing')).toBe(true);
    expect(looksLikeBoingName('11'.repeat(32))).toBe(false);
    expect(looksLikeBoingName('0x' + '11'.repeat(32))).toBe(false);
  });

  it('rent tiers 10/5/2/1', () => {
    expect([boingNameRentPerYear('abc'), boingNameRentPerYear('abcd'), boingNameRentPerYear('abcde'), boingNameRentPerYear('abcdef')]).toEqual([10n, 5n, 2n, 1n]);
  });

  it('commitment matches Rust', () => {
    expect(hex(boingNameCommitment('alice', A, b(0x22)))).toBe('92db9158685ad72efbc250ab5edf055da017cf0ee3819619effa69db3718bc1d');
    const tx = buildNameCommitTransaction({ nonce: 0n, senderHex: A, name: 'Alice', salt: b(0x22) });
    expect(tx.payload.kind).toBe('nameCommit');
  });

  it('payload bincode matches Rust', () => {
    const t = b(0x11);
    const got = [
      encodeTransactionPayload({ kind: 'nameCommit', commitment: b(0x33) }),
      encodeTransactionPayload({ kind: 'nameRegister', name: 'alice', salt: b(0x22), target: t, years: 2 }),
      encodeTransactionPayload({ kind: 'nameRenew', name: 'alice', years: 3 }),
      encodeTransactionPayload({ kind: 'nameTransfer', name: 'bob', newOwner: t }),
      encodeTransactionPayload({ kind: 'nameSetTarget', name: 'bob', target: t }),
      encodeTransactionPayload({ kind: 'nameSetPrimary', name: 'bob' }),
      encodeTransactionPayload({ kind: 'nameSetPrimary', name: null }),
      encodeTransactionPayload({ kind: 'nameRelease', name: 'bob' }),
    ].map(hex);
    expect(got).toEqual(VECTORS);
    const reg = buildNameRegisterTransaction({ nonce: 1n, senderHex: A, name: 'ALICE.boing', salt: b(0x22), years: 2 });
    expect(hex(encodeTransactionPayload(reg.payload))).toBe(VECTORS[1]);
    expect(() => encodeTransactionPayload({ kind: 'nameRenew', name: 'x', years: 11 })).toThrow();
  });
});