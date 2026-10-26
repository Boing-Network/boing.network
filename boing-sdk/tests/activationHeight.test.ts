import { describe, expect, it } from 'vitest';
// @ts-expect-error plain ES module without types
import { blockTimeStats, computeActivationHeight } from '../../scripts/lib/activation-height.mjs';

const steady = Array.from({ length: 101 }, (_, i) => ({ height: 1000 + i, timestamp: 5000 + i * 2 }));

describe('computeActivationHeight', () => {
  it('uses the average rate for a steady 2 s chain and rounds up to 100', () => {
    const r = computeActivationHeight({ head: 1100, headTimestamp: 5200, samples: steady, hours: 48, minBurstHours: 2 });
    expect(r.avgBlockSecs).toBe(2);
    expect(r.expectedBlocks).toBe(86400);
    expect(r.binding).toBe('average');
    expect(r.height).toBe(87500);
    expect(r.height % 100).toBe(0);
    expect(r.etaUnix.atAverageRate - 5200).toBe(r.blocksAway * 2);
  });

  it('is bound by the burst floor when the chain is mostly idle but sometimes fast', () => {
    // 50 blocks in 100 s, then a 10,000 s gap, then 50 blocks in 100 s.
    const samples = [
      ...Array.from({ length: 51 }, (_, i) => ({ height: i, timestamp: i * 2 })),
      ...Array.from({ length: 51 }, (_, i) => ({ height: 51 + i, timestamp: 10_100 + i * 2 })),
    ];
    const stats = blockTimeStats(samples, { burstWindow: 50 });
    expect(stats.burst).toBeCloseTo(2, 1);
    expect(stats.avg).toBeGreaterThan(50);
    const r = computeActivationHeight({ head: 101, headTimestamp: 10_200, samples, hours: 48, minBurstHours: 2, round: 100 });
    expect(r.binding).toBe('burst-floor');
    expect(r.burstFloorBlocks).toBe(3600);
    expect(r.height).toBe(3800); // 101 + 3600 = 3701, rounded up
    // Never reachable sooner than minBurstHours even at the fastest observed rate.
    expect(r.etaUnix.atBurstRate - 10_200).toBeGreaterThanOrEqual(2 * 3600 - 200);
  });

  it('never assumes blocks faster than the target time', () => {
    const fast = Array.from({ length: 101 }, (_, i) => ({ height: i, timestamp: Math.floor(i / 10) }));
    const r = computeActivationHeight({ head: 100, headTimestamp: 10, samples: fast, targetBlockTimeSecs: 2 });
    expect(r.avgBlockSecs).toBe(2);
    expect(r.burstBlockSecs).toBe(2);
  });

  it('counts ETAs from nowUnix when the head block is old', () => {
    const r = computeActivationHeight({ head: 1100, headTimestamp: 5200, nowUnix: 9000, samples: steady });
    expect(r.etaUnix.atTargetRate).toBe(9000 + r.blocksAway * 2);
  });

  it('rejects unusable input', () => {
    expect(() => blockTimeStats([{ height: 1, timestamp: 1 }])).toThrow();
    expect(() => computeActivationHeight({ head: -1, headTimestamp: 0, samples: steady })).toThrow();
  });
});