/**
 * Extract reference-NFT ownership events from a block with receipts.
 */

import {
  SELECTOR_MINT_BATCH,
  SELECTOR_SET_METADATA_HASH,
  SELECTOR_TRANSFER_NFT,
  decodeReferenceNftCalldata,
} from 'boing-sdk';
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
 */
export function extractNftEventsFromBlock(
  block: unknown,
  blockHeight: number,
  lookupCurrentOwner: (collectionHex: string, tokenIdHex: string) => string | null
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
    if (typeof calldata !== 'string' && !(calldata instanceof Uint8Array)) continue;
    const decoded = decodeReferenceNftCalldata(calldata as string | Uint8Array);
    if (!decoded) continue;

    const txId = resolveTxId(tx, receipt, i, blockHeight);

    if (decoded.selector === SELECTOR_MINT_BATCH) {
      for (let j = 0; j < decoded.tokenIds.length; j++) {
        const tokenIdHex = normalizeHex64(decoded.tokenIds[j])!;
        const toOwnerHex = normalizeHex64(decoded.to)!;
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
      const tokenIdHex = normalizeHex64(decoded.tokenId)!;
      const toOwnerHex = normalizeHex64(decoded.to)!;
      const k = keyOf(collectionHex, tokenIdHex);
      const fromOwnerHex =
        overlay.get(k) ?? lookupCurrentOwner(collectionHex, tokenIdHex) ?? null;
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
      const tokenIdHex = normalizeHex64(decoded.tokenId)!;
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
