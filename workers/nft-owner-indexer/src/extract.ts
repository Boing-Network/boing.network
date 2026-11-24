/**
 * Extract reference-NFT ownership events from a block with receipts.
 */

import {
  SELECTOR_MINT_BATCH,
  SELECTOR_SET_METADATA_HASH,
  SELECTOR_TRANSFER_NFT,
  decodeReferenceNftCalldata,
  decodeReferenceNftV4Log,
} from '@boing-network/boing-sdk';
import { isZeroHex64, normalizeHex64, zeros32 } from './cors.js';

export type NftEventKind = 'mint_batch' | 'transfer_nft' | 'set_metadata_hash';

export type ExtractedNftEvent = {
  blockHeight: number;
  txIndex: number;
  eventIndex: number;
  txId: string;
  collectionHex: string;
  tokenIdHex: string;
  eventKind: NftEventKind;
  fromOwnerHex: string | null;
  toOwnerHex: string | null;
  metadataHashHex: string | null;
};

function unwrapTaggedPayload(payload: unknown): Record<string, unknown> {
  if (!payload || typeof payload !== 'object' || Array.isArray(payload)) return {};
  const p = payload as Record<string, unknown>;
  const keys = Object.keys(p);
  if (keys.length === 1) {
    const key = keys[0]!;
    const val = p[key];
    if (val && typeof val === 'object' && !Array.isArray(val)) {
      const lower = key.toLowerCase();
      if (
        lower === 'contractcall' ||
        lower === 'transfer' ||
        lower === 'bond' ||
        /^[a-z][a-z0-9_]*$/i.test(key)
      ) {
        return val as Record<string, unknown>;
      }
    }
  }
  return p;
}

function isContractCallPayload(payload: unknown): boolean {
  if (!payload || typeof payload !== 'object' || Array.isArray(payload)) return false;
  const p = payload as Record<string, unknown>;
  const keys = Object.keys(p);
  if (keys.length === 1 && keys[0]!.toLowerCase() === 'contractcall') return true;
  const inner = unwrapTaggedPayload(payload);
  return typeof inner.contract === 'string' || typeof inner.Contract === 'string';
}

/** `tx.sender` as 0x + 64 hex (hex string or 32-number byte array). */
export function txSenderHex(tx: unknown): string | null {
  if (!tx || typeof tx !== 'object') return null;
  const o = tx as { sender?: unknown; from?: unknown };
  return normalizeHex64(o.sender ?? o.from);
}

/**
 * Collection admin lookup for the auth replay below. Return the admin AccountId (0x + 64 hex), or
 * `null` when the admin slot is empty (no call to a reference collection can then have succeeded).
 * Pass `undefined` to `extractNftEventsFromBlock` to skip auth replay (unit tests, legacy callers).
 */
export type AdminLookup = (collectionHex: string) => string | null;

/**
 * True when the collection's installed code is the pinned template v4 runtime. v4 faults on every failed
 * check and logs every change, so its receipts are read from logs instead of replaying auth rules.
 */
export type V4Lookup = (collectionHex: string) => boolean;

function hexOf(raw: unknown): string | null {
  if (typeof raw === 'string') return raw.trim();
  if (Array.isArray(raw) && raw.every((b) => Number.isInteger(b) && b >= 0 && b <= 255)) {
    return '0x' + (raw as number[]).map((b) => b.toString(16).padStart(2, '0')).join('');
  }
  return null;
}

/**
 * Events from a successful direct call to a v4 collection, read from its receipt logs. Logs carry no
 * emitter address, but v4 makes no nested calls, so every log of a direct call to it came from it.
 * Mint logs carry ids only; their metadata hashes come from the (now known successful) calldata.
 */
