import {
  createClient,
  fetchBlocksWithReceiptsForHeightRange,
  mergeInclusiveHeightRanges,
  nextContiguousIndexedHeightAfterOmittedFetch,
  planIndexerCatchUp,
  planIndexerChainTipsWithFallback,
  summarizeIndexerFetchGaps,
  unionInclusiveHeightRanges,
} from '@boing-network/boing-sdk';
import {
  blockHashAtHeight,
  maybeArmReadinessLagGuardD1,
  normalizeObserverBlockHash,
  persistBlockBundleD1,
  replaceBlockHeightGapsD1,
  upsertIngestCursorD1,
} from './persist-d1.js';
import { parseMaxReorgRewindSteps, rewindStaleTipIfNeeded } from './reorg-rewind.js';
import {
  computeReadinessReady,
  effectiveReadinessArmWhenLagLte,
  parseReadinessMaxLagFinalized,
  readinessFailureReasons,
} from './readiness.js';
import { DEFAULT_APP_VERSION } from './meta.js';
import {
  handleOptions,
  headersJson,
  headersText,
  jsonRes,
  parsePositiveCacheMaxAgeSec,
  type HeadersJsonOpts,
} from './cors.js';
import {
  MAX_BLOCK_SUMMARY_RANGE,
  MAX_LOG_BLOCK_SPAN,
  MAX_LOG_ROWS,
  MAX_RECENT_BLOCK_SUMMARIES,
  MAX_TX_BLOCK_SPAN,
  MAX_TX_ROWS,
  blockExistsAtHeight,
  blockExistsByHash,
  getBlockByHash,
  getBlockByHeight,
  getBlockChainTip,
  getBlockHeightGapRowsForChain,
  getBlockSummariesInRange,
  getDatabaseStats,
  getLogsByBlockHeight,
  getLogsByBlockHeightRange,
  getLogsByTxId,
  getRecentBlockSummaries,
  getReceiptByTxId,
  getReceiptsByTxIdsBulk,
  getTransactionByTxId,
  getTransactionsByBlockHeight,
  getTransactionsByBlockHeightRange,
  getTransactionsByTxIdsBulk,
  normalizeTxIdHex32,
  parseCommaSeparatedTxIds,
  parseInclusiveHeightRange,
  parseLogFilters,
  parseNonNegIntHeight,
  parsePositiveIntLimit,
  receiptExistsByTxId,
  transactionExistsByTxId,
  type ReceiptDetailRow,
  type TransactionDetailRow,
} from './read-api.js';

export interface Env {
  OBSERVER_DB: D1Database;
  BOING_RPC_URL: string;
  BOING_CHAIN_ID: string;
  /** Default 8 — keep small for cron CPU limits */
  BOING_MAX_BLOCKS_PER_TICK?: string;
  BOING_MAX_CONCURRENT?: string;
  /** `1` to use `onMissingBlock: 'omit'` (typical public RPC) */
  BOING_OMIT_MISSING?: string;
  /** Comma-separated **`Origin`** values, or omit / `*` for `Access-Control-Allow-Origin: *` */
  BOING_CORS_ORIGINS?: string;
  /** Optional display version for **`GET /api/version`** (defaults to built-in fallback). */
  BOING_APP_VERSION?: string;
  /** Max reorg rewind RPC steps per cron tick (positive integer; capped). */
  BOING_MAX_REORG_REWIND_STEPS?: string;
  /** `1` / `true` to skip reorg rewind (not recommended for production). */
  BOING_DISABLE_REORG_REWIND?: string;
  /**
   * Optional nonnegative int: if `finalized_height - last_committed_height` exceeds this,
   * **`GET /api/readiness`** returns **503** once the guard is **armed** (see **`BOING_READINESS_ARM_WHEN_LAG_LTE`**).
   */
  BOING_READINESS_MAX_LAG_FINALIZED?: string;
  /**
   * Arm the lag guard when a scheduled tick sees **`lagVsFinalized <=`** this value (default **128**).
   */
  BOING_READINESS_ARM_WHEN_LAG_LTE?: string;
  /**
   * Optional positive seconds: **`Cache-Control: public, max-age=…`** on successful
   * **`GET`/`HEAD`** for single block / tx / receipt and batch tx/receipt reads (capped at **86400** in code).
   * Omit for **`no-store`** everywhere (default). Does **not** apply to readiness, sync, lists, or logs.
   */
  BOING_READ_CACHE_MAX_AGE?: string;
}

