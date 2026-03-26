-- One-time seed / update for VibeMiner: merge into GET /api/networks (id must be exactly boing-devnet).
-- Apply to production D1, e.g.:
--   cd website && npx wrangler d1 execute boing-network-db --remote --file=migrations/insert-boing-devnet-listing.sql
--
-- Edit this file before running: set the release tag in the URL and optional SHA-256 of the downloaded archive/binary.
-- The Windows release zip from .github/workflows/release.yml contains boing-node-windows-x86_64.exe at the archive root.
-- node_command_template must match the binary name inside that zip (or your custom zip layout).

INSERT OR REPLACE INTO network_listings (id, node_download_url, node_command_template, node_binary_sha256, updated_at)
VALUES (
  'boing-devnet',
  'https://github.com/chiku524/boing.network/releases/download/<YOUR_RELEASE_TAG>/release-windows-x86_64.zip',
  'boing-node-windows-x86_64.exe --data-dir {dataDir} --p2p-listen /ip4/0.0.0.0/tcp/4001 --bootnodes /ip4/73.84.106.121/tcp/4001 --rpc-port 8545',
  NULL,
  datetime('now')
);
