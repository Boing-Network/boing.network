-- Portal wallet sign-in: ensure portal_registrations exists with password columns.
-- Use this when the table does not exist yet (e.g. local dev). For existing DBs that
-- already have portal_registrations without password columns, run the ALTERs in
-- 2026-03-06-portal-password-alter.sql instead.
-- Apply with:
--   wrangler d1 execute boing-network-db --file=./migrations/2026-03-06-portal-password.sql

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
