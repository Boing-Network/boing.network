#!/usr/bin/env node
/**
 * Demo one indexer tick: **planIndexerCatchUp** → optional **fetchBlocksWithReceiptsForHeightRange**.
 *
 * Env:
 *   BOING_RPC_URL                 — default http://127.0.0.1:8545
 *   BOING_LAST_INDEXED_HEIGHT     — default -1 (nothing indexed yet)
 *   BOING_MAX_BLOCKS_PER_TICK     — optional cap (integer >= 1)
 *   BOING_FETCH                   — if `1` or `true`, actually fetch blocks + receipts (default: plan only)
 *   BOING_MAX_CONCURRENT          — passed to fetch when BOING_FETCH (default 1)
 *   BOING_OMIT_MISSING            — if `1` or `true`, onMissingBlock: omit when fetching
 */
import {
  createClient,
  fetchBlocksWithReceiptsForHeightRange,
  nextContiguousIndexedHeightAfterOmittedFetch,
  planIndexerCatchUp,
  summarizeIndexerFetchGaps,
} from '@boing-network/boing-sdk';

const rpc = process.env.BOING_RPC_URL ?? 'http://127.0.0.1:8545';
const lastStr = process.env.BOING_LAST_INDEXED_HEIGHT ?? '-1';
const maxTickStr = process.env.BOING_MAX_BLOCKS_PER_TICK;
const fetchBlocks = process.env.BOING_FETCH === '1' || process.env.BOING_FETCH === 'true';
const concurrentRaw = process.env.BOING_MAX_CONCURRENT;
const omitMissing = process.env.BOING_OMIT_MISSING === '1' || process.env.BOING_OMIT_MISSING === 'true';

const lastIndexed = Number(lastStr);
if (!Number.isInteger(lastIndexed) || lastIndexed < -1) {
  console.error('BOING_LAST_INDEXED_HEIGHT must be an integer >= -1.');
  process.exit(1);
}

let maxBlocksPerTick;
if (maxTickStr != null && maxTickStr !== '') {
  maxBlocksPerTick = Number(maxTickStr);
  if (!Number.isInteger(maxBlocksPerTick) || maxBlocksPerTick < 1) {
    console.error('BOING_MAX_BLOCKS_PER_TICK must be an integer >= 1.');
    process.exit(1);
  }
}

let maxConcurrent = 1;
if (concurrentRaw != null && concurrentRaw !== '') {
  maxConcurrent = Number(concurrentRaw);
  if (!Number.isInteger(maxConcurrent) || maxConcurrent < 1) {
    console.error('BOING_MAX_CONCURRENT must be an integer >= 1.');
    process.exit(1);
  }
}

async function main() {
  const client = createClient(rpc);
  const plan = await planIndexerCatchUp(client, lastIndexed, {
    maxBlocksPerTick,
  });

  if (plan == null) {
    console.log(
      JSON.stringify(
        {
          ok: true,
          rpc,
          action: 'idle',
          message: 'Nothing to index (cursor at or past durable tip).',
        },
        null,
        2
      )
    );
    return;
  }

  const base = {
    ok: true,
    rpc,
    action: fetchBlocks ? 'fetch' : 'plan',
    tipsSource: plan.tipsSource,
    headHeight: plan.tips.headHeight,
    durableIndexThrough: plan.tips.durableIndexThrough,
    latestBlockHash: plan.tips.latestBlockHash,
    fromHeight: plan.fromHeight,
    toHeight: plan.toHeight,
  };

  if (!fetchBlocks) {
    console.log(JSON.stringify(base, null, 2));
    return;
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

  let txTotal = 0;
  for (const b of bundles) {
    txTotal += b.block.transactions?.length ?? 0;
  }

  const payload = {
    ...base,
    bundleCount: bundles.length,
    transactionCount: txTotal,
  };

  if (omitMissing) {
    const fetchGaps = summarizeIndexerFetchGaps(
      plan.fromHeight,
      plan.toHeight,
      bundles.map((b) => b.height)
    );
    payload.fetchGaps = fetchGaps;
    payload.suggestedNextContiguousIndexedHeight = nextContiguousIndexedHeightAfterOmittedFetch(
      lastIndexed,
      fetchGaps
    );
  }

  console.log(JSON.stringify(payload, null, 2));
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
