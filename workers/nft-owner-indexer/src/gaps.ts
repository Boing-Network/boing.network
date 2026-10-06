/**
 * Pure helpers for reconciling pruned-height gap ranges against one ingest window.
 * Pairs with D1 persistence in `persist.ts` (`loadGapRanges` / `replaceGapRanges`).
 */

import {
  mergeInclusiveHeightRanges,
  subtractInclusiveRangeFromRanges,
  type InclusiveHeightRange,
} from 'boing-sdk';

/**
 * Replace any stored gap records that fall inside `window` with `stillMissingWithinWindow`,
 * leaving gap records outside the window untouched. Use this after (re)indexing `window`
 * (cron tick or operator `POST /v1/backfill`) so a resolved gap (e.g. a prior prune that a
 * backfill call fixed) is dropped, while a gap newly discovered in this window is recorded.
 */
export function reconcileGapRangesForWindow(
  stored: readonly InclusiveHeightRange[],
  window: InclusiveHeightRange,
  stillMissingWithinWindow: readonly InclusiveHeightRange[]
): InclusiveHeightRange[] {
  const storedOutsideWindow = subtractInclusiveRangeFromRanges(window, stored);
  return mergeInclusiveHeightRanges([...storedOutsideWindow, ...stillMissingWithinWindow]);
}
