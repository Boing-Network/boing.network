import { describe, expect, it } from 'vitest';
import { fetchBlocksBatched } from '../src/batchFetch.js';

const OVERSIZE = 'Upstream RPC response exceeded gateway limit (6291456 bytes).';

function fakePost(oversizeHeights: Set<number>, batchTooBig = false) {
  const calls: number[][] = [];
  const post = async (body: unknown) => {
    const reqs = body as Array<{ id: number; params: [number, boolean] }>;
    calls.push(reqs.map((r) => r.params[0]));
    if (batchTooBig && reqs.some((r) => oversizeHeights.has(r.params[0]))) {
      return { jsonrpc: '2.0', id: null, error: { code: -32000, message: OVERSIZE } };
    }
    return reqs.map((r) =>
      oversizeHeights.has(r.params[0])
        ? { jsonrpc: '2.0', id: r.id, error: { code: -32000, message: OVERSIZE } }
        : { jsonrpc: '2.0', id: r.id, result: { height: r.params[0] } }
    );
  };
  const single = async (h: number) => {
    if (oversizeHeights.has(h)) throw new Error(OVERSIZE);
    return { height: h };
  };
  return { post, single, calls };
}

const range = (a: number, b: number) => Array.from({ length: b - a + 1 }, (_, i) => a + i);

describe('fetchBlocksBatched', () => {
  it('fetches in batches of at most 32 heights', async () => {
    const f = fakePost(new Set());
    const out = await fetchBlocksBatched(f.post, range(1, 70), { single: f.single });
    expect(f.calls.map((c) => c.length)).toEqual([32, 32, 6]);
    expect(out.map((b) => b.height)).toEqual(range(1, 70));
    expect(out.every((b) => b.oversize == null)).toBe(true);
  });

  it('marks per-entry oversize heights as skipped', async () => {
    const f = fakePost(new Set([5]));
    const out = await fetchBlocksBatched(f.post, range(1, 8), { single: f.single });
    expect(out.find((b) => b.height === 5)?.oversize).toContain('gateway limit');
    expect(out.filter((b) => b.oversize == null)).toHaveLength(7);
  });

  it('falls back to single fetches when the whole batch is over the cap', async () => {
    const f = fakePost(new Set([3]), true);
    const out = await fetchBlocksBatched(f.post, range(1, 4), { single: f.single });
    expect(out.map((b) => [b.height, b.oversize != null])).toEqual([
      [1, false],
      [2, false],
      [3, true],
      [4, false],
    ]);
  });

  it('treats other errors as transient (throws)', async () => {
    const post = async () => [{ jsonrpc: '2.0', id: 0, error: { code: -32603, message: 'internal' } }];
    await expect(fetchBlocksBatched(post, [1], { single: async () => null })).rejects.toThrow(/internal/);
  });

  it('omits missing blocks only when asked', async () => {
    const post = async (body: unknown) =>
      (body as Array<{ id: number }>).map((r) => ({ jsonrpc: '2.0', id: r.id, result: null }));
    await expect(fetchBlocksBatched(post, [1], { single: async () => null })).rejects.toThrow(/missing/);
    expect(await fetchBlocksBatched(post, [1], { single: async () => null, omitMissing: true })).toEqual([]);
  });
});
