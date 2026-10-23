#!/usr/bin/env node
/**
 * One ingestion tick with durable JSON state (cursor + gap ranges).
 *
 * @see README.md in this package
 */
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { createClient, mergeInclusiveHeightRanges, subtractInclusiveRangeFromRanges } from '@boing-network/boing-sdk';
import { runIndexerFetchTick } from './lib/ingest-fetch-tick.mjs';

const rpc = process.env.BOING_RPC_URL ?? 'http://127.0.0.1:8545';
const statePath = process.env.BOING_OBSERVER_STATE_PATH ?? 'observer-ingest-state.json';
const chainId = process.env.BOING_CHAIN_ID ?? 'unknown';
const maxTickStr = process.env.BOING_MAX_BLOCKS_PER_TICK;
const concurrentRaw = process.env.BOING_MAX_CONCURRENT;
const omitMissing = process.env.BOING_OMIT_MISSING === '1' || process.env.BOING_OMIT_MISSING === 'true';
const writeState = process.env.BOING_WRITE_STATE !== '0' && process.env.BOING_WRITE_STATE !== 'false';
const clearFromStr = process.env.BOING_GAP_CLEAR_FROM;
const clearToStr = process.env.BOING_GAP_CLEAR_TO;
const gapClearRequested =
  clearFromStr != null &&
  clearFromStr !== '' &&
  clearToStr != null &&
  clearToStr !== '';

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

function defaultState() {
  return {
    stateVersion: 1,
    chainId,
    lastIndexedHeight: -1,
    gapRanges: [],
  };
}

function loadState() {
  if (!existsSync(statePath)) {
    return defaultState();
  }
  let raw;
  try {
    raw = JSON.parse(readFileSync(statePath, 'utf8'));
  } catch (e) {
    console.error(JSON.stringify({ ok: false, error: `invalid_json:${String(e)}` }, null, 2));
    process.exit(1);
  }
  if (raw.stateVersion !== 1 || typeof raw.chainId !== 'string' || typeof raw.lastIndexedHeight !== 'number') {
    console.error(JSON.stringify({ ok: false, error: 'invalid_state_shape' }, null, 2));
    process.exit(1);
  }
  if (!Array.isArray(raw.gapRanges)) {
    console.error(JSON.stringify({ ok: false, error: 'gapRanges_must_be_array' }, null, 2));
    process.exit(1);
  }
  for (const g of raw.gapRanges) {
    if (
      g == null ||
      typeof g.fromHeight !== 'number' ||
      typeof g.toHeight !== 'number' ||
      !Number.isInteger(g.fromHeight) ||
      !Number.isInteger(g.toHeight)
    ) {
      console.error(JSON.stringify({ ok: false, error: 'invalid_gap_range_entry' }, null, 2));
      process.exit(1);
    }
  }
  return raw;
}

function persistState(s) {
  const out = {
    stateVersion: 1,
    chainId: s.chainId,
    lastIndexedHeight: s.lastIndexedHeight,
    gapRanges: mergeInclusiveHeightRanges(s.gapRanges),
  };
  if (writeState) {
    writeFileSync(statePath, `${JSON.stringify(out, null, 2)}\n`, 'utf8');
  }
  return out;
}

async function main() {
  let state = loadState();
  if (state.chainId !== chainId && process.env.BOING_CHAIN_ID != null) {
    state = { ...state, chainId };
  }

  let gapRanges = mergeInclusiveHeightRanges(state.gapRanges);

  if (gapClearRequested) {
    const clearFrom = Number(clearFromStr);
    const clearTo = Number(clearToStr);
    if (!Number.isInteger(clearFrom) || !Number.isInteger(clearTo) || clearFrom > clearTo || clearFrom < 0) {
      console.error(
        JSON.stringify(
          {
            ok: false,
            error: 'BOING_GAP_CLEAR_FROM and BOING_GAP_CLEAR_TO must be non-negative integers with FROM <= TO',
          },
          null,
          2
        )
      );
      process.exit(1);
    }
    gapRanges = subtractInclusiveRangeFromRanges(
      { fromHeight: clearFrom, toHeight: clearTo },
      gapRanges
    );
  }

  const client = createClient(rpc);
  const { plan, bundles, nextLast, gapRanges: outGaps } = await runIndexerFetchTick(client, {
    lastIndexedHeight: state.lastIndexedHeight,
    gapRanges,
    maxBlocksPerTick,
    maxConcurrent,
    omitMissing,
  });

  if (plan == null) {
    if (gapClearRequested) {
      const persisted = persistState({ ...state, gapRanges: outGaps });
      console.log(
        JSON.stringify(
          {
            ok: true,
            rpc,
            statePath,
            writeState,
            action: 'gaps_cleared_no_fetch',
            ...persisted,
          },
          null,
          2
        )
      );
    } else {
      console.log(
        JSON.stringify(
          {
            ok: true,
            rpc,
            statePath,
            action: 'idle',
            message: 'Nothing to index (cursor at or past durable tip).',
            lastIndexedHeight: state.lastIndexedHeight,
            gapRangeCount: mergeInclusiveHeightRanges(state.gapRanges).length,
          },
          null,
          2
        )
      );
    }
    return;
  }

  let txTotal = 0;
  for (const b of bundles) {
    txTotal += b.block.transactions?.length ?? 0;
  }

  const persisted = persistState({
    ...state,
    lastIndexedHeight: nextLast,
    gapRanges: outGaps,
  });

  console.log(
    JSON.stringify(
      {
        ok: true,
        rpc,
        statePath,
        writeState,
        action: 'fetch',
        tipsSource: plan.tipsSource,
        headHeight: plan.tips.headHeight,
        durableIndexThrough: plan.tips.durableIndexThrough,
        fromHeight: plan.fromHeight,
        toHeight: plan.toHeight,
        bundleCount: bundles.length,
        transactionCount: txTotal,
        ...persisted,
      },
      null,
      2
    )
  );
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
