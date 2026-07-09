-- Boing testnet node zips: testnet-v0.1.10 (50k faucet; SHA256 from GitHub release asset digests).
-- Apply (from website/): wrangler d1 execute boing-network-db --remote --file=./migrations/2026-07-09-network-listings-boing-testnet-v0-1-10.sql

INSERT OR REPLACE INTO network_listings (id, node_download_url, node_command_template, node_binary_sha256, updated_at)
VALUES (
  'boing-devnet',
  'https://github.com/Boing-Network/boing.network/releases/download/testnet-v0.1.10/release-windows-x86_64.zip',
  'boing-node-windows-x86_64.exe --data-dir {dataDir} --p2p-listen /ip4/0.0.0.0/tcp/4001 --bootnodes /ip4/73.84.106.121/tcp/4001,/ip4/73.84.106.121/tcp/4001 --rpc-port 8545 --faucet-enable',
  '76c89f0e25069bb4462244778291c673590b818547f637af9205b1efa2ffce8e',
  datetime('now')
);

INSERT OR REPLACE INTO network_listings (id, node_download_url, node_command_template, node_binary_sha256, updated_at)
VALUES (
  'boing-devnet-linux',
  'https://github.com/Boing-Network/boing.network/releases/download/testnet-v0.1.10/release-linux-x86_64.zip',
  'boing-node-linux-x86_64 --data-dir {dataDir} --p2p-listen /ip4/0.0.0.0/tcp/4001 --bootnodes /ip4/73.84.106.121/tcp/4001,/ip4/73.84.106.121/tcp/4001 --rpc-port 8545 --faucet-enable',
  'b576df6288d9ead28dd9ba380850e97738b7f9cf058ab95ed925293162409561',
  datetime('now')
);

INSERT OR REPLACE INTO network_listings (id, node_download_url, node_command_template, node_binary_sha256, updated_at)
VALUES (
  'boing-devnet-macos',
  'https://github.com/Boing-Network/boing.network/releases/download/testnet-v0.1.10/release-macos-aarch64.zip',
  'boing-node-macos-aarch64 --data-dir {dataDir} --p2p-listen /ip4/0.0.0.0/tcp/4001 --bootnodes /ip4/73.84.106.121/tcp/4001,/ip4/73.84.106.121/tcp/4001 --rpc-port 8545 --faucet-enable',
  '5ddd479223be9a195dd6b33f60cd1001836af754720bde5766f44e0e8d7b984f',
  datetime('now')
);
