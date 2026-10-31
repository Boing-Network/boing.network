import { describe, expect, it, vi } from 'vitest';

vi.mock('../src/persist.js', () => {
  const rows = new Map<number, { block_hash: string }>();
  return {
    getIndexedBlockAtHeight: vi.fn(async (_db: unknown, h: number) => rows.get(h) ?? null),
    deleteHeightAndRebuild: vi.fn(),
    reconcileCursorToTip: vi.fn(),
    upsertIngestCursor: vi.fn(),
    __rows: rows,
  };
});

import { findCanonicalForkHeight } from '../src/reorg.js';
import * as persist from '../src/persist.js';

describe('findCanonicalForkHeight', () => {
  it('finds fork with far fewer RPC calls than maxSteps', async () => {
    const tip = 2000;
    const fork = 1500;
    const rows = (persist as unknown as { __rows: Map<number, { block_hash: string }> }).__rows;
    rows.clear();
    for (let h = 0; h <= tip; h++) {
      rows.set(h, { block_hash: '0x' + 'aa'.repeat(32) });
    }
    const rpcCalls: number[] = [];
    const client = {
      getBlockByHeight: async (h: number) => {
        rpcCalls.push(h);
        return { hash: h <= fork ? '0x' + 'aa'.repeat(32) : '0x' + 'bb'.repeat(32) };
      },
    };
    const { forkHeight, rpcCalls: counted } = await findCanonicalForkHeight(
      {} as D1Database,
      client as never,
      tip,
      4096
    );
    expect(forkHeight).toBe(fork);
    expect(counted).toBe(rpcCalls.length);
    // Sequential walk would need ~500 calls; logarithmic should stay well under 40.
    expect(rpcCalls.length).toBeLessThan(40);
  });
});
