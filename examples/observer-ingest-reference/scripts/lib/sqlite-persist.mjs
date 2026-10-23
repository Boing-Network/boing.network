/**
 * Persist fetched block bundles to SQLite (node:sqlite DatabaseSync).
 */

/** @param {string | undefined} h */
function normHash(h) {
  if (h == null || h === '') return '0x' + '00'.repeat(32);
  const s = String(h).trim();
  if (s.startsWith('0x') || s.startsWith('0X')) return s.toLowerCase();
  if (/^[0-9a-fA-F]{64}$/.test(s)) return `0x${s.toLowerCase()}`;
  return s;
}

/**
 * @param {import('node:sqlite').DatabaseSync} db
 * @param {import('@boing-network/boing-sdk').BlockWithReceiptsBundle} bundle
 */
export function persistBlockBundle(db, bundle) {
  const { height, block } = bundle;
  const hash = normHash(block.hash);
  const parent = normHash(block.header?.parent_hash);
  const blockJson = JSON.stringify(block);

  db.prepare('DELETE FROM logs WHERE block_height = ?').run(height);
  const txIds = db
    .prepare('SELECT tx_id FROM transactions WHERE block_height = ?')
    .all(height)
    .map((r) => r.tx_id);
  for (const tid of txIds) {
    db.prepare('DELETE FROM receipts WHERE tx_id = ?').run(tid);
  }
  db.prepare('DELETE FROM transactions WHERE block_height = ?').run(height);

  db.prepare(
    `INSERT OR REPLACE INTO blocks (height, block_hash, parent_hash, block_json)
     VALUES (?, ?, ?, ?)`
  ).run(height, hash, parent, blockJson);

  const txs = Array.isArray(block.transactions) ? block.transactions : [];
  const receipts = Array.isArray(block.receipts) ? block.receipts : [];

  for (let i = 0; i < txs.length; i++) {
    const tx = txs[i];
    const rec = receipts[i] ?? null;
    const txId = resolveTxId(rec, tx);
    if (!txId) continue;

    const rawHex = typeof tx === 'string' ? tx : JSON.stringify(tx);
    let payloadKind = null;
    if (tx && typeof tx === 'object') {
      const keys = Object.keys(tx);
      if (keys.length === 1) payloadKind = keys[0] ?? null;
    }

    let sender = null;
    if (tx && typeof tx === 'object' && tx.Transfer != null && typeof tx.Transfer === 'object') {
      const from = tx.Transfer.from;
      if (typeof from === 'string') sender = from;
    }

    db.prepare(
      `INSERT OR REPLACE INTO transactions (tx_id, block_height, tx_index, sender, payload_kind, raw_hex)
       VALUES (?, ?, ?, ?, ?, ?)`
    ).run(txId, height, i, sender, payloadKind, rawHex);

    if (rec && typeof rec === 'object' && typeof rec.tx_id === 'string') {
      db.prepare(
        `INSERT OR REPLACE INTO receipts (tx_id, success, gas_used, return_data, error)
         VALUES (?, ?, ?, ?, ?)`
      ).run(
        rec.tx_id,
        rec.success ? 1 : 0,
        String(rec.gas_used ?? 0),
        rec.return_data != null ? String(rec.return_data) : null,
        rec.error != null ? String(rec.error) : null
      );

      const logs = Array.isArray(rec.logs) ? rec.logs : [];
      for (let li = 0; li < logs.length; li++) {
        const log = logs[li];
        if (!log || typeof log !== 'object') continue;
        const topics = Array.isArray(log.topics) ? log.topics : [];
        db.prepare(
          `INSERT OR REPLACE INTO logs (tx_id, log_index, block_height, address, topics_json, data_hex)
           VALUES (?, ?, ?, ?, ?, ?)`
        ).run(
          rec.tx_id,
          li,
          height,
          null,
          JSON.stringify(topics.map((t) => String(t))),
          log.data != null ? String(log.data) : '0x'
        );
      }
    }
  }

}

/** @param {unknown} rec @param {unknown} tx */
function resolveTxId(rec, tx) {
  if (rec && typeof rec === 'object' && typeof rec.tx_id === 'string') return rec.tx_id;
  if (tx && typeof tx === 'object') {
    const o = tx;
    if (typeof o.tx_id === 'string') return o.tx_id;
    if (typeof o.id === 'string') return o.id;
  }
  return null;
}

/**
 * @param {import('node:sqlite').DatabaseSync} db
 * @param {string} chainId
 * @param {readonly { fromHeight: number; toHeight: number }[]} gapRanges
 * @param {number} nowSec
 */
export function replaceBlockHeightGaps(db, chainId, gapRanges, nowSec) {
  db.prepare('DELETE FROM block_height_gaps WHERE chain_id = ?').run(chainId);
  const ins = db.prepare(
    `INSERT INTO block_height_gaps (chain_id, from_height, to_height, reason, recorded_at)
     VALUES (?, ?, ?, 'pruned', ?)`
  );
  for (const g of gapRanges) {
    ins.run(chainId, g.fromHeight, g.toHeight, nowSec);
  }
}

/**
 * @param {import('node:sqlite').DatabaseSync} db
 * @param {string} chainId
 * @param {number} lastHeight
 * @param {string} lastHash
 * @param {number} nowSec
 */
export function upsertIngestCursor(db, chainId, lastHeight, lastHash, nowSec) {
  db.prepare(
    `INSERT INTO ingest_cursor (chain_id, last_committed_height, last_committed_block_hash, updated_at)
     VALUES (?, ?, ?, ?)
     ON CONFLICT(chain_id) DO UPDATE SET
       last_committed_height = excluded.last_committed_height,
       last_committed_block_hash = excluded.last_committed_block_hash,
       updated_at = excluded.updated_at`
  ).run(chainId, lastHeight, normHash(lastHash), nowSec);
}

/**
 * @param {readonly import('@boing-network/boing-sdk').BlockWithReceiptsBundle[]} bundles
 * @param {number} height
 */
export function blockHashAtHeight(bundles, height) {
  const b = bundles.find((x) => x.height === height);
  if (!b) return null;
  return normHash(b.block.hash);
}
