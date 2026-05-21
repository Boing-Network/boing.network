#!/usr/bin/env node
/**
 * Apply public-testnet QA registry (with content blocklist) + pool config via
 * `boing_operatorApplyQaPolicy`.
 *
 *   node scripts/apply-public-testnet-qa-policy.mjs
 *   BOING_RPC_URL=https://testnet-rpc.boing.network/ BOING_OPERATOR_RPC_TOKEN=... node scripts/apply-public-testnet-qa-policy.mjs
 *
 * Requires operator auth when the node sets BOING_OPERATOR_RPC_TOKEN (header X-Boing-Operator).
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

function buildRegistryJson() {
  const base = loadJson('docs/config/qa_registry.canonical.json');
  const contentBlocklist = loadJson('docs/config/qa_content_blocklist.en.json');
  if (!Array.isArray(contentBlocklist)) {
    throw new Error('qa_content_blocklist.en.json must be a JSON array of strings');
  }
  const terms = [...new Set(contentBlocklist.map((t) => String(t).trim()).filter(Boolean))].sort((a, b) =>
    a.localeCompare(b),
  );
  return JSON.stringify({ ...base, content_blocklist: terms });
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
          'User-Agent': 'boing-apply-public-testnet-qa-policy/1',
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

const registryJson = buildRegistryJson();
const poolJson = JSON.stringify(loadJson('docs/config/qa_pool_config.public-testnet.json'));
const termCount = JSON.parse(registryJson).content_blocklist.length;

const headers = {};
if (operatorToken) {
  headers['X-Boing-Operator'] = operatorToken;
}

console.log('RPC URL:', rpcUrl);
console.log('Applying QA registry with', termCount, 'content_blocklist terms…');

const body = JSON.stringify({
  jsonrpc: '2.0',
  id: 1,
  method: 'boing_operatorApplyQaPolicy',
  params: [registryJson, poolJson],
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

console.log('OK    boing_operatorApplyQaPolicy');

const verifyBody = JSON.stringify({ jsonrpc: '2.0', id: 2, method: 'boing_getQaRegistry', params: [] });
const verify = await rpcPost(rpcUrl, verifyBody);
const liveCount = verify.json?.result?.content_blocklist?.length ?? 0;
console.log('OK    boing_getQaRegistry content_blocklist length =', liveCount);

const qaCheckBody = JSON.stringify({
  jsonrpc: '2.0',
  id: 3,
  method: 'boing_qaCheck',
  params: ['0x00', 'token', null, 'ShitCoin', 'SHIT'],
});
const qaCheck = await rpcPost(rpcUrl, qaCheckBody);
const qaResult = qaCheck.json?.result;
console.log(
  'QA smoke (ShitCoin):',
  qaResult?.result ?? qaCheck.json?.error?.message ?? 'unknown',
  qaResult?.rule_id ? `(${qaResult.rule_id})` : '',
);

if (qaResult?.result !== 'reject' || qaResult?.rule_id !== 'CONTENT_POLICY_VIOLATION') {
  console.warn('WARN  expected CONTENT_POLICY_VIOLATION reject for ShitCoin — verify manually');
}

console.log('\nPublic testnet QA content policy applied. See https://boing.observer/qa');
