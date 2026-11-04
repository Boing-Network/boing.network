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
