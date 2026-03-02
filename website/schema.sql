-- D1 schema for boing.network (block explorer / network stats)
-- Apply with: wrangler d1 execute boing-network-db --file=./schema.sql

CREATE TABLE IF NOT EXISTS blocks (
  height INTEGER PRIMARY KEY,
  hash TEXT NOT NULL,
  parent_hash TEXT,
  proposer TEXT,
  tx_count INTEGER,
  created_at TEXT
);

CREATE TABLE IF NOT EXISTS accounts (
  id TEXT PRIMARY KEY,
  balance TEXT,
  nonce INTEGER,
  updated_at TEXT
);

CREATE TABLE IF NOT EXISTS network_stats (
  id TEXT PRIMARY KEY DEFAULT 'latest',
  block_height INTEGER,
  validator_count INTEGER,
  updated_at TEXT
);

-- Community quests (incentivized testnet). See docs/COMMUNITY-QUESTS.md.
CREATE TABLE IF NOT EXISTS quests (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  description TEXT,
  verification_type TEXT NOT NULL,
  reward_tier TEXT,
  active INTEGER DEFAULT 1,
  created_at TEXT
);

CREATE TABLE IF NOT EXISTS quest_completions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  quest_id TEXT NOT NULL,
  account_id_hex TEXT NOT NULL,
  proof_type TEXT,
  proof_value TEXT,
  submitted_at TEXT NOT NULL,
  verified_at TEXT,
  rejected_reason TEXT,
  FOREIGN KEY (quest_id) REFERENCES quests(id)
);

CREATE INDEX IF NOT EXISTS idx_quest_completions_account ON quest_completions(account_id_hex);
CREATE INDEX IF NOT EXISTS idx_quest_completions_quest ON quest_completions(quest_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_quest_completions_unique ON quest_completions(quest_id, account_id_hex);

-- Seed quests (ids must match website/src/config/quests.ts)
INSERT OR IGNORE INTO quests (id, name, description, verification_type, reward_tier, active, created_at) VALUES
  ('faucet', 'First drip', 'Request testnet BOING from the faucet.', 'on_chain', 'base', 1, datetime('now')),
  ('first_tx', 'First transaction', 'Send any transaction on testnet.', 'on_chain', 'base', 1, datetime('now')),
  ('validator_connect', 'Join the network', 'Run a node connected to testnet bootnodes.', 'manual', 'validator', 1, datetime('now')),
  ('feedback', 'Share feedback', 'Answer a few short questions about UX, docs, or bugs.', 'manual', 'bonus', 1, datetime('now')),
  ('social', 'Join community', 'Join Discord and post in #testnet-intros.', 'manual', 'bonus', 1, datetime('now'));

-- Testnet portal: registration and developer dApps. See docs/TESTNET-PORTAL.md.
CREATE TABLE IF NOT EXISTS portal_registrations (
  account_id_hex TEXT PRIMARY KEY,
  role TEXT NOT NULL,
  email TEXT,
  discord_handle TEXT,
  github_username TEXT,
  node_multiaddr TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS portal_dapps (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  contract_hex TEXT NOT NULL UNIQUE,
  owner_account_hex TEXT,
  name TEXT,
  registered_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_portal_dapps_owner ON portal_dapps(owner_account_hex);