async function loadCursorAndGaps(db: D1Database, chainId: string): Promise<{
  lastIndexedHeight: number;
  lastHash: string;
  readinessLagGuardArmed: boolean;
  gapRanges: { fromHeight: number; toHeight: number }[];
}> {
  const cursor = await db
    .prepare(
      'SELECT last_committed_height, last_committed_block_hash, COALESCE(readiness_lag_guard_armed, 0) AS readiness_lag_guard_armed FROM ingest_cursor WHERE chain_id = ?'
    )
    .bind(chainId)
    .first<{
      last_committed_height: number;
      last_committed_block_hash: string;
      readiness_lag_guard_armed: number;
    }>();

  const lastIndexedHeight = cursor?.last_committed_height ?? -1;
  const lastHash = cursor?.last_committed_block_hash ?? '0x' + '00'.repeat(32);
  const readinessLagGuardArmed = (cursor?.readiness_lag_guard_armed ?? 0) === 1;

  const { results } = await db
    .prepare(
      'SELECT from_height, to_height FROM block_height_gaps WHERE chain_id = ? ORDER BY from_height ASC'
    )
    .bind(chainId)
    .all<{ from_height: number; to_height: number }>();

  const gapRanges = mergeInclusiveHeightRanges(
    (results ?? []).map((r) => ({ fromHeight: r.from_height, toHeight: r.to_height }))
  );

  return { lastIndexedHeight, lastHash, readinessLagGuardArmed, gapRanges };
}

