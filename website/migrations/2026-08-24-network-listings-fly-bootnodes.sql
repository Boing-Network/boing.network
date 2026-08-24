-- Point VibeMiner command templates at the hosted Fly bootnodes.
-- Apply (from website/): wrangler d1 execute boing-network-db --remote --file=./migrations/2026-08-24-network-listings-fly-bootnodes.sql

UPDATE network_listings
SET
  node_command_template = REPLACE(
    node_command_template,
    '/ip4/73.84.106.121/tcp/4001,/ip4/73.84.106.121/tcp/4001',
    '/ip4/169.155.48.188/tcp/4001,/ip4/109.105.220.118/tcp/4001'
  ),
  updated_at = datetime('now')
WHERE id IN ('boing-devnet', 'boing-devnet-linux', 'boing-devnet-macos');
