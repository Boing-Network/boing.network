-- Portal wallet auth: nonce table for replay-resistant sign-in.
-- Apply with:
--   wrangler d1 execute boing-network-db --file=./migrations/2026-03-06-portal-auth-nonces.sql

CREATE TABLE IF NOT EXISTS portal_auth_nonces (
  nonce TEXT PRIMARY KEY,
  origin TEXT NOT NULL,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  used_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_portal_auth_nonces_expires ON portal_auth_nonces(expires_at);