async function runIngest(env: Env): Promise<void> {
  const chainId = env.BOING_CHAIN_ID ?? 'unknown';
  const maxBlocksPerTick = Math.max(
    1,
    parseInt(env.BOING_MAX_BLOCKS_PER_TICK ?? '8', 10) || 8
  );
  const maxConcurrent = Math.max(1, parseInt(env.BOING_MAX_CONCURRENT ?? '1', 10) || 1);
  const omitMissing = env.BOING_OMIT_MISSING === '1' || env.BOING_OMIT_MISSING === 'true';

  const client = createClient(env.BOING_RPC_URL);
  const db = env.OBSERVER_DB;

  let { lastIndexedHeight, lastHash, gapRanges } = await loadCursorAndGaps(db, chainId);
  const nowSec = Math.floor(Date.now() / 1000);

  const disableRewind =
    env.BOING_DISABLE_REORG_REWIND === '1' || env.BOING_DISABLE_REORG_REWIND === 'true';
  const maxRewindSteps = parseMaxReorgRewindSteps(env.BOING_MAX_REORG_REWIND_STEPS);
  const afterRewind = await rewindStaleTipIfNeeded(db, client, chainId, lastIndexedHeight, lastHash, nowSec, {
    disabled: disableRewind,
    maxSteps: maxRewindSteps,
  });
  lastIndexedHeight = afterRewind.lastIndexedHeight;
  lastHash = afterRewind.lastHash;

  let plan = await planIndexerCatchUp(client, lastIndexedHeight, { maxBlocksPerTick });
  if (plan == null) {
    const maxLagFinalized = parseReadinessMaxLagFinalized(env.BOING_READINESS_MAX_LAG_FINALIZED);
    const armWhenLagLte = effectiveReadinessArmWhenLagLte(env.BOING_READINESS_ARM_WHEN_LAG_LTE);
    if (maxLagFinalized != null && lastIndexedHeight >= 0) {
      const { tips } = await planIndexerChainTipsWithFallback(client);
      const idleSec = Math.floor(Date.now() / 1000);
      const armedNow = await maybeArmReadinessLagGuardD1(
        db,
        chainId,
        lastIndexedHeight,
        tips.finalizedHeight,
        maxLagFinalized,
        armWhenLagLte,
        idleSec
      );
      if (armedNow) {
        console.log(
          JSON.stringify({
            ok: true,
            action: 'readiness_lag_guard_armed',
            chainId,
            idle: true,
            lag: tips.finalizedHeight - lastIndexedHeight,
            armWhenLagLte,
            maxLagFinalized,
          })
        );
      }
    }
    console.log(
      JSON.stringify({
        ok: true,
        action: 'idle',
        chainId,
        lastIndexedHeight,
        rewindHeights: afterRewind.rewindHeights,
      })
    );
    return;
  }

  let bundles = await fetchBlocksWithReceiptsForHeightRange(client, plan.fromHeight, plan.toHeight, {
    maxConcurrent,
    onMissingBlock: omitMissing ? 'omit' : 'throw',
  });

  let parentRetryDone = false;
  while (bundles.length > 0 && lastIndexedHeight >= 0) {
    const first = bundles[0]!;
    if (first.height !== lastIndexedHeight + 1) break;
    const parent = normalizeObserverBlockHash(first.block.header?.parent_hash);
    const expected = normalizeObserverBlockHash(lastHash);
    if (parent === expected) break;
    if (parentRetryDone || disableRewind) {
      console.log(
        JSON.stringify({
          ok: false,
          action: 'ingest_abort_parent_mismatch',
          chainId,
          blockHeight: first.height,
          expectedParent: expected,
          actualParent: parent,
        })
      );
      return;
    }
    parentRetryDone = true;
    const retrySec = Math.floor(Date.now() / 1000);
    const again = await rewindStaleTipIfNeeded(db, client, chainId, lastIndexedHeight, lastHash, retrySec, {
      disabled: false,
      maxSteps: maxRewindSteps,
    });
    lastIndexedHeight = again.lastIndexedHeight;
    lastHash = again.lastHash;
    console.log(
      JSON.stringify({
        ok: true,
        action: 'ingest_parent_mismatch_rewind_retry',
        chainId,
        rewindHeights: again.rewindHeights,
        lastIndexedHeight,
      })
    );
    plan = await planIndexerCatchUp(client, lastIndexedHeight, { maxBlocksPerTick });
    if (plan == null) {
      console.log(
        JSON.stringify({
          ok: true,
          action: 'idle_after_parent_mismatch_rewind',
          chainId,
          lastIndexedHeight,
        })
      );
      return;
    }
    bundles = await fetchBlocksWithReceiptsForHeightRange(client, plan.fromHeight, plan.toHeight, {
      maxConcurrent,
      onMissingBlock: omitMissing ? 'omit' : 'throw',
    });
  }

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

  const persistSec = Math.floor(Date.now() / 1000);

  for (const b of bundles) {
    await persistBlockBundleD1(db, b);
  }

  const tipHash = blockHashAtHeight(bundles, nextLast) ?? lastHash;
  await upsertIngestCursorD1(db, chainId, nextLast, tipHash, persistSec);
  await replaceBlockHeightGapsD1(db, chainId, mergeInclusiveHeightRanges(outGaps), persistSec);

  const maxLagFinalized = parseReadinessMaxLagFinalized(env.BOING_READINESS_MAX_LAG_FINALIZED);
  const armWhenLagLte = effectiveReadinessArmWhenLagLte(env.BOING_READINESS_ARM_WHEN_LAG_LTE);
  const armedNow = await maybeArmReadinessLagGuardD1(
    db,
    chainId,
    nextLast,
    plan.tips.finalizedHeight,
    maxLagFinalized,
    armWhenLagLte,
    persistSec
  );
  if (armedNow) {
    console.log(
      JSON.stringify({
        ok: true,
        action: 'readiness_lag_guard_armed',
        chainId,
        lag: plan.tips.finalizedHeight - nextLast,
        armWhenLagLte,
        maxLagFinalized,
      })
    );
  }

  console.log(
    JSON.stringify({
      ok: true,
      action: 'fetch',
      chainId,
      fromHeight: plan.fromHeight,
      toHeight: plan.toHeight,
      bundleCount: bundles.length,
      nextLast,
      gapRangeCount: mergeInclusiveHeightRanges(outGaps).length,
      rewindHeights: afterRewind.rewindHeights,
      tipsSource: plan.tipsSource,
      headHeight: plan.tips.headHeight,
      finalizedHeight: plan.tips.finalizedHeight,
      durableIndexThrough: plan.tips.durableIndexThrough,
    })
  );
}

