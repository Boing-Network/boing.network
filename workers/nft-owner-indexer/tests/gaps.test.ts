import { describe, expect, it } from 'vitest';
import {
  nextContiguousIndexedHeightAfterOmittedFetch,
  summarizeIndexerFetchGaps,
} from 'boing-sdk';
import { reconcileGapRangesForWindow } from '../src/gaps.js';

describe('reconcileGapRangesForWindow', () => {
  it('records a newly-discovered pruned range inside the window', () => {
    const out = reconcileGapRangesForWindow(
      [],
      { fromHeight: 100, toHeight: 110 },
      [{ fromHeight: 103, toHeight: 105 }]
    );
    expect(out).toEqual([{ fromHeight: 103, toHeight: 105 }]);
  });

  it('leaves stored gaps outside the window untouched', () => {
    const stored = [{ fromHeight: 1, toHeight: 5 }];
    const out = reconcileGapRangesForWindow(stored, { fromHeight: 100, toHeight: 110 }, []);
    expect(out).toEqual([{ fromHeight: 1, toHeight: 5 }]);
  });

  it('drops a previously-recorded gap once a backfill resolves it (no longer missing)', () => {
    const stored = [{ fromHeight: 100, toHeight: 110 }];
    // Operator backfilled [100,110] and every height came back present this time.
    const out = reconcileGapRangesForWindow(stored, { fromHeight: 100, toHeight: 110 }, []);
    expect(out).toEqual([]);
  });

  it('narrows a stored gap when backfill only resolves part of it', () => {
    const stored = [{ fromHeight: 100, toHeight: 110 }];
    // Backfill of [100,105] found 100-102 present; 103-105 still pruned.
    const out = reconcileGapRangesForWindow(
      stored,
      { fromHeight: 100, toHeight: 105 },
      [{ fromHeight: 103, toHeight: 105 }]
    );
    expect(out).toEqual([
      { fromHeight: 103, toHeight: 110 },
    ]);
  });

  it('merges adjacent/overlapping ranges after reconciliation', () => {
    const stored = [
      { fromHeight: 1, toHeight: 5 },
      { fromHeight: 20, toHeight: 25 },
    ];
    const out = reconcileGapRangesForWindow(
      stored,
      { fromHeight: 6, toHeight: 19 },
      [{ fromHeight: 6, toHeight: 10 }]
    );
    expect(out).toEqual([{ fromHeight: 1, toHeight: 10 }, { fromHeight: 20, toHeight: 25 }]);
  });
});

describe('cursor advancement stays behind a pruned gap (sdk gap helpers)', () => {
  it('does not advance past a hole at the start of the window', () => {
    const summary = summarizeIndexerFetchGaps(100, 110, [102, 103, 104]);
    expect(nextContiguousIndexedHeightAfterOmittedFetch(99, summary)).toBe(99);
  });

  it('advances to just before a mid-window hole', () => {
    const summary = summarizeIndexerFetchGaps(100, 110, [100, 101, 102, 105, 106]);
    expect(nextContiguousIndexedHeightAfterOmittedFetch(99, summary)).toBe(102);
  });

  it('advances through the full window when nothing is missing', () => {
    const fetched = [100, 101, 102, 103];
    const summary = summarizeIndexerFetchGaps(100, 103, fetched);
    expect(nextContiguousIndexedHeightAfterOmittedFetch(99, summary)).toBe(103);
  });
});
