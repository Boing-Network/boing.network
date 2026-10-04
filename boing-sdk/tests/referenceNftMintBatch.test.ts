import { describe, expect, it } from 'vitest';
import {
  encodeReferenceMintBatchCalldata,
  encodeReferenceMintBatchCalldataHex,
  MAX_REFERENCE_NFT_MINT_BATCH,
  SELECTOR_MINT_BATCH,
  referenceNftTokenIdWordFromU64,
} from '../src/referenceNft.js';
import { REFERENCE_NFT_COLLECTION_TEMPLATE_VERSION } from '../src/canonicalDeployArtifacts.js';

const to = '0x' + '11'.repeat(32);

function id(n: number): string {
  return referenceNftTokenIdWordFromU64(n);
}

function hash(n: number): string {
  return '0x' + n.toString(16).padStart(2, '0') + 'aa'.repeat(31);
}

describe('encodeReferenceMintBatchCalldata', () => {
  it('encodes n=2 layout (96+64n, selector 0x06)', () => {
    const ids = [id(1), id(2)];
    const hashes = [hash(1), hash(2)];
    const bytes = encodeReferenceMintBatchCalldata(to, ids, hashes);
    expect(bytes.length).toBe(96 + 64 * 2);
    expect(bytes[31]).toBe(SELECTOR_MINT_BATCH);
    expect(bytes[95]).toBe(2);
    expect(encodeReferenceMintBatchCalldataHex(to, ids, hashes).startsWith('0x')).toBe(true);
  });

  it('encodes n=MAX (50) at the documented production cap', () => {
    expect(MAX_REFERENCE_NFT_MINT_BATCH).toBe(50);
    const n = MAX_REFERENCE_NFT_MINT_BATCH;
    const ids = Array.from({ length: n }, (_, i) => id(i + 1));
    const hashes = Array.from({ length: n }, (_, i) => hash(i + 1));
    const bytes = encodeReferenceMintBatchCalldata(to, ids, hashes);
    expect(bytes.length).toBe(96 + 64 * n);
    expect(bytes[95]).toBe(n);
  });

  it('rejects length mismatch', () => {
    expect(() => encodeReferenceMintBatchCalldata(to, [id(1), id(2)], [hash(1)])).toThrow(
      /length mismatch/,
    );
  });

  it('rejects n=0 and n>MAX', () => {
    expect(() => encodeReferenceMintBatchCalldata(to, [], [])).toThrow(/1\.\.=50/);
    const ids = Array.from({ length: 51 }, (_, i) => id(i + 1));
    const hashes = Array.from({ length: 51 }, (_, i) => hash(i + 1));
    expect(() => encodeReferenceMintBatchCalldata(to, ids, hashes)).toThrow(/1\.\.=50/);
  });

  it('template version is 2', () => {
    expect(REFERENCE_NFT_COLLECTION_TEMPLATE_VERSION).toBe('2');
  });
});
