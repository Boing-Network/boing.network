-- network_listings refresh for Boing-Network/boing.network tag testnet-v0.1.23

INSERT OR REPLACE INTO network_listings (id, node_download_url, node_command_template, node_binary_sha256, updated_at)
VALUES (
  'boing-devnet',
  'https://github.com/Boing-Network/boing.network/releases/download/testnet-v0.1.23/release-windows-x86_64.zip',
  'boing-node-windows-x86_64.exe --data-dir {dataDir} --p2p-listen /ip4/0.0.0.0/tcp/4001 --bootnodes /ip4/169.155.48.188/tcp/4001 --rpc-port 8545 --faucet-enable --validator-set open --open-validator-activation-height 400 --name-registry-activation-height 640',
  'e97d8e80e24260a35bc5924e542c4c23b06b6223a7a280e8b70a1aaa842fdc46',
  datetime('now')
);

INSERT OR REPLACE INTO network_listings (id, node_download_url, node_command_template, node_binary_sha256, updated_at)
VALUES (
  'boing-devnet-linux',
  'https://github.com/Boing-Network/boing.network/releases/download/testnet-v0.1.23/release-linux-x86_64.zip',
  'boing-node-linux-x86_64 --data-dir {dataDir} --p2p-listen /ip4/0.0.0.0/tcp/4001 --bootnodes /ip4/169.155.48.188/tcp/4001 --rpc-port 8545 --faucet-enable --validator-set open --open-validator-activation-height 400 --name-registry-activation-height 640',
  '69abb7650fcb2a909f9f84f992e0e79c8c9a8d2be5b172d68ec373878beb3d2f',
  datetime('now')
);

INSERT OR REPLACE INTO network_listings (id, node_download_url, node_command_template, node_binary_sha256, updated_at)
VALUES (
  'boing-devnet-macos',
  'https://github.com/Boing-Network/boing.network/releases/download/testnet-v0.1.23/release-macos-aarch64.zip',
  'boing-node-macos-aarch64 --data-dir {dataDir} --p2p-listen /ip4/0.0.0.0/tcp/4001 --bootnodes /ip4/169.155.48.188/tcp/4001 --rpc-port 8545 --faucet-enable --validator-set open --open-validator-activation-height 400 --name-registry-activation-height 640',
  'efca4b634029cd91c5db3dde3c894d3e5ceaf568ba3000f46cdf58358aec0b42',
  datetime('now')
);
