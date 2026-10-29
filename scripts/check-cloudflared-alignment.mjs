#!/usr/bin/env node
/**
 * Check that ~/.cloudflared/config.yml routes testnet-rpc.boing.network → local :8545.
 * Does not modify files. Exits 1 if missing or misaligned.
 *
 *   node scripts/check-cloudflared-alignment.mjs
 *   CLOUDFLARED_CONFIG=C:\path\to\config.yml node scripts/check-cloudflared-alignment.mjs
 */
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';

const PUBLIC_HOST = 'testnet-rpc.boing.network';
const EXPECT_TUNNEL_NAME = 'boing-testnet-rpc';

const configPath =
  process.env.CLOUDFLARED_CONFIG?.trim() ||
  path.join(os.homedir(), '.cloudflared', 'config.yml');

console.log('Config file:', configPath);

if (!fs.existsSync(configPath)) {
  console.error(`
NOT FOUND. Create it from docs/cloudflared-config.example.yml after:
  cloudflared tunnel login
  cloudflared tunnel create ${EXPECT_TUNNEL_NAME}
  Zero Trust → Tunnels → Public hostname: ${PUBLIC_HOST} → http://127.0.0.1:8545
`);
  process.exit(1);
}

const text = fs.readFileSync(configPath, 'utf8');
const issues = [];

if (!text.includes(PUBLIC_HOST)) {
  issues.push(`Missing hostname "${PUBLIC_HOST}" in ingress.`);
}
if (!/127\.0\.0\.1:8545/.test(text) && !/localhost:8545/.test(text)) {
  issues.push('Missing service http://127.0.0.1:8545 (or localhost:8545) in ingress.');
}

if (issues.length) {
  console.error('ALIGNMENT ISSUES:\n', issues.map((s) => `  - ${s}`).join('\n'));
  console.error(`
Add this block under top-level "ingress:" (before the catch-all 404 rule):

  - hostname: ${PUBLIC_HOST}
    service: http://127.0.0.1:8545

Full example: docs/cloudflared-config.example.yml

Cloudflare dashboard:
  - "Custom domain" on Pages/Workers is NOT the tunnel. Remove wrong DNS for testnet-rpc.
  - Zero Trust → Tunnels → Public hostname must match ${PUBLIC_HOST} → http://127.0.0.1:8545
  - DNS → testnet-rpc should be the tunnel CNAME (cfargotunnel.com), not an A record to a web server.
`);
  process.exit(1);
}

console.log(`OK: ${PUBLIC_HOST} and local :8545 appear in ${path.basename(configPath)}.`);
console.log(`Default tunnel name matches repo scripts: "${EXPECT_TUNNEL_NAME}".`);
console.log(`
If https://${PUBLIC_HOST}/ still returns HTTP 405 for POST:
  - cloudflared must be RUNNING (scripts/start-cloudflare-tunnel, or: cloudflared tunnel --config ... run ${EXPECT_TUNNEL_NAME})
  - Cloudflare → DNS: only ONE record for testnet-rpc — the tunnel CNAME (*.cfargotunnel.com). Delete extras.
  - Zero Trust → Tunnels → this tunnel → confirm public hostname ${PUBLIC_HOST} → http://127.0.0.1:8545
  - WAF / Transform rules: allow POST for this hostname
`);
console.log('Next: start tunnel + node scripts/verify-public-testnet-rpc.mjs');
process.exit(0);