export default {
  async scheduled(_event: ScheduledEvent, env: Env, _ctx: ExecutionContext): Promise<void> {
    try {
      await runIngest(env);
    } catch (e) {
      console.error('boing-observer-d1-worker scheduled error', e);
    }
  },

  async fetch(req: Request, env: Env): Promise<Response> {
    const url = new URL(req.url);
    const path = url.pathname.replace(/\/$/, '') || '/';
    const readCacheAgeSec = parsePositiveCacheMaxAgeSec(env.BOING_READ_CACHE_MAX_AGE);
    const readCacheHdr: HeadersJsonOpts | undefined =
      readCacheAgeSec != null ? { cacheMaxAgeSec: readCacheAgeSec } : undefined;

    if (req.method === 'OPTIONS') {
      return handleOptions(req, env);
    }

    if (path === '/ingest-status' || path === '/api/ingest-status') {
      if (req.method !== 'GET' && req.method !== 'HEAD') {
        return new Response('Method not allowed', { status: 405, headers: headersText(req, env) });
      }
      const chainId = url.searchParams.get('chain_id') ?? env.BOING_CHAIN_ID ?? 'unknown';
      const st = await loadCursorAndGaps(env.OBSERVER_DB, chainId);
      const body = JSON.stringify({
        ok: true,
        chainId,
        lastCommittedHeight: st.lastIndexedHeight,
        lastCommittedBlockHash: st.lastHash,
        readinessLagGuardArmed: st.readinessLagGuardArmed,
        gapRanges: st.gapRanges,
        gapRangeCount: st.gapRanges.length,
      });
      return new Response(req.method === 'HEAD' ? '' : body, { status: 200, headers: headersJson(req, env) });
    }

    if (path === '/api/version' && (req.method === 'GET' || req.method === 'HEAD')) {
      const version = env.BOING_APP_VERSION?.trim() || DEFAULT_APP_VERSION;
      const body = JSON.stringify({
        ok: true,
        service: 'boing-observer-d1-worker',
        version,
      });
      return new Response(req.method === 'HEAD' ? '' : body, { status: 200, headers: headersJson(req, env) });
    }

    if (path === '/api/stats' && (req.method === 'GET' || req.method === 'HEAD')) {
      const stats = await getDatabaseStats(env.OBSERVER_DB);
      const body = JSON.stringify({ ok: true, ...stats });
      return new Response(req.method === 'HEAD' ? '' : body, { status: 200, headers: headersJson(req, env) });
    }

    if (path === '/api/readiness' && (req.method === 'GET' || req.method === 'HEAD')) {
      const chainId = url.searchParams.get('chain_id') ?? env.BOING_CHAIN_ID ?? 'unknown';
      const maxLag = parseReadinessMaxLagFinalized(env.BOING_READINESS_MAX_LAG_FINALIZED);

      let d1Ok = false;
      let d1Error: string | undefined;
      try {
        await env.OBSERVER_DB.prepare('SELECT 1 as x').first<{ x: number }>();
        d1Ok = true;
      } catch (e) {
        d1Error = e instanceof Error ? e.message : String(e);
      }

      const client = createClient(env.BOING_RPC_URL);
      let rpcPayload:
        | {
            ok: true;
            headHeight: number;
            finalizedHeight: number;
            durableIndexThrough: number;
            latestBlockHash: string;
            source: 'sync_state' | 'chain_height';
          }
        | { ok: false; error: string };
      try {
        const { tips, tipsSource } = await planIndexerChainTipsWithFallback(client);
        rpcPayload = {
          ok: true,
          headHeight: tips.headHeight,
          finalizedHeight: tips.finalizedHeight,
          durableIndexThrough: tips.durableIndexThrough,
          latestBlockHash: tips.latestBlockHash,
          source: tipsSource,
        };
      } catch (e) {
        rpcPayload = { ok: false, error: e instanceof Error ? e.message : String(e) };
      }

      const armWhenLagLte = effectiveReadinessArmWhenLagLte(env.BOING_READINESS_ARM_WHEN_LAG_LTE);
      const st = d1Ok
        ? await loadCursorAndGaps(env.OBSERVER_DB, chainId)
        : {
            lastIndexedHeight: -1,
            lastHash: '0x' + '00'.repeat(32),
            readinessLagGuardArmed: false,
            gapRanges: [] as { fromHeight: number; toHeight: number }[],
          };

      const lagVsFinalized =
        rpcPayload.ok && Number.isFinite(rpcPayload.finalizedHeight)
          ? rpcPayload.finalizedHeight - st.lastIndexedHeight
          : null;

      const ready = computeReadinessReady({
        d1Ok,
        rpcOk: rpcPayload.ok,
        lastCommittedHeight: st.lastIndexedHeight,
        lagVsFinalized,
        maxLagFinalized: maxLag,
        readinessLagGuardArmed: st.readinessLagGuardArmed,
      });
      const reasons = readinessFailureReasons({
        d1Ok,
        rpcOk: rpcPayload.ok,
        lastCommittedHeight: st.lastIndexedHeight,
        lagVsFinalized,
        maxLagFinalized: maxLag,
        readinessLagGuardArmed: st.readinessLagGuardArmed,
      });

      const body = JSON.stringify({
        ok: ready,
        ready,
        chainId,
        checks: {
          d1: d1Ok ? { ok: true as const } : { ok: false as const, error: d1Error },
          rpc: rpcPayload,
        },
        lastCommittedHeight: st.lastIndexedHeight,
        lagVsFinalized,
        readinessMaxLagFinalized: maxLag,
        readinessArmWhenLagLte: maxLag != null ? armWhenLagLte : null,
        readinessLagGuardArmed: st.readinessLagGuardArmed,
        reasons: ready ? [] : reasons,
      });
      const status = ready ? 200 : 503;
      return new Response(req.method === 'HEAD' ? '' : body, {
        status,
        headers: headersJson(req, env),
      });
    }

    if (path === '/api/sync' && (req.method === 'GET' || req.method === 'HEAD')) {
      const chainId = url.searchParams.get('chain_id') ?? env.BOING_CHAIN_ID ?? 'unknown';
      const st = await loadCursorAndGaps(env.OBSERVER_DB, chainId);
      const client = createClient(env.BOING_RPC_URL);
      let rpc: {
        head_height: number;
        finalized_height: number;
        latest_block_hash: string;
      } | null = null;
      let rpcError: string | null = null;
      try {
        const s = await client.getSyncState();
        rpc = {
          head_height: s.head_height,
          finalized_height: s.finalized_height,
          latest_block_hash: s.latest_block_hash,
        };
      } catch (e) {
        rpcError = e instanceof Error ? e.message : String(e);
      }
      const lagVsRpcHead =
        rpc != null && Number.isFinite(rpc.head_height) ? rpc.head_height - st.lastIndexedHeight : null;
      const lagVsFinalized =
        rpc != null && Number.isFinite(rpc.finalized_height)
          ? rpc.finalized_height - st.lastIndexedHeight
          : null;
      const maxLag = parseReadinessMaxLagFinalized(env.BOING_READINESS_MAX_LAG_FINALIZED);
      const armWhenLagLte = effectiveReadinessArmWhenLagLte(env.BOING_READINESS_ARM_WHEN_LAG_LTE);
      const body = JSON.stringify({
        ok: true,
        chainId,
        rpc,
        rpcError,
        lastCommittedHeight: st.lastIndexedHeight,
        lastCommittedBlockHash: st.lastHash,
        readinessLagGuardArmed: st.readinessLagGuardArmed,
        readinessMaxLagFinalized: maxLag,
        readinessArmWhenLagLte: maxLag != null ? armWhenLagLte : null,
        gapRanges: st.gapRanges,
        gapRangeCount: st.gapRanges.length,
        lagVsRpcHead,
        lagVsFinalized,
      });
      return new Response(req.method === 'HEAD' ? '' : body, { status: 200, headers: headersJson(req, env) });
    }

    if (path === '/api/gaps' && req.method === 'GET') {
      const chainId = url.searchParams.get('chain_id') ?? env.BOING_CHAIN_ID ?? 'unknown';
      const gapRows = await getBlockHeightGapRowsForChain(env.OBSERVER_DB, chainId);
      const gapRanges = mergeInclusiveHeightRanges(
        gapRows.map((r) => ({ fromHeight: r.from_height, toHeight: r.to_height }))
      );
      return jsonRes(req, env, {
        ok: true,
        chainId,
        gapRowCount: gapRows.length,
        gapRows,
        gapRanges,
        gapRangeCount: gapRanges.length,
      });
    }

    if (path === '/api/tip' && (req.method === 'GET' || req.method === 'HEAD')) {
      const tip = await getBlockChainTip(env.OBSERVER_DB);
      if (req.method === 'HEAD') {
        return new Response(null, {
          status: tip != null ? 200 : 404,
          headers: headersJson(req, env),
        });
      }
      if (tip == null) {
        return jsonRes(req, env, { ok: true, indexed: false });
      }
      return jsonRes(req, env, {
        ok: true,
        indexed: true,
        height: tip.height,
        block_hash: tip.block_hash,
      });
    }

    if (path === '/api/block' && (req.method === 'GET' || req.method === 'HEAD')) {
      const heightRaw = url.searchParams.get('height');
      const hashRaw = url.searchParams.get('hash') ?? url.searchParams.get('block_hash');
      const db = env.OBSERVER_DB;
      const head = req.method === 'HEAD';

      if (heightRaw != null && heightRaw !== '' && hashRaw != null && hashRaw !== '') {
        return jsonRes(req, env, { ok: false, error: 'height_and_hash_mutually_exclusive' }, 400);
      }
      if (hashRaw != null && hashRaw !== '') {
        const bh = normalizeTxIdHex32(hashRaw);
        if (bh === null) {
          return jsonRes(req, env, { ok: false, error: 'missing_or_invalid_hash' }, 400);
        }
        if (head) {
          const ok = await blockExistsByHash(db, bh);
          return new Response(null, {
            status: ok ? 200 : 404,
            headers: headersJson(req, env, ok ? readCacheHdr : undefined),
          });
        }
        const row = await getBlockByHash(db, bh);
        if (!row) {
          return jsonRes(req, env, { ok: false, error: 'not_found' }, 404);
        }
        return jsonRes(req, env, { ok: true, ...row }, 200, readCacheHdr);
      }
      const h = parseNonNegIntHeight(heightRaw);
      if (h === null) {
        return jsonRes(
          req,
          env,
          { ok: false, error: 'missing_or_invalid_height', hint: 'Use height=<n> or hash=<0x+64hex>' },
          400
        );
      }
      if (head) {
        const ok = await blockExistsAtHeight(db, h);
        return new Response(null, {
          status: ok ? 200 : 404,
          headers: headersJson(req, env, ok ? readCacheHdr : undefined),
        });
      }
      const row = await getBlockByHeight(db, h);
      if (!row) {
        return jsonRes(req, env, { ok: false, error: 'not_found' }, 404);
      }
      return jsonRes(req, env, { ok: true, ...row }, 200, readCacheHdr);
    }

    if (path === '/api/transaction' && (req.method === 'GET' || req.method === 'HEAD')) {
      const txId = normalizeTxIdHex32(url.searchParams.get('tx_id'));
      if (txId === null) {
        return jsonRes(req, env, { ok: false, error: 'missing_or_invalid_tx_id' }, 400);
      }
      const db = env.OBSERVER_DB;
      if (req.method === 'HEAD') {
        const ok = await transactionExistsByTxId(db, txId);
        return new Response(null, {
          status: ok ? 200 : 404,
          headers: headersJson(req, env, ok ? readCacheHdr : undefined),
        });
      }
      const row = await getTransactionByTxId(db, txId);
      if (!row) {
        return jsonRes(req, env, { ok: false, error: 'not_found' }, 404);
      }
      return jsonRes(req, env, { ok: true, transaction: row }, 200, readCacheHdr);
    }

    if (path === '/api/receipt' && (req.method === 'GET' || req.method === 'HEAD')) {
      const txId = normalizeTxIdHex32(url.searchParams.get('tx_id'));
      if (txId === null) {
        return jsonRes(req, env, { ok: false, error: 'missing_or_invalid_tx_id' }, 400);
      }
      const db = env.OBSERVER_DB;
      if (req.method === 'HEAD') {
        const ok = await receiptExistsByTxId(db, txId);
        return new Response(null, {
          status: ok ? 200 : 404,
          headers: headersJson(req, env, ok ? readCacheHdr : undefined),
        });
      }
      const row = await getReceiptByTxId(db, txId);
      if (!row) {
        return jsonRes(req, env, { ok: false, error: 'not_found' }, 404);
      }
      return jsonRes(req, env, { ok: true, receipt: row }, 200, readCacheHdr);
    }

    if (path === '/api/transactions/batch' && req.method === 'GET') {
      const parsed = parseCommaSeparatedTxIds(url.searchParams.get('tx_ids'));
      if ('error' in parsed) {
        return jsonRes(req, env, { ok: false, error: parsed.error }, 400);
      }
      const ids = parsed.ids;
      const map = await getTransactionsByTxIdsBulk(env.OBSERVER_DB, ids);
      const transactions: TransactionDetailRow[] = [];
      const missing: string[] = [];
      for (const id of ids) {
        const row = map.get(id);
        if (row != null) transactions.push(row);
        else missing.push(id);
      }
      return jsonRes(
        req,
        env,
        {
          ok: true,
          requested: ids.length,
          found: transactions.length,
          transactions,
          missing,
        },
        200,
        readCacheHdr
      );
    }

    if (path === '/api/receipts/batch' && req.method === 'GET') {
      const parsed = parseCommaSeparatedTxIds(url.searchParams.get('tx_ids'));
      if ('error' in parsed) {
        return jsonRes(req, env, { ok: false, error: parsed.error }, 400);
      }
      const ids = parsed.ids;
      const map = await getReceiptsByTxIdsBulk(env.OBSERVER_DB, ids);
      const receipts: ReceiptDetailRow[] = [];
      const missing: string[] = [];
      for (const id of ids) {
        const row = map.get(id);
        if (row != null) receipts.push(row);
        else missing.push(id);
      }
      return jsonRes(
        req,
        env,
        {
          ok: true,
          requested: ids.length,
          found: receipts.length,
          receipts,
          missing,
        },
        200,
        readCacheHdr
      );
    }

    if (path === '/api/blocks/recent' && req.method === 'GET') {
      const lim = parsePositiveIntLimit(
        url.searchParams.get('limit'),
        16,
        MAX_RECENT_BLOCK_SUMMARIES
      );
      const blocks = await getRecentBlockSummaries(env.OBSERVER_DB, lim);
      return jsonRes(req, env, {
        ok: true,
        limit: lim,
        count: blocks.length,
        blocks,
      });
    }

    if (path === '/api/blocks' && req.method === 'GET') {
      const range = parseInclusiveHeightRange(
        url.searchParams.get('from_height'),
        url.searchParams.get('to_height'),
        MAX_BLOCK_SUMMARY_RANGE
      );
      if (range === null) {
        return jsonRes(
          req,
          env,
          {
            ok: false,
            error: 'invalid_or_too_wide_range',
            hint: `from_height and to_height required; inclusive span ≤ ${MAX_BLOCK_SUMMARY_RANGE}`,
          },
          400
        );
      }
      const blocks = await getBlockSummariesInRange(env.OBSERVER_DB, range.from, range.to);
      return jsonRes(req, env, {
        ok: true,
        fromHeight: range.from,
        toHeight: range.to,
        count: blocks.length,
        blocks,
      });
    }

    if (path === '/api/logs' && req.method === 'GET') {
      const limit = parsePositiveIntLimit(url.searchParams.get('limit'), 500, MAX_LOG_ROWS);
      const db = env.OBSERVER_DB;
      const txQ = url.searchParams.get('tx_id');
      const blockH = url.searchParams.get('block_height');
      const fromH = url.searchParams.get('from_height');
      const toH = url.searchParams.get('to_height');

      if (txQ != null && txQ !== '') {
        const txId = normalizeTxIdHex32(txQ);
        if (txId === null) {
          return jsonRes(req, env, { ok: false, error: 'invalid_tx_id' }, 400);
        }
        const ft = parseLogFilters(url.searchParams);
        if (ft.error != null) {
          return jsonRes(req, env, { ok: false, error: ft.error }, 400);
        }
        const logs = await getLogsByTxId(db, txId, limit, { address: ft.address, topics: ft.topics });
        return jsonRes(req, env, {
          ok: true,
          query: {
            tx_id: txId,
            address: ft.address,
            topic0: ft.topics[0],
            topic1: ft.topics[1],
            topic2: ft.topics[2],
            topic3: ft.topics[3],
          },
          limit,
          count: logs.length,
          logs,
        });
      }

      if (blockH != null && blockH !== '') {
        if (fromH != null || toH != null) {
          return jsonRes(req, env, { ok: false, error: 'block_height_is_exclusive_with_from_height_to_height' }, 400);
        }
        const h = parseNonNegIntHeight(blockH);
        if (h === null) {
          return jsonRes(req, env, { ok: false, error: 'invalid_block_height' }, 400);
        }
        const ft = parseLogFilters(url.searchParams);
        if (ft.error != null) {
          return jsonRes(req, env, { ok: false, error: ft.error }, 400);
        }
        const logs = await getLogsByBlockHeight(db, h, limit, ft.address, ft.topics);
        return jsonRes(req, env, {
          ok: true,
          query: {
            block_height: h,
            address: ft.address,
            topic0: ft.topics[0],
            topic1: ft.topics[1],
            topic2: ft.topics[2],
            topic3: ft.topics[3],
          },
          limit,
          count: logs.length,
          logs,
        });
      }

      if (fromH != null || toH != null) {
        const range = parseInclusiveHeightRange(fromH, toH, MAX_LOG_BLOCK_SPAN);
        if (range === null) {
          return jsonRes(
            req,
            env,
            {
              ok: false,
              error: 'invalid_or_too_wide_log_range',
              hint: `from_height and to_height required; inclusive span ≤ ${MAX_LOG_BLOCK_SPAN}`,
            },
            400
          );
        }
        const ft = parseLogFilters(url.searchParams);
        if (ft.error != null) {
          return jsonRes(req, env, { ok: false, error: ft.error }, 400);
        }
        const logs = await getLogsByBlockHeightRange(db, range.from, range.to, limit, ft.address, ft.topics);
        return jsonRes(req, env, {
          ok: true,
          query: {
            from_height: range.from,
            to_height: range.to,
            address: ft.address,
            topic0: ft.topics[0],
            topic1: ft.topics[1],
            topic2: ft.topics[2],
            topic3: ft.topics[3],
          },
          limit,
          count: logs.length,
          logs,
        });
      }

      return jsonRes(
        req,
        env,
        {
          ok: false,
          error: 'missing_query',
          hint:
            'Use tx_id=, block_height=, or from_height=+to_height=; optional address= / topic0..topic3 (or topic_0..topic_3) on block/range/tx_id',
        },
        400
      );
    }

    if (path === '/api/txs' && req.method === 'GET') {
      const limit = parsePositiveIntLimit(url.searchParams.get('limit'), 500, MAX_TX_ROWS);
      const db = env.OBSERVER_DB;
      const blockH = url.searchParams.get('block_height');
      const fromH = url.searchParams.get('from_height');
      const toH = url.searchParams.get('to_height');

      if (blockH != null && blockH !== '') {
        if (fromH != null || toH != null) {
          return jsonRes(req, env, { ok: false, error: 'block_height_is_exclusive_with_from_height_to_height' }, 400);
        }
        const h = parseNonNegIntHeight(blockH);
        if (h === null) {
          return jsonRes(req, env, { ok: false, error: 'invalid_block_height' }, 400);
        }
        const transactions = await getTransactionsByBlockHeight(db, h, limit);
        return jsonRes(req, env, {
          ok: true,
          query: { block_height: h },
          limit,
          count: transactions.length,
          transactions,
        });
      }

      if (fromH != null || toH != null) {
        const range = parseInclusiveHeightRange(fromH, toH, MAX_TX_BLOCK_SPAN);
        if (range === null) {
          return jsonRes(
            req,
            env,
            {
              ok: false,
              error: 'invalid_or_too_wide_tx_range',
              hint: `from_height and to_height required; inclusive span ≤ ${MAX_TX_BLOCK_SPAN}`,
            },
            400
          );
        }
        const transactions = await getTransactionsByBlockHeightRange(db, range.from, range.to, limit);
        return jsonRes(req, env, {
          ok: true,
          query: { from_height: range.from, to_height: range.to },
          limit,
          count: transactions.length,
          transactions,
        });
      }

      return jsonRes(
        req,
        env,
        {
          ok: false,
          error: 'missing_query',
          hint: 'Use block_height= or from_height=+to_height=',
        },
        400
      );
    }

    if (path === '/health') {
      // Liveness only — no D1/RPC. Prefer GET /api/readiness for uptime monitors; see OBSERVER-HOSTED-SERVICE.md §8.1
      return new Response('ok\n', { headers: headersText(req, env) });
    }

    if (path === '/') {
      return jsonRes(req, env, {
        service: 'boing-observer-d1-worker',
        endpoints: {
          health: '/health',
          readiness: '/api/readiness?chain_id=<optional> — D1 + RPC + optional lag vs finalized (GET/HEAD; 503 when not ready)',
          version: '/api/version',
          stats: '/api/stats',
          tip: '/api/tip — max indexed height + block_hash (GET/HEAD)',
          sync: '/api/sync?chain_id=<optional> — RPC tip vs indexer cursor + lag',
          gaps: '/api/gaps?chain_id=<optional> — pruned gap rows + merged ranges',
          ingestStatus: '/ingest-status?chain_id=<optional>',
          block: '/api/block?height=<n> | ?hash=<0x+64hex> — GET or HEAD (HEAD: 200/404, no body)',
          blocks: `/api/blocks?from_height=&to_height= (span ≤ ${MAX_BLOCK_SUMMARY_RANGE})`,
          blocksRecent: `/api/blocks/recent?limit= (default 16, max ${MAX_RECENT_BLOCK_SUMMARIES}, newest first)`,
          transaction: '/api/transaction?tx_id=<0x+64hex> (GET/HEAD)',
          transactionsBatch: `/api/transactions/batch?tx_ids=<comma-separated 0x+64hex, max 32>`,
          receiptsBatch: `/api/receipts/batch?tx_ids=<same as transactions batch>`,
          receipt: '/api/receipt?tx_id=<0x+64hex> (GET/HEAD)',
          logs: `/api/logs?tx_id= | block_height= | from_height=&to_height=; optional address= / topic0..topic3 (block/range/tx); span ≤ ${MAX_LOG_BLOCK_SPAN}; limit≤${MAX_LOG_ROWS}`,
          txs: `/api/txs?block_height= | from_height=&to_height= (span ≤ ${MAX_TX_BLOCK_SPAN}); limit≤${MAX_TX_ROWS}`,
        },
      });
    }

    return new Response('Not found', { status: 404, headers: headersText(req, env) });
  },
};
