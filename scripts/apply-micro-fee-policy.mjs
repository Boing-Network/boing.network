#!/usr/bin/env node
/**
 * Apply recommended micro fee policy (`execution_gas_fee_bps=100`) via
 * `boing_operatorApplyFeePolicy`.
 *
 *   node scripts/apply-micro-fee-policy.mjs
 *   BOING_RPC_URL=https://testnet-rpc.boing.network/ BOING_OPERATOR_RPC_TOKEN=... node scripts/apply-micro-fee-policy.mjs
 *
 * On history networks, set `BOING_MICRO_FEES_ACTIVATION_HEIGHT` on the node **before** or
 * alongside this so reduced BPS is not applied during replay. See docs/OPTIONAL-PRIORITY-FEES.md.
 */
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import https from 'node:https';
import http from 'node:http';
import { URL } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(__dirname, '..');

const raw = (process.env.BOING_RPC_URL || process.env.TESTNET_RPC_URL || 'https://testnet-rpc.boing.network').trim();
const rpcUrl = `${raw.replace(/\/+$/, '')}/`;
const operatorToken = (process.env.BOING_OPERATOR_RPC_TOKEN || '').trim();

function loadJson(relPath) {
  return JSON.parse(readFileSync(join(repoRoot, relPath), 'utf8'));
}

function rpcPost(urlString, jsonBody, headers = {}) {
  return new Promise((resolve, reject) => {
    const u = new URL(urlString);
    const body = Buffer.from(jsonBody, 'utf8');
    const lib = u.protocol === 'https:' ? https : http;
    const req = lib.request(
      {
        hostname: u.hostname,
        port: u.port || (u.protocol === 'https:' ? 443 : 80),
        path: `${u.pathname}${u.search}`,
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Content-Length': body.length,
          Accept: 'application/json',
          'User-Agent': 'boing-apply-micro-fee-policy/1',
          ...headers,
        },
      },
      (res) => {
        const chunks = [];
        res.on('data', (c) => chunks.push(c));
        res.on('end', () => {
          const text = Buffer.concat(chunks).toString('utf8');
          try {
            resolve({ status: res.statusCode ?? 0, json: JSON.parse(text), text });
          } catch {
            reject(new Error(`HTTP ${res.statusCode}: invalid JSON — ${text.slice(0, 200)}`));
          }
        });
      },
    );
    req.on('error', reject);
    req.write(body);
    req.end();
  });
}

const policy = loadJson('docs/config/network_fee_config.canonical.json');
const policyJson = JSON.stringify(policy);

const headers = {};
if (operatorToken) {
  headers['X-Boing-Operator'] = operatorToken;
}

console.log('RPC URL:', rpcUrl);
console.log('Applying micro fee policy (execution_gas_fee_bps=%s)…', policy.execution_gas_fee_bps);

const body = JSON.stringify({
  jsonrpc: '2.0',
  id: 1,
  method: 'boing_operatorApplyFeePolicy',
  params: [policyJson],
});

const { status, json, text } = await rpcPost(rpcUrl, body, headers);

if (status !== 200) {
  console.error('FAIL HTTP', status, text.slice(0, 300));
  process.exit(1);
}
if (json.error) {
  console.error('FAIL RPC', json.error.code, json.error.message);
  if (json.error.code === -32057) {
    console.error('Set BOING_OPERATOR_RPC_TOKEN to match the node operator secret.');
  }
  process.exit(1);
}
if (!json.result?.ok) {
  console.error('FAIL unexpected result:', JSON.stringify(json.result));
  process.exit(1);
}

console.log('OK    boing_operatorApplyFeePolicy');

const verifyBody = JSON.stringify({ jsonrpc: '2.0', id: 2, method: 'boing_getNetworkFeePolicy', params: [] });
const verify = await rpcPost(rpcUrl, verifyBody);
const live = verify.json?.result ?? {};
console.log(
  'OK    boing_getNetworkFeePolicy execution_gas_fee_bps=%s tip_proposer_bps=%s',
  live.execution_gas_fee_bps,
  live.tip_proposer_bps,
);

if (Number(live.execution_gas_fee_bps) !== Number(policy.execution_gas_fee_bps)) {
  console.warn(
    'WARN  live execution_gas_fee_bps=%s does not match applied %s (height gate may still clamp to full schedule)',
    live.execution_gas_fee_bps,
    policy.execution_gas_fee_bps,
  );
}

console.log('\nMicro fee policy applied. See docs/OPTIONAL-PRIORITY-FEES.md');
