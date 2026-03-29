-- VibeMiner /api/networks D1 overrides (merged with static entries in functions/api/networks.js).
-- Canonical ids: boing-devnet (Windows), boing-devnet-linux, boing-devnet-macos.
--
-- SHA256 values below match published GitHub assets for tag testnet-v0.1.3 (verified via
-- `node scripts/network-listings-release-sql.mjs testnet-v0.1.3`). Re-run that script after each
-- new release to regenerate SQL, then apply.
--
-- Apply to remote D1 (or use script with --apply and CLOUDFLARE_API_TOKEN — see script header):
--   npx wrangler d1 execute boing-network-db --remote --file=migrations/insert-boing-devnet-listing.sql

INSERT OR REPLACE INTO network_listings (id, node_download_url, node_command_template, node_binary_sha256, updated_at)
VALUES (
  'boing-devnet',
  'https://github.com/chiku524/boing.network/releases/download/testnet-v0.1.3/release-windows-x86_64.zip',
  'boing-node-windows-x86_64.exe --data-dir {dataDir} --p2p-listen /ip4/0.0.0.0/tcp/4001 --bootnodes /ip4/73.84.106.121/tcp/4001 --rpc-port 8545 --faucet-enable',
  '37b2dc37164227f944b35712d709dc1d74dacd6f8c352def4b9dcefc239634ea',
  datetime('now')
);

INSERT OR REPLACE INTO network_listings (id, node_download_url, node_command_template, node_binary_sha256, updated_at)
VALUES (
  'boing-devnet-linux',
  'https://github.com/chiku524/boing.network/releases/download/testnet-v0.1.3/release-linux-x86_64.zip',
  'boing-node-linux-x86_64 --data-dir {dataDir} --p2p-listen /ip4/0.0.0.0/tcp/4001 --bootnodes /ip4/73.84.106.121/tcp/4001 --rpc-port 8545 --faucet-enable',
  'a2dc84bc1c92769408a29f64c782ca86b991ef8d8d99ee14b5c07ddb5d6ed546',
  datetime('now')
);

INSERT OR REPLACE INTO network_listings (id, node_download_url, node_command_template, node_binary_sha256, updated_at)
VALUES (
  'boing-devnet-macos',
  'https://github.com/chiku524/boing.network/releases/download/testnet-v0.1.3/release-macos-aarch64.zip',
  'boing-node-macos-aarch64 --data-dir {dataDir} --p2p-listen /ip4/0.0.0.0/tcp/4001 --bootnodes /ip4/73.84.106.121/tcp/4001 --rpc-port 8545 --faucet-enable',
  'c7a7ed578c58b47e7b21cfc301c4e128b78da26eaf810ee13e0bfd223594be49',
  datetime('now')
);
