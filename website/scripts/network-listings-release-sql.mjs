#!/usr/bin/env node
/**
 * Fetch release zips from GitHub, print SHA-256 and SQL to refresh network_listings for a tag.
 *
 * Usage (from website/):
 *   node scripts/network-listings-release-sql.mjs testnet-v0.1.2
 *   node scripts/network-listings-release-sql.mjs testnet-v0.1.2 --apply
 *
 * --apply writes a temp .sql file and runs wrangler d1 execute --remote --file (requires auth).
 */

import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { writeFileSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const WEBSITE_ROOT = join(__dirname, '..');

const OWNER_REPO = 'chiku524/boing.network';
const BOOT = '/ip4/73.84.106.121/tcp/4001';
const CMD_SUFFIX = `--data-dir {dataDir} --p2p-listen /ip4/0.0.0.0/tcp/4001 --bootnodes ${BOOT} --rpc-port 8545`;

const ROWS = [
  {
    id: 'boing-devnet',
    zip: 'release-windows-x86_64.zip',
    template: `boing-node-windows-x86_64.exe ${CMD_SUFFIX}`,
  },
  {
    id: 'boing-devnet-linux',
    zip: 'release-linux-x86_64.zip',
    template: `boing-node-linux-x86_64 ${CMD_SUFFIX}`,
  },
  {
    id: 'boing-devnet-macos',
    zip: 'release-macos-aarch64.zip',
    template: `boing-node-macos-aarch64 ${CMD_SUFFIX}`,
  },
];

function sqlEscape(s) {
  return String(s).replace(/'/g, "''");
}

const args = process.argv.slice(2).filter((a) => a !== '--apply');
const doApply = process.argv.includes('--apply');
const tag = args[0];

if (!tag) {
  console.error('Usage: node scripts/network-listings-release-sql.mjs <release-tag> [--apply]');
  process.exit(1);
}

const base = `https://github.com/${OWNER_REPO}/releases/download/${tag}`;

const resolved = [];
for (const row of ROWS) {
  const url = `${base}/${row.zip}`;
  const res = await fetch(url);
  if (!res.ok) {
    console.error(`Failed ${url}: HTTP ${res.status}`);
    process.exit(1);
  }
  const buf = Buffer.from(await res.arrayBuffer());
  const sha = createHash('sha256').update(buf).digest('hex');
  resolved.push({ ...row, url, sha, size: buf.length });
  console.error(`ok ${row.id} ${row.zip} ${buf.length} bytes sha256=${sha}`);
}

console.log(`-- network_listings refresh for ${OWNER_REPO} tag ${tag}`);
for (const r of resolved) {
  console.log(`
INSERT OR REPLACE INTO network_listings (id, node_download_url, node_command_template, node_binary_sha256, updated_at)
VALUES (
  '${sqlEscape(r.id)}',
  '${sqlEscape(r.url)}',
  '${sqlEscape(r.template)}',
  '${sqlEscape(r.sha)}',
  datetime('now')
);`);
}

if (doApply) {
  const dir = mkdtempSync(join(tmpdir(), 'boing-net-listings-'));
  const sqlPath = join(dir, 'apply.sql');
  const body = resolved
    .map(
      (r) => `INSERT OR REPLACE INTO network_listings (id, node_download_url, node_command_template, node_binary_sha256, updated_at) VALUES ('${sqlEscape(r.id)}', '${sqlEscape(r.url)}', '${sqlEscape(r.template)}', '${sqlEscape(r.sha)}', datetime('now'));`
    )
    .join('\n');
  writeFileSync(sqlPath, body, 'utf8');
  const r = spawnSync(
    'npx',
    ['wrangler', 'd1', 'execute', 'boing-network-db', '--remote', '--file', sqlPath],
    { cwd: WEBSITE_ROOT, stdio: 'inherit', shell: true }
  );
  try {
    rmSync(dir, { recursive: true });
  } catch {
    /* ignore */
  }
  process.exit(r.status ?? 1);
}
