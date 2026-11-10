-- Reference multi-token (shared item types) touch index — request-driven, no cron.
-- Stores which (contract, item id, account) pairs ever moved; balances are always read live from
-- contract storage, so a reorged-out touch is harmless (it just shows a zero balance).

CREATE TABLE IF NOT EXISTS mt_cursor (
  chain_id TEXT PRIMARY KEY,
  last_scanned_height INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS mt_contracts (
  contract_hex TEXT PRIMARY KEY,
  is_reference INTEGER NOT NULL,
  checked_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS mt_holdings (
  contract_hex TEXT NOT NULL,
  item_id TEXT NOT NULL,
  account_hex TEXT NOT NULL,
  last_block_height INTEGER NOT NULL,
  last_tx_id TEXT NOT NULL,
  PRIMARY KEY (contract_hex, item_id, account_hex)
);

CREATE INDEX IF NOT EXISTS idx_mt_holdings_account ON mt_holdings (account_hex, contract_hex, item_id);
