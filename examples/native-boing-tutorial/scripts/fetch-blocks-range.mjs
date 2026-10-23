#!/usr/bin/env node
/**
 * Example: **`fetchBlocksWithReceiptsForHeightRange`** — canonical replay-style fetch
 * (`boing_getBlockByHeight(h, true)` per height, optional parallelism).
 *
 * Env:
 *   BOING_RPC_URL              — default http://127.0.0.1:8545
 *   BOING_FROM_HEIGHT          — required (integer)
 *   BOING_TO_HEIGHT            — required (integer)
 *   BOING_MAX_CONCURRENT       — optional default 1
 *   BOING_CLAMP_TO_DURABLE     — if `1` or `true`, cap `to` with getIndexerChainTips + clampIndexerHeightRange
 *   BOING_OMIT_MISSING         — if `1` or `true`, onMissingBlock: omit (else throw on null block)
 *   BOING_VERBOSE              — if `1` or `true` (or pass `--verbose`), add tx/receipt samples to JSON
 *   BOING_VERBOSE_TX_LIMIT     — max `tx_id` rows in `verbose.txIdsSample` (default 24)
 *
 * CLI: optional `--verbose` as first/any argument (in addition to env).
 */
import {
  clampIndexerHeightRange,
  createClient,
  fetchBlocksWithReceiptsForHeightRange,
  getIndexerChainTips,
} from '@boing-network/boing-sdk';

const rpc = process.env.BOING_RPC_URL ?? 'http://127.0.0.1:8545';
const fromStr = process.env.BOING_FROM_HEIGHT;
const toStr = process.env.BOING_TO_HEIGHT;
const concurrentRaw = process.env.BOING_MAX_CONCURRENT;
const clampRaw = process.env.BOING_CLAMP_TO_DURABLE;
const omitRaw = process.env.BOING_OMIT_MISSING;
const verboseRaw = process.env.BOING_VERBOSE;
const verboseLimitStr = process.env.BOING_VERBOSE_TX_LIMIT;

const argvVerbose = process.argv.slice(2).includes('--verbose');
const verbose =
  argvVerbose || verboseRaw === '1' || verboseRaw === 'true';

let verboseTxLimit = 24;
if (verboseLimitStr != null && verboseLimitStr !== '') {
  verboseTxLimit = Number(verboseLimitStr);
  if (!Number.isInteger(verboseTxLimit) || verboseTxLimit < 0) {
    console.error('BOING_VERBOSE_TX_LIMIT must be a non-negative integer.');
    process.exit(1);
  }
}

if (fromStr == null || toStr == null) {
  console.error('Set BOING_FROM_HEIGHT and BOING_TO_HEIGHT (integers).');
  process.exit(1);
}

const fromHeight = Number(fromStr);
const toHeight = Number(toStr);
if (!Number.isInteger(fromHeight) || !Number.isInteger(toHeight)) {
  console.error('BOING_FROM_HEIGHT and BOING_TO_HEIGHT must be integers.');
  process.exit(1);
}

let maxConcurrent = 1;
if (concurrentRaw != null && concurrentRaw !== '') {
  maxConcurrent = Number(concurrentRaw);
  if (!Number.isInteger(maxConcurrent) || maxConcurrent < 1) {
    console.error('BOING_MAX_CONCURRENT must be an integer >= 1.');
    process.exit(1);
  }
}

const clampToDurable = clampRaw === '1' || clampRaw === 'true';
const omitMissing = omitRaw === '1' || omitRaw === 'true';

/** @param {{ height: number; block: { transactions?: unknown[]; receipts?: unknown[]; header?: { parent_hash?: string } } }[]} bundles */
function buildVerbosePayload(bundles, limit) {
  /** @type {Array<{ height: number; txCount: number; receiptCount: number; parent_hash?: string }>} */
  const perBlock = [];
  /** @type {Array<{ height: number; tx_index: number; tx_id: string; success?: boolean }>} */
  const txIdsSample = [];

  for (const { height, block } of bundles) {
    const txs = block.transactions ?? [];
    const recs = block.receipts ?? [];
    perBlock.push({
      height,
      txCount: txs.length,
      receiptCount: recs.length,
      parent_hash: block.header?.parent_hash,
    });
    for (let i = 0; i < recs.length && txIdsSample.length < limit; i++) {
      const r = recs[i];
      if (r && typeof r === 'object' && typeof r.tx_id === 'string') {
        txIdsSample.push({
          height,
          tx_index: i,
          tx_id: r.tx_id,
          success: typeof r.success === 'boolean' ? r.success : undefined,
        });
      }
    }
  }

  return { perBlock, txIdsSample };
}

async function main() {
  const client = createClient(rpc);
  let fromH = fromHeight;
  let toH = toHeight;
  let clamped = null;

  if (clampToDurable) {
    const tips = await getIndexerChainTips(client);
    clamped = clampIndexerHeightRange(fromH, toH, tips.durableIndexThrough);
    if (clamped == null) {
      const base = {
        ok: true,
        rpc,
        clampToDurable: true,
        tips,
        clampedRange: null,
        message: 'Range empty after clamping to durableIndexThrough',
        bundles: [],
      };
      console.log(JSON.stringify(verbose ? { ...base, verbose: { perBlock: [], txIdsSample: [] } } : base, null, 2));
      return;
    }
    fromH = clamped.fromHeight;
    toH = clamped.toHeight;
  }

  const bundles = await fetchBlocksWithReceiptsForHeightRange(client, fromH, toH, {
    maxConcurrent,
    onMissingBlock: omitMissing ? 'omit' : 'throw',
  });

  const txTotal = bundles.reduce((s, b) => s + b.block.transactions.length, 0);
  const receiptSlots = bundles.reduce((s, b) => s + (b.block.receipts?.length ?? 0), 0);

  const summary = {
    ok: true,
    rpc,
    requestedFrom: fromHeight,
    requestedTo: toHeight,
    fetchedFrom: fromH,
    fetchedTo: toH,
    clampToDurable,
    clampedRange: clamped,
    maxConcurrent,
    onMissingBlock: omitMissing ? 'omit' : 'throw',
    blockCount: bundles.length,
    txTotal,
    receiptSlots,
    heights: bundles.map((b) => b.height),
    sampleHeader: bundles[0]?.block.header ?? null,
  };

  if (verbose) {
    const v = buildVerbosePayload(bundles, verboseTxLimit);
    console.log(JSON.stringify({ ...summary, verbose: v }, null, 2));
  } else {
    console.log(JSON.stringify(summary, null, 2));
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
