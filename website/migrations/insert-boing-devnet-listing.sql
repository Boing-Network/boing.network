-- /api/networks D1 overrides (merged with static entries in functions/api/networks.js).
-- Canonical ids: boing-devnet (Windows), boing-devnet-linux, boing-devnet-macos.
--
-- Prefer regenerating from: `node scripts/network-listings-release-sql.mjs <tag>`, or copy the latest
-- dated migration (e.g. 2026-04-08-network-listings-boing-testnet-v0-1-8.sql) when it matches
-- `BOING_TESTNET_DOWNLOAD_TAG` in functions/api/networks.js.
--
-- Example below matches published testnet-v0.1.8 zips (refresh via network-listings-release-sql.mjs when retagging).
--
-- Apply to remote D1 (or use script with --apply and CLOUDFLARE_API_TOKEN — see script header):
--   npx wrangler d1 execute boing-network-db --remote --file=migrations/insert-boing-devnet-listing.sql

INSERT OR REPLACE INTO network_listings (id, node_download_url, node_command_template, node_binary_sha256, updated_at)
VALUES (
  'boing-devnet',
  'https://github.com/Boing-Network/boing.network/releases/download/testnet-v0.1.8/release-windows-x86_64.zip',
  'boing-node-windows-x86_64.exe --data-dir {dataDir} --p2p-listen /ip4/0.0.0.0/tcp/4001 --bootnodes /ip4/73.84.106.121/tcp/4001,/ip4/73.84.106.121/tcp/4001 --rpc-port 8545 --faucet-enable',
  '2cea7a6f093990c02bf405a20caf3b68bb59b434b69421449ab6bb4fec96a16a',
  datetime('now')
);

INSERT OR REPLACE INTO network_listings (id, node_download_url, node_command_template, node_binary_sha256, updated_at)
VALUES (
  'boing-devnet-linux',
  'https://github.com/Boing-Network/boing.network/releases/download/testnet-v0.1.8/release-linux-x86_64.zip',
  'boing-node-linux-x86_64 --data-dir {dataDir} --p2p-listen /ip4/0.0.0.0/tcp/4001 --bootnodes /ip4/73.84.106.121/tcp/4001,/ip4/73.84.106.121/tcp/4001 --rpc-port 8545 --faucet-enable',
  '70355e6e6c6c9f33804957df1c215a531bec0c329fe5c1fc48f3d23350bd296c',
  datetime('now')
);

INSERT OR REPLACE INTO network_listings (id, node_download_url, node_command_template, node_binary_sha256, updated_at)
VALUES (
  'boing-devnet-macos',
  'https://github.com/Boing-Network/boing.network/releases/download/testnet-v0.1.8/release-macos-aarch64.zip',
  'boing-node-macos-aarch64 --data-dir {dataDir} --p2p-listen /ip4/0.0.0.0/tcp/4001 --bootnodes /ip4/73.84.106.121/tcp/4001,/ip4/73.84.106.121/tcp/4001 --rpc-port 8545 --faucet-enable',
  '435216299129a6bcc04d4775cf7956315246c4860bf2fd8a769df93bea7e7bbc',
  datetime('now')
);
