-- Testnet Passport stage 1 (2026-11-03). Rules: website/src/config/passport-rules-v1.json (v1.0.0).
-- Apply: npx wrangler d1 execute boing-network-db --remote --file=migrations/2026-11-03-testnet-passport-stage1.sql
-- Idempotent except the two ALTERs (re-running them fails harmlessly with "duplicate column").

CREATE TABLE IF NOT EXISTS portal_tracks (
  account_id_hex TEXT NOT NULL,
  track TEXT NOT NULL,
  joined_at TEXT NOT NULL,
  PRIMARY KEY (account_id_hex, track)
);
CREATE INDEX IF NOT EXISTS idx_portal_tracks_track ON portal_tracks(track);
INSERT OR IGNORE INTO portal_tracks (account_id_hex, track, joined_at)
  SELECT account_id_hex, role, COALESCE(created_at, datetime('now')) FROM portal_registrations WHERE role IS NOT NULL;

ALTER TABLE quest_completions ADD COLUMN verification_status TEXT;
ALTER TABLE quest_completions ADD COLUMN verified_by TEXT;
UPDATE quest_completions SET verification_status = CASE
  WHEN verified_at IS NOT NULL THEN 'verified' WHEN rejected_reason IS NOT NULL THEN 'rejected' ELSE 'pending' END
  WHERE verification_status IS NULL;

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
