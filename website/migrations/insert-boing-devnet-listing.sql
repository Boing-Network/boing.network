-- VibeMiner /api/networks D1 overrides (merged with static entries in functions/api/networks.js).
-- Canonical ids: boing-devnet (Windows), boing-devnet-linux, boing-devnet-macos.
--
-- Regenerate hashes for a new GitHub release tag (from website/):
--   node scripts/network-listings-release-sql.mjs <tag>
--
-- Apply to remote D1:
--   npx wrangler d1 execute boing-network-db --remote --file=migrations/insert-boing-devnet-listing.sql

INSERT OR REPLACE INTO network_listings (id, node_download_url, node_command_template, node_binary_sha256, updated_at)
VALUES (
  'boing-devnet',
  'https://github.com/chiku524/boing.network/releases/download/testnet-v0.1.2/release-windows-x86_64.zip',
  'boing-node-windows-x86_64.exe --data-dir {dataDir} --p2p-listen /ip4/0.0.0.0/tcp/4001 --bootnodes /ip4/73.84.106.121/tcp/4001 --rpc-port 8545',
  '2e767306eb58947ab40e8c3d17034a7b0b42140bdfa6479513826d24a78ea669',
  datetime('now')
);

INSERT OR REPLACE INTO network_listings (id, node_download_url, node_command_template, node_binary_sha256, updated_at)
VALUES (
  'boing-devnet-linux',
  'https://github.com/chiku524/boing.network/releases/download/testnet-v0.1.2/release-linux-x86_64.zip',
  'boing-node-linux-x86_64 --data-dir {dataDir} --p2p-listen /ip4/0.0.0.0/tcp/4001 --bootnodes /ip4/73.84.106.121/tcp/4001 --rpc-port 8545',
  '8f80d3bae42980f1fa3c8378dc87591dd991e624f9570a9d2ee19ad6258cfe66',
  datetime('now')
);

INSERT OR REPLACE INTO network_listings (id, node_download_url, node_command_template, node_binary_sha256, updated_at)
VALUES (
  'boing-devnet-macos',
  'https://github.com/chiku524/boing.network/releases/download/testnet-v0.1.2/release-macos-aarch64.zip',
  'boing-node-macos-aarch64 --data-dir {dataDir} --p2p-listen /ip4/0.0.0.0/tcp/4001 --bootnodes /ip4/73.84.106.121/tcp/4001 --rpc-port 8545',
  '4926dacf1d1061323d96979e324b086a65febfe0c2a21a90fddd7c1ff8300501',
  datetime('now')
);
