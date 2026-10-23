/**
 * Shared plan + fetch + gap accounting for JSON and SQLite ingest scripts.
 */
import {
  fetchBlocksWithReceiptsForHeightRange,
  nextContiguousIndexedHeightAfterOmittedFetch,
  planIndexerCatchUp,
  summarizeIndexerFetchGaps,
  unionInclusiveHeightRanges,
} from '@boing-network/boing-sdk';

/**
 * @param {import('@boing-network/boing-sdk').BoingClient} client
 * @param {object} input
 * @param {number} input.lastIndexedHeight
 * @param {readonly { fromHeight: number; toHeight: number }[]} input.gapRanges
 * @param {number | undefined} input.maxBlocksPerTick
 * @param {number} input.maxConcurrent
 * @param {boolean} input.omitMissing
 */
export async function runIndexerFetchTick(client, input) {
  const {
    lastIndexedHeight,
    gapRanges,
    maxBlocksPerTick,
    maxConcurrent,
    omitMissing,
  } = input;

  const plan = await planIndexerCatchUp(client, lastIndexedHeight, { maxBlocksPerTick });
  if (plan == null) {
    return { plan: null, bundles: [], nextLast: lastIndexedHeight, gapRanges };
  }

  const bundles = await fetchBlocksWithReceiptsForHeightRange(
    client,
    plan.fromHeight,
    plan.toHeight,
    {
      maxConcurrent,
      onMissingBlock: omitMissing ? 'omit' : 'throw',
    }
  );

  let nextLast = lastIndexedHeight;
  let outGaps = gapRanges;
  if (omitMissing) {
    const fetchGaps = summarizeIndexerFetchGaps(
      plan.fromHeight,
      plan.toHeight,
      bundles.map((b) => b.height)
    );
    nextLast = nextContiguousIndexedHeightAfterOmittedFetch(lastIndexedHeight, fetchGaps);
    outGaps = unionInclusiveHeightRanges(gapRanges, fetchGaps.missingHeightRangesInclusive);
  } else {
    nextLast = plan.toHeight;
  }

  return { plan, bundles, nextLast, gapRanges: outGaps };
}
