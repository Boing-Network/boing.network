-- network_listings refresh for Boing-Network/boing.network tag testnet-v0.1.22

INSERT OR REPLACE INTO network_listings (id, node_download_url, node_command_template, node_binary_sha256, updated_at)
VALUES (
  'boing-devnet',
  'https://github.com/Boing-Network/boing.network/releases/download/testnet-v0.1.22/release-windows-x86_64.zip',
  'boing-node-windows-x86_64.exe --data-dir {dataDir} --p2p-listen /ip4/0.0.0.0/tcp/4001 --bootnodes /ip4/169.155.48.188/tcp/4001 --rpc-port 8545 --faucet-enable --validator-set open --open-validator-activation-height 400 --name-registry-activation-height 640',
  'c829e6c132eff99fe9fcdf10c30e0999400e80d704c6296cedb3b89d94393770',
  datetime('now')
);

INSERT OR REPLACE INTO network_listings (id, node_download_url, node_command_template, node_binary_sha256, updated_at)
VALUES (
  'boing-devnet-linux',
  'https://github.com/Boing-Network/boing.network/releases/download/testnet-v0.1.22/release-linux-x86_64.zip',
  'boing-node-linux-x86_64 --data-dir {dataDir} --p2p-listen /ip4/0.0.0.0/tcp/4001 --bootnodes /ip4/169.155.48.188/tcp/4001 --rpc-port 8545 --faucet-enable --validator-set open --open-validator-activation-height 400 --name-registry-activation-height 640',
  '98238057cef70524c0ce895fb55491a1817be13aa65f129e650a408a5b1d4a30',
  datetime('now')
);

INSERT OR REPLACE INTO network_listings (id, node_download_url, node_command_template, node_binary_sha256, updated_at)
VALUES (
  'boing-devnet-macos',
  'https://github.com/Boing-Network/boing.network/releases/download/testnet-v0.1.22/release-macos-aarch64.zip',
  'boing-node-macos-aarch64 --data-dir {dataDir} --p2p-listen /ip4/0.0.0.0/tcp/4001 --bootnodes /ip4/169.155.48.188/tcp/4001 --rpc-port 8545 --faucet-enable --validator-set open --open-validator-activation-height 400 --name-registry-activation-height 640',
  '6777fdad66f3fd1309e8889c01183cbb961042f9d5c1f26be6be1918dca8d9f6',
  datetime('now')
);
