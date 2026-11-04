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

CREATE INDEX IF NOT EXISTS idx_blocks_proposer ON blocks(proposer);

-- Community quests (incentivized testnet). See docs/TESTNET.md Part 2 §2.7 (Community Quests).
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
  verification_status TEXT, -- pending | verified | rejected (Testnet Passport)
  verified_by TEXT,         -- rpc | reviewer id
  FOREIGN KEY (quest_id) REFERENCES quests(id)
);

CREATE INDEX IF NOT EXISTS idx_quest_completions_account ON quest_completions(account_id_hex);
CREATE INDEX IF NOT EXISTS idx_quest_completions_quest ON quest_completions(quest_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_quest_completions_unique ON quest_completions(quest_id, account_id_hex);

-- Seed quests (generated from website/src/config/passport-rules-v1.json; keep in sync)
INSERT INTO quests (id, name, description, verification_type, reward_tier, active, created_at) VALUES
  ('faucet', 'First drip', 'Request testnet BOING from the faucet.', 'on_chain', 'base', 1, datetime('now')),
  ('first_tx', 'First transaction', 'Send any transaction on testnet.', 'on_chain', 'base', 1, datetime('now')),
  ('express_wallet', 'Open Express', 'Create or unlock a Boing Express wallet and copy your testnet address.', 'manual', 'ecosystem', 1, datetime('now')),
  ('finance_swap', 'Try Finance', 'Swap tokens or add liquidity on Boing Finance.', 'on_chain', 'ecosystem', 1, datetime('now')),
  ('observer_lookup', 'Look up on Observer', 'Find your account or a recent block on Boing Observer.', 'manual', 'ecosystem', 1, datetime('now')),
  ('first_deploy', 'Ship a contract', 'Deploy a contract that passes the on-chain QA gate.', 'on_chain', 'ecosystem', 1, datetime('now')),
  ('validator_connect', 'Join the validator set', 'Run a node and register it as an open validator.', 'on_chain', 'validator', 1, datetime('now')),
  ('feedback', 'Share feedback', 'Answer a few short questions about UX, docs, or bugs.', 'manual', 'bonus', 1, datetime('now')),
  ('social', 'Join community', 'Join Discord and post in #testnet-intros.', 'manual', 'bonus', 1, datetime('now'))
ON CONFLICT(id) DO UPDATE SET name = excluded.name, description = excluded.description,
  verification_type = excluded.verification_type, reward_tier = excluded.reward_tier, active = 1;
UPDATE quests SET active = 0 WHERE id NOT IN ('faucet','first_tx','express_wallet','finance_swap','observer_lookup','first_deploy','validator_connect','feedback','social');

-- Testnet portal: registration and developer dApps. See docs/TESTNET.md Part 2.
CREATE TABLE IF NOT EXISTS portal_registrations (
  account_id_hex TEXT PRIMARY KEY,
  role TEXT NOT NULL,
  email TEXT,
  discord_handle TEXT,
  github_username TEXT,
  node_multiaddr TEXT,
  password_salt TEXT,
  password_hash TEXT,
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

CREATE TABLE IF NOT EXISTS portal_auth_nonces (
  nonce TEXT PRIMARY KEY,
  origin TEXT NOT NULL,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  used_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_portal_auth_nonces_expires ON portal_auth_nonces(expires_at);

-- Portal API rate-limit buckets (issue #61 item 68).
CREATE TABLE IF NOT EXISTS portal_rate_limits (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  bucket_key TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_portal_rate_limits_bucket_created
  ON portal_rate_limits(bucket_key, created_at);

-- Node listing: D1 overrides for public network listings (merged by GET /api/networks).
-- ids: boing-devnet (Windows), boing-devnet-linux, boing-devnet-macos (see migrations/insert-boing-devnet-listing.sql)
CREATE TABLE IF NOT EXISTS network_listings (
  id TEXT PRIMARY KEY,
  node_download_url TEXT,
  node_command_template TEXT,
  node_binary_sha256 TEXT,
  updated_at TEXT
);

-- Testnet Passport: an account can join several tracks (user | developer | node_operator).
CREATE TABLE IF NOT EXISTS portal_tracks (
  account_id_hex TEXT NOT NULL,
  track TEXT NOT NULL,
  joined_at TEXT NOT NULL,
  PRIMARY KEY (account_id_hex, track)
);
CREATE INDEX IF NOT EXISTS idx_portal_tracks_track ON portal_tracks(track);

-- Testnet Passport stage 2: credibility indexer tables (written by workers/credibility-indexer).
CREATE TABLE IF NOT EXISTS passport_signals (
  account_id_hex TEXT PRIMARY KEY,
  tx_sent INTEGER NOT NULL DEFAULT 0,
  deploys INTEGER NOT NULL DEFAULT 0,
  dex_calls INTEGER NOT NULL DEFAULT 0,
  faucet_drips INTEGER NOT NULL DEFAULT 0,
  active_days INTEGER NOT NULL DEFAULT 0,
  validator_joined INTEGER NOT NULL DEFAULT 0,
  uptime_avg_pct REAL,
  uptime_samples INTEGER NOT NULL DEFAULT 0,
  first_activity_at TEXT,
  last_activity_at TEXT,
  history_complete INTEGER NOT NULL DEFAULT 0,
  rules_version TEXT,
  updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_passport_signals_updated ON passport_signals(updated_at);

CREATE TABLE IF NOT EXISTS passport_badges (
  account_id_hex TEXT NOT NULL,
  badge_id TEXT NOT NULL,
  track TEXT NOT NULL,
  earned_at TEXT NOT NULL,
  historic INTEGER NOT NULL DEFAULT 0, -- 1 = earned before Passport registration (credited from genesis)
  evidence TEXT,
  PRIMARY KEY (account_id_hex, badge_id)
);

CREATE TABLE IF NOT EXISTS passport_tiers (
  account_id_hex TEXT NOT NULL,
  track TEXT NOT NULL,
  tier TEXT NOT NULL, -- none | bronze | silver | gold
  tier_rank INTEGER NOT NULL DEFAULT 0,
  score INTEGER NOT NULL DEFAULT 0,
  badge_count INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (account_id_hex, track)
);
CREATE INDEX IF NOT EXISTS idx_passport_tiers_board ON passport_tiers(track, tier_rank DESC, score DESC);

CREATE TABLE IF NOT EXISTS validator_uptime_samples (
  sampled_at TEXT NOT NULL,
  validator_id TEXT NOT NULL,
  owner TEXT,
  in_set INTEGER NOT NULL DEFAULT 0,
  status TEXT,
  uptime_pct REAL,
  proposals_scheduled INTEGER,
  proposals_missed INTEGER,
  PRIMARY KEY (sampled_at, validator_id)
);
CREATE INDEX IF NOT EXISTS idx_validator_uptime_validator ON validator_uptime_samples(validator_id, sampled_at);

CREATE TABLE IF NOT EXISTS passport_snapshots (
  week TEXT PRIMARY KEY, -- ISO week, e.g. 2026-W45
  merkle_root TEXT,
  rules_version TEXT NOT NULL,
  entries INTEGER NOT NULL,
  json TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS indexer_state (
  key TEXT PRIMARY KEY,
  value TEXT,
  updated_at TEXT
);

-- Testnet Passport stage 4: Discord OAuth linking + tier-role sync.
CREATE TABLE IF NOT EXISTS discord_oauth_states (
  state TEXT PRIMARY KEY,
  account_id_hex TEXT NOT NULL,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  used_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_discord_oauth_states_expires ON discord_oauth_states(expires_at);

CREATE TABLE IF NOT EXISTS discord_links (
  account_id_hex TEXT PRIMARY KEY,
  discord_id TEXT NOT NULL UNIQUE,
  discord_username TEXT,
  linked_at TEXT NOT NULL,
  in_guild INTEGER,
  roles_synced TEXT NOT NULL DEFAULT '[]',
  last_synced_at TEXT,
  sync_error TEXT
);

CREATE TABLE IF NOT EXISTS discord_role_removals (
  discord_id TEXT PRIMARY KEY,
  role_ids TEXT NOT NULL,
  queued_at TEXT NOT NULL
);
