-- Heights the public RPC gateway refused as over its response size cap (blob-sized test blocks).
-- The ingest cursor moves past them; backfill from an own node with POST /v1/backfill if ever needed.
CREATE TABLE IF NOT EXISTS skipped_blocks (
  chain_id TEXT NOT NULL,
  height INTEGER NOT NULL,
  reason TEXT NOT NULL,
  skipped_at INTEGER NOT NULL,
  PRIMARY KEY (chain_id, height)
);
