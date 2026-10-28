import { describe, expect, it } from 'vitest';
import { isOversizeRejection, mergeSkipped, parseSkipped, planChunk, MAX_SKIPPED_RECORDS } from '../src/ingest';

const NOW = '2026-10-27T19:00:00.000Z';
const ok = (v: unknown): PromiseFulfilledResult<unknown> => ({ status: 'fulfilled', value: v });
const bad = (message: string): PromiseRejectedResult => ({ status: 'rejected', reason: Object.assign(new Error(message), { code: -32000 }) });

describe('isOversizeRejection', () => {
  it('matches the gateway size-cap errors', () => {
    expect(isOversizeRejection(new Error('Upstream RPC response exceeded gateway limit (6291456 bytes).'))).toBe(true);
    expect(isOversizeRejection(new Error('Upstream RPC response too large (9000000 bytes; gateway limit 6291456).'))).toBe(true);
  });
  it('does not match transient failures', () => {
    for (const m of [
      'HTTP 503: error code: 1102',
      'HTTP 429: rate limited',
      'Request timed out after 30000ms',
      'fetch failed',
      'Upstream RPC response truncated (10 of 20 bytes).',
      'Invalid RPC response: no result or error',
    ]) {
      expect(isOversizeRejection(new Error(m))).toBe(false);
    }
    expect(isOversizeRejection(null)).toBe(false);
    expect(isOversizeRejection('Upstream RPC response too large')).toBe(false);
  });
});

describe('planChunk', () => {
  it('advances over every fetched block', () => {
    const p = planChunk([5, 6, 7], [ok({ h: 5 }), ok(null), ok({ h: 7 })], NOW);
    expect(p.advanceBy).toBe(3);
    expect(p.blocks).toEqual([{ h: 5 }, { h: 7 }]);
    expect(p.skipped).toEqual([]);
    expect(p.stopError).toBeUndefined();
  });

  it('skips oversize heights, records them, and keeps the good blocks around them (1311..1316 shape)', () => {
    const over = 'Upstream RPC response exceeded gateway limit (6291456 bytes).';
    const p = planChunk([1311, 1312, 1313, 1314, 1315, 1316], [ok({ h: 1311 }), bad(over), bad(over), bad(over), bad(over), bad(over)], NOW);
    expect(p.advanceBy).toBe(6);
    expect(p.blocks).toEqual([{ h: 1311 }]);
    expect(p.skipped.map((s) => s.height)).toEqual([1312, 1313, 1314, 1315, 1316]);
    expect(p.skipped[0]).toMatchObject({ at: NOW });
    expect(p.stopError).toBeUndefined();
  });

  it('stops at a transient failure and does not advance past it', () => {
    const p = planChunk([10, 11, 12, 13], [ok({ h: 10 }), ok({ h: 11 }), bad('HTTP 503: boom'), ok({ h: 13 })], NOW);
    expect(p.advanceBy).toBe(2);
    expect(p.blocks).toEqual([{ h: 10 }, { h: 11 }]);
    expect((p.stopError as Error).message).toMatch(/503/);
  });

  it('keeps an earlier skip but stops at a later transient failure', () => {
    const p = planChunk(
      [1, 2, 3],
      [bad('Upstream RPC response too large (9 bytes; gateway limit 6).'), bad('Request timed out after 30000ms'), ok({ h: 3 })],
      NOW,
    );
    expect(p.advanceBy).toBe(1);
    expect(p.skipped.map((s) => s.height)).toEqual([1]);
    expect(p.stopError).toBeDefined();
  });

  it('does not advance when the first height fails transiently', () => {
    const p = planChunk([1, 2], [bad('fetch failed'), ok({ h: 2 })], NOW);
    expect(p.advanceBy).toBe(0);
    expect(p.blocks).toEqual([]);
  });
});

describe('mergeSkipped / parseSkipped', () => {
  it('dedupes by height and keeps ascending order', () => {
    const a = { height: 5, reason: 'old', at: 'a' };
    const b = { height: 3, reason: 'x', at: 'b' };
    const c = { height: 5, reason: 'new', at: 'c' };
    expect(mergeSkipped([a], [b, c])).toEqual([b, c]);
  });
  it('is bounded', () => {
    const many = Array.from({ length: MAX_SKIPPED_RECORDS + 20 }, (_, i) => ({ height: i, reason: 'r', at: 'a' }));
    const out = mergeSkipped([], many);
    expect(out).toHaveLength(MAX_SKIPPED_RECORDS);
    expect(out[out.length - 1]!.height).toBe(MAX_SKIPPED_RECORDS + 19);
  });
  it('parses defensively', () => {
    expect(parseSkipped(null)).toEqual([]);
    expect(parseSkipped('not json')).toEqual([]);
    expect(parseSkipped('{"a":1}')).toEqual([]);
    expect(parseSkipped('[{"height":1,"reason":"r","at":"t"},{"nope":1}]')).toEqual([{ height: 1, reason: 'r', at: 't' }]);
  });
});
