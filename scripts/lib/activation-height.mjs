/**
 * Pure maths for proposing a storage-tree v2 activation height (see docs/STORAGE-TREE-V2-ACTIVATION.md).
 *
 * Boing testnet blocks are produced in bursts (a single validator, demand-driven), so the wall-clock
 * time per block is anything but constant. A height that is "48 h away" at the long-run average can be
 * reached in a few hours during a burst. The proposal therefore takes the larger of:
 *
 *   1. the number of blocks that makes up `hours` at the recent *average* block time, and
 *   2. the number of blocks that makes up `minBurstHours` at the *fastest* recent window,
 *
 * and rounds the result up to a multiple of `round` so operators can read it aloud.
 */

/**
 * @param {{ height: number, timestamp: number }[]} samples  Blocks sorted ascending by height (timestamps in unix seconds).
 * @returns {{ avg: number, burst: number, windowBlocks: number }}  Seconds per block.
 */
export function blockTimeStats(samples, { burstWindow = 50, targetBlockTimeSecs = 2 } = {}) {
  if (!Array.isArray(samples) || samples.length < 2) {
    throw new Error('need at least two block samples');
  }
  const first = samples[0];
  const last = samples[samples.length - 1];
  const blocks = last.height - first.height;
  if (blocks <= 0) throw new Error('samples must span at least one block');
  const floor = Math.max(targetBlockTimeSecs, 0.001);
  const avg = Math.max((last.timestamp - first.timestamp) / blocks, floor);

  // Sliding window over the samples, in block counts (samples may be a dense run or a stride).
  let burst = avg;
  for (let i = 0; i < samples.length; i += 1) {
    for (let j = i + 1; j < samples.length; j += 1) {
      const span = samples[j].height - samples[i].height;
      if (span < burstWindow) continue;
      const rate = (samples[j].timestamp - samples[i].timestamp) / span;
      if (rate < burst) burst = rate;
      break; // the first j past the window is the tightest for this i
    }
  }
  return { avg, burst: Math.max(burst, floor), windowBlocks: blocks };
}

/**
 * ETAs count from 
owUnix (default: the head block's timestamp), because an idle chain's head can be hours old.
 *
 * @param {{ head: number, headTimestamp: number, nowUnix?: number, samples: { height: number, timestamp: number }[],
 *   hours?: number, minBurstHours?: number, round?: number, burstWindow?: number, targetBlockTimeSecs?: number }} o
 */
export function computeActivationHeight(o) {
  const {
    head,
    headTimestamp,
    samples,
    hours = 48,
    minBurstHours = 2,
    round = 100,
    burstWindow = 50,
    targetBlockTimeSecs = 2,
    nowUnix = headTimestamp,
  } = o;
  if (!Number.isInteger(head) || head < 0) throw new Error('head must be a non-negative integer');
  if (!(hours > 0) || !(minBurstHours > 0)) throw new Error('hours and minBurstHours must be positive');
  const { avg, burst, windowBlocks } = blockTimeStats(samples, { burstWindow, targetBlockTimeSecs });
  const expectedBlocks = Math.ceil((hours * 3600) / avg);
  const burstFloorBlocks = Math.ceil((minBurstHours * 3600) / burst);
  const margin = Math.max(expectedBlocks, burstFloorBlocks);
  const raw = head + margin;
  const step = Math.max(1, Math.floor(round));
  const height = Math.ceil(raw / step) * step;
  const blocksAway = height - head;
  const etaBase = Math.max(nowUnix, headTimestamp);
  const eta = (secsPerBlock) => etaBase + Math.round(blocksAway * secsPerBlock);
  return {
    head,
    headTimestamp,
    hours,
    minBurstHours,
    windowBlocks,
    avgBlockSecs: avg,
    burstBlockSecs: burst,
    targetBlockSecs: targetBlockTimeSecs,
    expectedBlocks,
    burstFloorBlocks,
    binding: expectedBlocks >= burstFloorBlocks ? 'average' : 'burst-floor',
    height,
    blocksAway,
    etaUnix: {
      atAverageRate: eta(avg),
      atBurstRate: eta(burst),
      atTargetRate: eta(targetBlockTimeSecs),
    },
  };
}