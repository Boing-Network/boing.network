-- Durable reference-NFT ownership index (mint_batch / transfer_nft / set_metadata_hash).
-- Reorg-safe: rewind deletes indexed_blocks + nft_events at height, then rebuilds nft_ownership.

CREATE TABLE IF NOT EXISTS ingest_cursor (
  chain_id TEXT PRIMARY KEY,
  last_committed_height INTEGER NOT NULL,
  last_committed_block_hash TEXT NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS indexed_blocks (
  height INTEGER PRIMARY KEY,
  block_hash TEXT NOT NULL,
  parent_hash TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_indexed_blocks_hash ON indexed_blocks (block_hash);

CREATE TABLE IF NOT EXISTS nft_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  block_height INTEGER NOT NULL,
  tx_index INTEGER NOT NULL,
  event_index INTEGER NOT NULL,
  tx_id TEXT NOT NULL,
  collection_hex TEXT NOT NULL,
  token_id_hex TEXT NOT NULL,
  event_kind TEXT NOT NULL,
  from_owner_hex TEXT,
  to_owner_hex TEXT,
  metadata_hash_hex TEXT,
  UNIQUE (block_height, tx_index, event_index, collection_hex, token_id_hex)
);

CREATE INDEX IF NOT EXISTS idx_nft_events_height ON nft_events (block_height);
CREATE INDEX IF NOT EXISTS idx_nft_events_token ON nft_events (
  collection_hex,
  token_id_hex,
  block_height DESC,
  tx_index DESC,
  event_index DESC
);

CREATE TABLE IF NOT EXISTS nft_ownership (
  collection_hex TEXT NOT NULL,
  token_id_hex TEXT NOT NULL,
  owner_hex TEXT NOT NULL,
  metadata_hash_hex TEXT,
  last_block_height INTEGER NOT NULL,
  last_tx_id TEXT NOT NULL,
  last_event_kind TEXT NOT NULL,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (collection_hex, token_id_hex)
);

CREATE INDEX IF NOT EXISTS idx_nft_ownership_owner ON nft_ownership (
  owner_hex,
  collection_hex,
  token_id_hex
);
