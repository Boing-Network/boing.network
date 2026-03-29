#!/usr/bin/env node
/**
 * Fetch release zips from GitHub, print SHA-256 and SQL to refresh network_listings for a tag.
 *
 * Usage (from website/):
 *   node scripts/network-listings-release-sql.mjs testnet-v0.1.3
 *   node scripts/network-listings-release-sql.mjs testnet-v0.1.3 --apply
 *
 * --apply writes a temp .sql file and runs wrangler d1 execute --remote --file.
 * Prefer **CLOUDFLARE_API_TOKEN** (Dashboard → API Tokens, include D1 edit) if `wrangler login`
 * OAuth hits Authentication error [10000] on D1 import.
 *
 * Requires a *published* GitHub release for that tag (drafts return 404 on public download URLs).
 * Uses node:https instead of fetch() to avoid a Windows libuv crash (UV_HANDLE_CLOSING)
 * seen with Undici after failed or successful requests in some shells.
 */

import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { writeFileSync, mkdtempSync, rmSync, existsSync } from 'node:fs';
import https from 'node:https';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const WEBSITE_ROOT = join(__dirname, '..');

const OWNER_REPO = 'chiku524/boing.network';
const BOOT = '/ip4/73.84.106.121/tcp/4001';
const CMD_SUFFIX = `--data-dir {dataDir} --p2p-listen /ip4/0.0.0.0/tcp/4001 --bootnodes ${BOOT} --rpc-port 8545 --faucet-enable`;

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

/**
 * GET url (HTTPS only). Returns { statusCode, body }.
 * Follows one redirect (GitHub release assets often 302 to githubusercontent).
 */
function httpsGetBuffer(urlString) {
  return new Promise((resolve, reject) => {
    const tryOnce = (url, redirectDepth) => {
      const req = https.get(
        url,
        {
          headers: { 'User-Agent': 'boing-network-listings-script', Accept: '*/*' },
        },
        (res) => {
          const chunks = [];
          res.on('data', (c) => chunks.push(c));
          res.on('end', () => {
            const body = Buffer.concat(chunks);
            const loc = res.headers.location;
            if (res.statusCode >= 300 && res.statusCode < 400 && loc && redirectDepth < 5) {
              const next = new URL(loc, url).href;
              tryOnce(next, redirectDepth + 1);
              return;
            }
            resolve({ statusCode: res.statusCode ?? 0, body });
          });
        }
      );
      req.on('error', reject);
    };
    tryOnce(urlString, 0);
  });
}

const args = process.argv.slice(2).filter((a) => a !== '--apply');
const doApply = process.argv.includes('--apply');
const tag = args[0];

async function main() {
  if (!tag) {
    console.error('Usage: node scripts/network-listings-release-sql.mjs <release-tag> [--apply]');
    return 1;
  }

  const base = `https://github.com/${OWNER_REPO}/releases/download/${tag}`;
  const resolved = [];

  for (const row of ROWS) {
    const url = `${base}/${row.zip}`;
    let statusCode;
    let buf;
    try {
      const r = await httpsGetBuffer(url);
      statusCode = r.statusCode;
      buf = r.body;
    } catch (e) {
      console.error(`Failed ${url}: ${e.message || e}`);
      return 1;
    }
    if (statusCode !== 200) {
      console.error(`Failed ${url}: HTTP ${statusCode}`);
      if (statusCode === 404) {
        console.error('');
        console.error(`No file at that URL. Common causes:`);
        console.error(`  • Release is still a draft — GitHub hides draft assets from /releases/download/... (publish the release).`);
        console.error(`  • Tag "${tag}" has no release yet, or the release uses a different tag name.`);
        console.error(`  • Workflow has not finished uploading assets yet.`);
        console.error(`  • Asset names differ (expected: ${ROWS.map((r) => r.zip).join(', ')}).`);
        console.error('');
        console.error(`Working example: testnet-v0.1.3 — see GitHub → Releases.`);
      }
      return 1;
    }
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
        (r) =>
          `INSERT OR REPLACE INTO network_listings (id, node_download_url, node_command_template, node_binary_sha256, updated_at) VALUES ('${sqlEscape(r.id)}', '${sqlEscape(r.url)}', '${sqlEscape(r.template)}', '${sqlEscape(r.sha)}', datetime('now'));`
      )
      .join('\n');
    writeFileSync(sqlPath, body, 'utf8');
    const wranglerJs = join(WEBSITE_ROOT, 'node_modules', 'wrangler', 'bin', 'wrangler.js');
    if (!existsSync(wranglerJs)) {
      console.error('Missing website/node_modules/wrangler. Run: cd website && npm install');
      return 1;
    }
    const r = spawnSync(
      process.execPath,
      [wranglerJs, 'd1', 'execute', 'boing-network-db', '--remote', '--yes', '--file', sqlPath],
      { cwd: WEBSITE_ROOT, stdio: 'inherit' }
    );
    try {
      rmSync(dir, { recursive: true });
    } catch {
      /* ignore */
    }
    if (r.status !== 0) return r.status ?? 1;
  }

  return 0;
}

main()
  .then((code) => {
    setImmediate(() => process.exit(code));
  })
  .catch((e) => {
    console.error(e);
    setImmediate(() => process.exit(1));
  });
