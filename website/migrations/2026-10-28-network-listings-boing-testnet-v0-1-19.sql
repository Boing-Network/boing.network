-- network_listings refresh for Boing-Network/boing.network tag testnet-v0.1.19

INSERT OR REPLACE INTO network_listings (id, node_download_url, node_command_template, node_binary_sha256, updated_at)
VALUES (
  'boing-devnet',
  'https://github.com/Boing-Network/boing.network/releases/download/testnet-v0.1.19/release-windows-x86_64.zip',
  'boing-node-windows-x86_64.exe --data-dir {dataDir} --p2p-listen /ip4/0.0.0.0/tcp/4001 --bootnodes /ip4/169.155.48.188/tcp/4001 --rpc-port 8545 --faucet-enable',
  '5221a5deb5c8b4395d27231a78f60d687b027772dec588c4885e757b4957aded',
  datetime('now')
);

INSERT OR REPLACE INTO network_listings (id, node_download_url, node_command_template, node_binary_sha256, updated_at)
VALUES (
  'boing-devnet-linux',
  'https://github.com/Boing-Network/boing.network/releases/download/testnet-v0.1.19/release-linux-x86_64.zip',
  'boing-node-linux-x86_64 --data-dir {dataDir} --p2p-listen /ip4/0.0.0.0/tcp/4001 --bootnodes /ip4/169.155.48.188/tcp/4001 --rpc-port 8545 --faucet-enable',
  '687e90680d3302f03f7aebdc6ce095db0d845cad16e43781b896060af8130aa3',
  datetime('now')
);

INSERT OR REPLACE INTO network_listings (id, node_download_url, node_command_template, node_binary_sha256, updated_at)
VALUES (
  'boing-devnet-macos',
  'https://github.com/Boing-Network/boing.network/releases/download/testnet-v0.1.19/release-macos-aarch64.zip',
  'boing-node-macos-aarch64 --data-dir {dataDir} --p2p-listen /ip4/0.0.0.0/tcp/4001 --bootnodes /ip4/169.155.48.188/tcp/4001 --rpc-port 8545 --faucet-enable',
  'e18fffa1ff7bacd90df08aa0eb14f51cf7214b42bd14def17a1a81e34a012f11',
  datetime('now')
);
