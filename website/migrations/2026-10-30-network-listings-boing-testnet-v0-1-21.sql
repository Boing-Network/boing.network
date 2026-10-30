-- network_listings refresh for Boing-Network/boing.network tag testnet-v0.1.21

INSERT OR REPLACE INTO network_listings (id, node_download_url, node_command_template, node_binary_sha256, updated_at)
VALUES (
  'boing-devnet',
  'https://github.com/Boing-Network/boing.network/releases/download/testnet-v0.1.21/release-windows-x86_64.zip',
  'boing-node-windows-x86_64.exe --data-dir {dataDir} --p2p-listen /ip4/0.0.0.0/tcp/4001 --bootnodes /ip4/169.155.48.188/tcp/4001 --rpc-port 8545 --faucet-enable',
  '6b15c32deee46a6e17b252fac0050a992994c96c1447c930013f180bb4081a5f',
  datetime('now')
);

INSERT OR REPLACE INTO network_listings (id, node_download_url, node_command_template, node_binary_sha256, updated_at)
VALUES (
  'boing-devnet-linux',
  'https://github.com/Boing-Network/boing.network/releases/download/testnet-v0.1.21/release-linux-x86_64.zip',
  'boing-node-linux-x86_64 --data-dir {dataDir} --p2p-listen /ip4/0.0.0.0/tcp/4001 --bootnodes /ip4/169.155.48.188/tcp/4001 --rpc-port 8545 --faucet-enable',
  'ecf3dd80d9e52c1b73387c7c437eda11ea470f6fc149e98a2fa7d7f7e5eaecca',
  datetime('now')
);

INSERT OR REPLACE INTO network_listings (id, node_download_url, node_command_template, node_binary_sha256, updated_at)
VALUES (
  'boing-devnet-macos',
  'https://github.com/Boing-Network/boing.network/releases/download/testnet-v0.1.21/release-macos-aarch64.zip',
  'boing-node-macos-aarch64 --data-dir {dataDir} --p2p-listen /ip4/0.0.0.0/tcp/4001 --bootnodes /ip4/169.155.48.188/tcp/4001 --rpc-port 8545 --faucet-enable',
  'e11e4301eee6edaa96983a0122a5b095e6cfe5d51e77d04a4279a0d8825ad1d5',
  datetime('now')
);
