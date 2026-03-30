#!/usr/bin/env node
/**
 * Verify JSON-RPC on the public testnet URL (default: https://testnet-rpc.boing.network/).
 * Uses node:https (not fetch) to avoid Windows libuv crashes seen with Undici after requests.
 *
 * Usage:
 *   node scripts/verify-public-testnet-rpc.mjs
 *   TESTNET_RPC_URL=https://testnet-rpc.boing.network/ node scripts/verify-public-testnet-rpc.mjs
 */
import https from 'node:https';
import { URL } from 'node:url';

const raw = (process.env.TESTNET_RPC_URL || 'https://testnet-rpc.boing.network').trim();
const url = `${raw.replace(/\/+$/, '')}/`;

const checks = [
  { method: 'boing_chainHeight', params: [], name: 'chain height' },
  { method: 'boing_getQaRegistry', params: [], name: 'QA registry (explorer transparency)' },
  { method: 'boing_qaPoolConfig', params: [], name: 'QA pool config' },
];

/**
 * POST JSON-RPC body; resolves parsed JSON (even when JSON-RPC error object present).
 * Rejects on network error or non-2xx HTTP (message includes status code).
 */
function rpcPost(urlString, jsonBody) {
  return new Promise((resolve, reject) => {
    const u = new URL(urlString);
    const body = Buffer.from(jsonBody, 'utf8');
    const opts = {
      hostname: u.hostname,
      port: u.port || 443,
      path: `${u.pathname}${u.search}`,
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Content-Length': body.length,
        Accept: 'application/json',
        'User-Agent': 'boing-verify-public-testnet-rpc/2',
      },
    };

    const req = https.request(opts, (res) => {
      const chunks = [];
      res.on('data', (c) => chunks.push(c));
      res.on('end', () => {
        const text = Buffer.concat(chunks).toString('utf8');
        const code = res.statusCode ?? 0;
        if (code !== 200) {
          const snippet = text.replace(/\s+/g, ' ').trim().slice(0, 120);
          reject(
            new Error(
              `HTTP ${code} ${res.statusMessage || ''}${snippet ? ` — body: ${snippet}` : ''}`
            )
          );
          return;
        }
        try {
          resolve(JSON.parse(text));
        } catch {
          reject(new Error(`HTTP 200 but invalid JSON: ${text.slice(0, 200)}`));
        }
      });
    });
    req.on('error', reject);
    req.write(body);
    req.end();
  });
}

async function rpc(method, params) {
  const jsonBody = JSON.stringify({ jsonrpc: '2.0', id: 1, method, params });
  return rpcPost(url, jsonBody);
}

let failed = false;
let saw405 = false;
console.log('RPC URL:', url);
for (const { method, params, name } of checks) {
  try {
    const j = await rpc(method, params);
    if (j.error) {
      failed = true;
      console.log(`FAIL  ${method} (${name}):`, j.error.message || JSON.stringify(j.error));
    } else {
      console.log(`OK    ${method} (${name})`);
    }
  } catch (e) {
    failed = true;
    const msg = e instanceof Error ? e.message : String(e);
    if (msg.includes('HTTP 405')) saw405 = true;
    console.log(`FAIL  ${method} (${name}):`, msg);
  }
}

if (failed) {
  if (saw405) {
    console.log(`
HTTP 405 = the hostname is NOT talking JSON-RPC POST to boing-node (or something strips POST).

Checklist:
  1) DNS: In Cloudflare → boing.network → DNS, the record for **testnet-rpc** must route to your
     **Tunnel** (CNAME to *.cfargotunnel.com), not to Pages, an A record to a web server, or a proxy
     that only serves GET.
  2) Duplicate records: Remove any extra A/AAAA/CNAME for testnet-rpc that point elsewhere.
  3) Tunnel ingress: cloudflared config must map **https://testnet-rpc.boing.network** → **http://127.0.0.1:8545**
     (the process where boing-node listens). VibeMiner's tunnel uses YOUR local config — if the public
     hostname is not in that config, global DNS can still point to a different target → 405.
  4) Prove local RPC: on the PC running the node:
       curl -s -X POST http://127.0.0.1:8545 -H "Content-Type: application/json" \\
         -d "{\\"jsonrpc\\":\\"2.0\\",\\"id\\":1,\\"method\\":\\"boing_chainHeight\\",\\"params\\":[]}"
     If that works but HTTPS 405 fails, the problem is Cloudflare/Tunnel/DNS, not the node binary.
  5) WAF / Rules: Cloudflare "Block" or odd Transform rules on POST for this hostname — allow POST to origin.

`);
  } else {
    console.log(`
If QA methods failed but chain height works, the node behind this URL is an older
boing-node build. On the primary machine that serves localhost:8545 to Cloudflare:
  1) git pull && cargo build --release  (or install zips from the latest testnet-v* release)
  2) Stop the old node; start the new binary with the same flags (see scripts/start-bootnode-1.* and INFRASTRUCTURE-SETUP.md).
  3) Keep cloudflared running (testnet-rpc.boing.network -> http://127.0.0.1:8545).
Re-run this script until all checks pass.
`);
  }
  process.exit(1);
}

process.exit(0);
