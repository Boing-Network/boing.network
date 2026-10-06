import { describe, expect, it } from 'vitest';
import {
  decodeReferenceNftCalldata,
  encodeReferenceMintBatchCalldata,
  encodeReferenceMintBatchCalldataHex,
  encodeReferenceTransferNftCalldata,
  MAX_REFERENCE_NFT_MINT_BATCH,
  SELECTOR_MINT_BATCH,
  SELECTOR_TRANSFER_NFT,
  referenceNftTokenIdWordFromU64,
} from '../src/referenceNft.js';
import { REFERENCE_NFT_COLLECTION_TEMPLATE_VERSION } from '../src/canonicalDeployArtifacts.js';

const to = '0x' + '11'.repeat(32);

function id(n: number): string {
  return referenceNftTokenIdWordFromU64(n);
}

function hash(n: number): string {
  // Full 32-byte word (64 hex chars): high byte 0xaa, low 8 bytes = n.
  const low = n.toString(16).padStart(16, '0');
  return ('0x' + 'aa' + '00'.repeat(23) + low) as string;
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

  it('encodes n=MAX (500) at the documented production cap', () => {
    expect(MAX_REFERENCE_NFT_MINT_BATCH).toBe(500);
    const n = MAX_REFERENCE_NFT_MINT_BATCH;
    const ids = Array.from({ length: n }, (_, i) => id(i + 1));
    const hashes = Array.from({ length: n }, (_, i) => hash(i + 1));
    const bytes = encodeReferenceMintBatchCalldata(to, ids, hashes);
    expect(bytes.length).toBe(96 + 64 * n);
    // n=500 = 0x01f4 in the low 8 bytes of word2
    expect(bytes[94]).toBe(0x01);
    expect(bytes[95]).toBe(0xf4);
  });

  it('rejects length mismatch', () => {
    expect(() => encodeReferenceMintBatchCalldata(to, [id(1), id(2)], [hash(1)])).toThrow(
      /length mismatch/,
    );
  });

  it('rejects n=0 and n>MAX', () => {
    expect(() => encodeReferenceMintBatchCalldata(to, [], [])).toThrow(/1\.\.=500/);
    const ids = Array.from({ length: 501 }, (_, i) => id(i + 1));
    const hashes = Array.from({ length: 501 }, (_, i) => hash(i + 1));
    expect(() => encodeReferenceMintBatchCalldata(to, ids, hashes)).toThrow(/1\.\.=500/);
  });

  it('template version is 3', () => {
    expect(REFERENCE_NFT_COLLECTION_TEMPLATE_VERSION).toBe('3');
  });
});

describe('decodeReferenceNftCalldata', () => {
  it('round-trips mint_batch', () => {
    const ids = [id(1), id(2)];
    const hashes = [hash(1), hash(2)];
    const bytes = encodeReferenceMintBatchCalldata(to, ids, hashes);
    const decoded = decodeReferenceNftCalldata(bytes);
    expect(decoded).toEqual({
      selector: SELECTOR_MINT_BATCH,
      to,
      n: 2,
      tokenIds: ids,
      metadataHashes: hashes,
    });
  });

  it('round-trips transfer_nft', () => {
    const tokenId = id(7);
    const bytes = encodeReferenceTransferNftCalldata(to, tokenId);
    const decoded = decodeReferenceNftCalldata(bytes);
    expect(decoded).toEqual({
      selector: SELECTOR_TRANSFER_NFT,
      to,
      tokenId,
    });
  });
});
