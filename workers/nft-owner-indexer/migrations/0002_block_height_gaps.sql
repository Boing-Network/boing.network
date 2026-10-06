-- Track pruned / missing block heights so the indexer never silently claims
-- completeness across a gap (public RPC nodes prune history beyond a window).
-- Mirrors tools/observer-indexer-schema.sql's block_height_gaps table.
-- SDK gap helpers: boing-sdk `summarizeIndexerFetchGaps`, `mergeInclusiveHeightRanges`,
-- `subtractInclusiveRangeFromRanges`, `nextContiguousIndexedHeightAfterOmittedFetch`.

CREATE TABLE IF NOT EXISTS block_height_gaps (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  chain_id TEXT NOT NULL,
  from_height INTEGER NOT NULL,
  to_height INTEGER NOT NULL,
  reason TEXT NOT NULL DEFAULT 'pruned',
  recorded_at INTEGER NOT NULL,
  CHECK (from_height >= 0),
  CHECK (from_height <= to_height)
);

CREATE INDEX IF NOT EXISTS idx_block_height_gaps_chain_from ON block_height_gaps (chain_id, from_height);