function v4EventsFromReceipt(
  receipt: unknown,
  calldata: unknown,
  base: Pick<ExtractedNftEvent, 'blockHeight' | 'txIndex' | 'txId' | 'collectionHex'>
): ExtractedNftEvent[] {
  const logs = (receipt as { logs?: unknown }).logs;
  if (!Array.isArray(logs)) return [];
  const mintHashes = new Map<string, string | null>();
  const cd = hexOf(calldata);
  const decoded = cd ? decodeReferenceNftCalldata(cd) : null;
  if (decoded?.selector === SELECTOR_MINT_BATCH) {
    decoded.tokenIds.forEach((t, j) => {
      const id = normalizeHex64(t);
      const h = normalizeHex64(decoded.metadataHashes[j]);
      if (id) mintHashes.set(id, h && !isZeroHex64(h) ? h : null);
    });
  }
  const out: ExtractedNftEvent[] = [];
  const push = (e: Omit<ExtractedNftEvent, keyof typeof base | 'eventIndex'>) =>
    out.push({ ...base, eventIndex: out.length, ...e });
  for (const raw of logs) {
    if (!raw || typeof raw !== 'object') continue;
    const l = raw as { topics?: unknown; data?: unknown };
    const topics = Array.isArray(l.topics) ? l.topics.map((t) => normalizeHex64(t)) : [];
    const data = hexOf(l.data) ?? '0x';
    if (topics.some((t) => t == null)) continue;
    const d = decodeReferenceNftV4Log({ topics: topics as string[], data });
    if (d?.kind === 'mint_batch') {
      for (const tokenIdHex of d.tokenIds) {
        push({
          tokenIdHex,
          eventKind: 'mint_batch',
          fromOwnerHex: zeros32(),
          toOwnerHex: d.to,
          metadataHashHex: mintHashes.get(tokenIdHex) ?? null,
        });
      }
    } else if (d?.kind === 'transfer') {
      push({
        tokenIdHex: d.tokenId,
        eventKind: 'transfer_nft',
        fromOwnerHex: d.from,
        toOwnerHex: d.to,
        metadataHashHex: null,
      });
    } else if (d?.kind === 'metadata') {
      push({
        tokenIdHex: d.tokenId,
        eventKind: 'set_metadata_hash',
        fromOwnerHex: null,
        toOwnerHex: null,
        metadataHashHex: d.metadataHash,
      });
    }
  }
  return out;
}

function resolveTxId(tx: unknown, receipt: unknown, txIndex: number, blockHeight: number): string {
  if (receipt && typeof receipt === 'object') {
    const tid = (receipt as { tx_id?: unknown }).tx_id;
    const n = normalizeHex64(tid);
    if (n) return n;
  }
  if (tx && typeof tx === 'object') {
    const o = tx as { tx_id?: unknown; id?: unknown };
    const n = normalizeHex64(o.tx_id) ?? normalizeHex64(o.id);
    if (n) return n;
  }
  // Synthetic fallback so UNIQUE constraints still work if RPC omits ids.
  return normalizeHex64(
    `0x${blockHeight.toString(16).padStart(16, '0')}${txIndex.toString(16).padStart(16, '0')}${'00'.repeat(16)}`
  )!;
}

/**
 * Pure helper: scan one `boing_getBlockByHeight(..., true)` block for successful
 * reference NFT mint_batch / transfer_nft / set_metadata_hash calls.
 *
 * `lookupCurrentOwner(collection, tokenId)` supplies prior owner for transfers
 * (calldata has no `from`).
 *
 * `adminOf` turns on auth replay. The reference template (v1–v3) ends unauthorized `transfer_nft`
 * and `set_metadata_hash` calls with `STOP`, so their receipts say `success: true` although nothing
 * changed. Without replay, anyone could send `transfer_nft(me, someone_else's_token)` and be indexed
 * as the owner. Replay applies the template's own rules: an owned token moves only when the sender
 * is its owner; an unowned token (lazy mint) and `mint_batch` need the admin; metadata needs the
 * admin or the owner. A contract with no admin in the template's admin slot is not indexed.
 *
 * `isV4` marks collections running template v4: their events come from receipt logs (only when the
 * receipt says `success: true`), with no replay, because v4 also lets approved spenders and operators
 * transfer and reverts every failed call.
 */
