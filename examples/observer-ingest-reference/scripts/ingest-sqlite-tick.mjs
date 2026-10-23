#!/usr/bin/env node
/**
 * One ingestion tick into SQLite via `node:sqlite` (Node.js 22+).
 * Applies `tools/observer-indexer-schema.sql`, persists blocks/receipts/logs, cursor, and gaps.
 */
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createClient, mergeInclusiveHeightRanges, subtractInclusiveRangeFromRanges } from '@boing-network/boing-sdk';
import { runIndexerFetchTick } from './lib/ingest-fetch-tick.mjs';
import {
  blockHashAtHeight,
  persistBlockBundle,
  replaceBlockHeightGaps,
  upsertIngestCursor,
} from './lib/sqlite-persist.mjs';

let DatabaseSync;
try {
  ({ DatabaseSync } = await import('node:sqlite'));
} catch {
  console.error(
    JSON.stringify(
      {
        ok: false,
        error: 'node_sqlite_unavailable',
        hint: 'Use Node.js 22+ with stable node:sqlite (e.g. `DatabaseSync` from node:sqlite).',
      },
      null,
      2
    )
  );
  process.exit(1);
}

const rpc = process.env.BOING_RPC_URL ?? 'http://127.0.0.1:8545';
const dbPath = process.env.BOING_SQLITE_PATH;
const chainId = process.env.BOING_CHAIN_ID ?? 'unknown';
const maxTickStr = process.env.BOING_MAX_BLOCKS_PER_TICK;
const concurrentRaw = process.env.BOING_MAX_CONCURRENT;
const omitMissing = process.env.BOING_OMIT_MISSING === '1' || process.env.BOING_OMIT_MISSING === 'true';
const writeDb = process.env.BOING_WRITE_STATE !== '0' && process.env.BOING_WRITE_STATE !== 'false';
const clearFromStr = process.env.BOING_GAP_CLEAR_FROM;
const clearToStr = process.env.BOING_GAP_CLEAR_TO;
const gapClearRequested =
  clearFromStr != null &&
  clearFromStr !== '' &&
  clearToStr != null &&
  clearToStr !== '';

const __dirname = dirname(fileURLToPath(import.meta.url));
const schemaPath = join(__dirname, '../../../tools/observer-indexer-schema.sql');

if (dbPath == null || dbPath === '') {
  console.error(JSON.stringify({ ok: false, error: 'BOING_SQLITE_PATH is required' }, null, 2));
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

function openDb() {
  const db = new DatabaseSync(dbPath);
  db.exec(readFileSync(schemaPath, 'utf8'));
  return db;
}

function loadCursorAndGaps(db) {
  const cursor = db
    .prepare('SELECT last_committed_height, last_committed_block_hash FROM ingest_cursor WHERE chain_id = ?')
    .get(chainId);
  const lastIndexedHeight = cursor?.last_committed_height ?? -1;
  const lastHash = cursor?.last_committed_block_hash ?? '0x' + '00'.repeat(32);
  const rows = db
    .prepare(
      'SELECT from_height, to_height FROM block_height_gaps WHERE chain_id = ? ORDER BY from_height ASC'
    )
    .all(chainId);
  const gapRanges = mergeInclusiveHeightRanges(
    rows.map((r) => ({ fromHeight: r.from_height, toHeight: r.to_height }))
  );
  return { lastIndexedHeight, lastHash, gapRanges };
}

async function main() {
  const db = openDb();

  let { lastIndexedHeight, lastHash, gapRanges } = loadCursorAndGaps(db);

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
    lastIndexedHeight,
    gapRanges,
    maxBlocksPerTick,
    maxConcurrent,
    omitMissing,
  });

  const nowSec = Math.floor(Date.now() / 1000);

  if (plan == null) {
    if (gapClearRequested && writeDb) {
      db.exec('BEGIN IMMEDIATE');
      try {
        replaceBlockHeightGaps(db, chainId, mergeInclusiveHeightRanges(outGaps), nowSec);
        db.exec('COMMIT');
      } catch (e) {
        db.exec('ROLLBACK');
        throw e;
      }
    }
    console.log(
      JSON.stringify(
        {
          ok: true,
          rpc,
          dbPath,
          writeDb,
          action: gapClearRequested ? 'gaps_cleared_no_fetch' : 'idle',
          message:
            plan == null && !gapClearRequested
              ? 'Nothing to index (cursor at or past durable tip).'
              : undefined,
          lastIndexedHeight,
          gapRangeCount: mergeInclusiveHeightRanges(outGaps).length,
        },
        null,
        2
      )
    );
    return;
  }

  if (writeDb) {
    db.exec('BEGIN IMMEDIATE');
    try {
      for (const b of bundles) {
        persistBlockBundle(db, b);
      }
      const tipHash = blockHashAtHeight(bundles, nextLast) ?? lastHash;
      upsertIngestCursor(db, chainId, nextLast, tipHash, nowSec);
      replaceBlockHeightGaps(db, chainId, mergeInclusiveHeightRanges(outGaps), nowSec);
      db.exec('COMMIT');
    } catch (e) {
      db.exec('ROLLBACK');
      throw e;
    }
  }

  let txTotal = 0;
  for (const b of bundles) {
    txTotal += b.block.transactions?.length ?? 0;
  }

  console.log(
    JSON.stringify(
      {
        ok: true,
        rpc,
        dbPath,
        writeDb,
        action: 'fetch',
        tipsSource: plan.tipsSource,
        headHeight: plan.tips.headHeight,
        durableIndexThrough: plan.tips.durableIndexThrough,
        fromHeight: plan.fromHeight,
        toHeight: plan.toHeight,
        bundleCount: bundles.length,
        transactionCount: txTotal,
        lastIndexedHeight: nextLast,
        gapRangeCount: mergeInclusiveHeightRanges(outGaps).length,
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
