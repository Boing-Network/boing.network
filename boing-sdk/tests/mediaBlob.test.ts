import { describe, expect, it } from 'vitest';
import { encodeTransactionPayload } from '../src/bincode.js';
import {
  blobIdOf,
  MAX_MEDIA_BLOB_BYTES,
  mediaBlobGas,
  mediaBlobUri,
  parseMediaBlobId,
  putBlobPayload,
  validatePutBlob,
} from '../src/mediaBlob.js';

describe('mediaBlob', () => {
  it('content-addresses and URIs round-trip', () => {
    const data = new TextEncoder().encode('hello media');
    const id = blobIdOf(data);
    expect(id.length).toBe(32);
    const uri = mediaBlobUri(id);
    expect(uri.startsWith('boing://blob/0x')).toBe(true);
    expect(parseMediaBlobId(uri)).toEqual(id);
  });

  it('validates caps and scales forever gas', () => {
    const data = new Uint8Array(64);
    validatePutBlob({ kind: 'prunable', ttlBlocks: 100n }, 'image/png', data);
    expect(mediaBlobGas(64, { kind: 'forever' })).toBe(mediaBlobGas(64, { kind: 'prunable', ttlBlocks: 1n }) * 10n);
    expect(() =>
      validatePutBlob({ kind: 'forever' }, '', new Uint8Array(MAX_MEDIA_BLOB_BYTES + 1)),
    ).toThrow(/MAX_MEDIA_BLOB_BYTES/);
  });

  it('encodes PutBlob payload discriminant 21', () => {
    const data = new Uint8Array([1, 2, 3, 4]);
    const payload = putBlobPayload({
      retention: { kind: 'forever' },
      contentType: 'image/gif',
      data,
    });
    const enc = encodeTransactionPayload(payload);
    expect(enc[0]).toBe(21);
    expect(enc[1]).toBe(0);
    expect(enc[2]).toBe(0);
    expect(enc[3]).toBe(0);
  });
});
