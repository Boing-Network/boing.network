-- 2026-11-04 Testnet Passport stage 2 (credibility indexer). Idempotent.
-- Apply: npx wrangler d1 execute boing-network-db --remote --file=./migrations/2026-11-04-testnet-passport-stage2.sql
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
