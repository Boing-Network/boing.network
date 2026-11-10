import { describe, expect, it } from 'vitest';
import { cursorTargetHeight, isOversizeRejection } from '../src/oversize.js';

describe('oversize handling', () => {
  it('matches only explicit gateway size-cap rejections', () => {
    expect(isOversizeRejection(new Error('Upstream RPC response exceeded gateway limit (6291456 bytes). Run your own node for blob-sized blocks.'))).toBe(true);
    expect(isOversizeRejection(new Error('Upstream RPC response too large (9000000 bytes; gateway limit 6291456).'))).toBe(true);
    expect(isOversizeRejection(new Error('Upstream RPC response truncated'))).toBe(false);
    expect(isOversizeRejection(new Error('HTTP 503'))).toBe(false);
    expect(isOversizeRejection(null)).toBe(false);
  });

  it('lands the cursor on the last real (non-skipped) block', () => {
    const hashes = new Map([
      [10, 'a'],
      [11, 'b'],
      [14, 'e'],
    ]);
    expect(cursorTargetHeight(10, 15, hashes)).toBe(14);
    expect(cursorTargetHeight(12, 13, hashes)).toBeNull();
    expect(cursorTargetHeight(10, 11, hashes)).toBe(11);
  });
});