export function extractNftEventsFromBlock(
  block: unknown,
  blockHeight: number,
  lookupCurrentOwner: (collectionHex: string, tokenIdHex: string) => string | null,
  adminOf?: AdminLookup,
  isV4?: V4Lookup
): ExtractedNftEvent[] {
  if (!block || typeof block !== 'object') return [];
  const b = block as { transactions?: unknown; receipts?: unknown };
  const txs = Array.isArray(b.transactions) ? b.transactions : [];
  const receipts = Array.isArray(b.receipts) ? b.receipts : [];
  const out: ExtractedNftEvent[] = [];

  // In-block ownership overlay so later txs in the same block see earlier updates.
  const overlay = new Map<string, string>();
  const keyOf = (c: string, t: string) => `${c}:${t}`;

  for (let i = 0; i < txs.length; i++) {
    const tx = txs[i];
    if (!tx || typeof tx !== 'object') continue;
    const receipt = receipts[i] ?? null;
    if (receipt && typeof receipt === 'object' && (receipt as { success?: unknown }).success === false) {
      continue;
    }
    const payload = (tx as { payload?: unknown }).payload;
    if (!isContractCallPayload(payload)) continue;
    const inner = unwrapTaggedPayload(payload);
    const collectionHex = normalizeHex64(inner.contract ?? inner.Contract);
    if (!collectionHex) continue;
    const calldata = inner.calldata ?? inner.Calldata;
    if (isV4?.(collectionHex)) {
      if (!receipt || (receipt as { success?: unknown }).success !== true) continue;
      const events = v4EventsFromReceipt(receipt, calldata, {
        blockHeight,
        txIndex: i,
        txId: resolveTxId(tx, receipt, i, blockHeight),
        collectionHex,
      });
      for (const e of events) {
        if (e.toOwnerHex) overlay.set(keyOf(collectionHex, e.tokenIdHex), e.toOwnerHex);
        out.push(e);
      }
      continue;
    }
    if (typeof calldata !== 'string' && !(calldata instanceof Uint8Array)) continue;
    const decoded = decodeReferenceNftCalldata(calldata as string | Uint8Array);
    if (!decoded) continue;

    const txId = resolveTxId(tx, receipt, i, blockHeight);
    const sender = adminOf ? txSenderHex(tx) : null;
    const admin = adminOf ? adminOf(collectionHex) : null;
    if (adminOf && (!sender || !admin)) continue;
    const isAdmin = adminOf ? sender === admin : true;

    if (decoded.selector === SELECTOR_MINT_BATCH) {
      if (!isAdmin) continue;
      for (let j = 0; j < decoded.tokenIds.length; j++) {
        // Skip malformed hex rather than asserting into D1 (audit #61 item 74).
        const tokenIdHex = normalizeHex64(decoded.tokenIds[j]);
        const toOwnerHex = normalizeHex64(decoded.to);
        if (!tokenIdHex || !toOwnerHex) continue;
        const meta = normalizeHex64(decoded.metadataHashes[j]);
        const metadataHashHex = meta && !isZeroHex64(meta) ? meta : null;
        out.push({
          blockHeight,
          txIndex: i,
          eventIndex: j,
          txId,
          collectionHex,
          tokenIdHex,
          eventKind: 'mint_batch',
          fromOwnerHex: zeros32(),
          toOwnerHex,
          metadataHashHex,
        });
        overlay.set(keyOf(collectionHex, tokenIdHex), toOwnerHex);
      }
      continue;
    }

    if (decoded.selector === SELECTOR_TRANSFER_NFT) {
      const tokenIdHex = normalizeHex64(decoded.tokenId);
      const toOwnerHex = normalizeHex64(decoded.to);
      if (!tokenIdHex || !toOwnerHex) continue;
      const k = keyOf(collectionHex, tokenIdHex);
      const fromOwnerHex =
        overlay.get(k) ?? lookupCurrentOwner(collectionHex, tokenIdHex) ?? null;
      if (adminOf) {
        const owned = fromOwnerHex != null && !isZeroHex64(fromOwnerHex);
        if (owned ? sender !== fromOwnerHex : !isAdmin) continue;
      }
      out.push({
        blockHeight,
        txIndex: i,
        eventIndex: 0,
        txId,
        collectionHex,
        tokenIdHex,
        eventKind: 'transfer_nft',
        fromOwnerHex,
        toOwnerHex,
        metadataHashHex: null,
      });
      overlay.set(k, toOwnerHex);
      continue;
    }

    if (decoded.selector === SELECTOR_SET_METADATA_HASH) {
      const tokenIdHex = normalizeHex64(decoded.tokenId);
      if (!tokenIdHex) continue;
      if (adminOf && !isAdmin) {
        const k = keyOf(collectionHex, tokenIdHex);
        const owner = overlay.get(k) ?? lookupCurrentOwner(collectionHex, tokenIdHex) ?? null;
        if (!owner || sender !== owner) continue;
      }
      const metadataHashHex = normalizeHex64(decoded.metadataHash);
      out.push({
        blockHeight,
        txIndex: i,
        eventIndex: 0,
        txId,
        collectionHex,
        tokenIdHex,
        eventKind: 'set_metadata_hash',
        fromOwnerHex: null,
        toOwnerHex: null,
        metadataHashHex,
      });
    }
  }

  return out;
}
