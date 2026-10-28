/**
 * Helpers for the additive protocol parameters published by `boing_getNetworkInfo` and
 * `boing_health` (`storage_tree_v2_activation_height`, `receipt_retention_blocks`).
 * Both keys are absent on older nodes, which these helpers report as "unknown" (`undefined`).
 */

import type { BoingChainParameters } from './types.js';

/** Any `boing_getNetworkInfo` / `boing_health` result (only the relevant keys are read). */
export type ChainParametersSource = BoingChainParameters & { head_height?: number };

/**
 * Whether contract-storage roots at `height` use 256-bit key routing (storage-tree v2).
 * `undefined`: the node does not publish the field (unknown); `false`: not configured or `height` is
 * below the activation height; `true`: `height >= storage_tree_v2_activation_height`.
 */
export function storageTreeV2ActiveAt(
  info: ChainParametersSource,
  height: number,
): boolean | undefined {
  const h = info.storage_tree_v2_activation_height;
  if (h === undefined) return undefined;
  if (h === null) return false;
  return height >= h;
}

/**
 * Lowest height whose receipts the node still serves, given its tip (`headHeight`, default
 * `info.head_height`). `0` when unbounded. `undefined` when the node does not publish
 * `receipt_retention_blocks` or the tip is unknown. The node prunes every 256 blocks, so treat the
 * value as approximate; transactions below it return `null` from `boing_getTransactionReceipt`.
 */
export function firstServedReceiptHeight(
  info: ChainParametersSource,
  headHeight: number | undefined = info.head_height,
): number | undefined {
  const w = info.receipt_retention_blocks;
  if (w === undefined) return undefined;
  if (w === null || w <= 0) return 0;
  if (headHeight === undefined) return undefined;
  return Math.max(0, headHeight - w + 1);
}