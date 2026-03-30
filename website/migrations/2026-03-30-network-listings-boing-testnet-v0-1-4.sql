-- Refresh Boing testnet node zips to GitHub release testnet-v0.1.4 (SHA256 from network-listings-release-sql.mjs).
-- Apply (from website/): wrangler d1 execute boing-network-db --remote --file=./migrations/2026-03-30-network-listings-boing-testnet-v0-1-4.sql
-- (Use your D1 database name / --local as appropriate.)
--
-- Runtime note: `functions/api/networks.js` also rewrites stale `testnet-v0.1.0`–`v0.1.2` download URLs to
-- v0.1.4 after you deploy Pages/Workers — but applying this migration keeps D1 authoritative and SHA256s correct.

INSERT OR REPLACE INTO network_listings (id, node_download_url, node_command_template, node_binary_sha256, updated_at)
VALUES (
  'boing-devnet',
  'https://github.com/chiku524/boing.network/releases/download/testnet-v0.1.4/release-windows-x86_64.zip',
  'boing-node-windows-x86_64.exe --data-dir {dataDir} --p2p-listen /ip4/0.0.0.0/tcp/4001 --bootnodes /ip4/73.84.106.121/tcp/4001,/ip4/73.84.106.121/tcp/4001 --rpc-port 8545 --faucet-enable',
  '50898a02f3cba1effe0c91a6f0ea48d3eed62ab87b7aeb3ebb653b30a1248f65',
  datetime('now')
);

INSERT OR REPLACE INTO network_listings (id, node_download_url, node_command_template, node_binary_sha256, updated_at)
VALUES (
  'boing-devnet-linux',
  'https://github.com/chiku524/boing.network/releases/download/testnet-v0.1.4/release-linux-x86_64.zip',
  'boing-node-linux-x86_64 --data-dir {dataDir} --p2p-listen /ip4/0.0.0.0/tcp/4001 --bootnodes /ip4/73.84.106.121/tcp/4001,/ip4/73.84.106.121/tcp/4001 --rpc-port 8545 --faucet-enable',
  'a96987461201f00d618afad5a494b52837663f90f6d9d3d5c097b6843cad17ab',
  datetime('now')
);

INSERT OR REPLACE INTO network_listings (id, node_download_url, node_command_template, node_binary_sha256, updated_at)
VALUES (
  'boing-devnet-macos',
  'https://github.com/chiku524/boing.network/releases/download/testnet-v0.1.4/release-macos-aarch64.zip',
  'boing-node-macos-aarch64 --data-dir {dataDir} --p2p-listen /ip4/0.0.0.0/tcp/4001 --bootnodes /ip4/73.84.106.121/tcp/4001,/ip4/73.84.106.121/tcp/4001 --rpc-port 8545 --faucet-enable',
  '26fd3477dfead760b3a04d5449173cbb7468286f33a51eec09d07d96982c0718',
  datetime('now')
);
