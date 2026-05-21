#!/usr/bin/env node
/**
 * Compare live `boing_getQaRegistry` / `boing_qaPoolConfig` against canonical JSON in docs/config/.
 *
 *   node scripts/verify-qa-registry-alignment.mjs
 *   TESTNET_RPC_URL=https://testnet-rpc.boing.network/ node scripts/verify-qa-registry-alignment.mjs
 *   BOING_QA_ALIGNMENT_STRICT=1 node scripts/verify-qa-registry-alignment.mjs
 *
 * Exit 0: RPC reachable and (unless strict) report printed.
 * Exit 1: RPC failure, missing QA methods, or strict diff vs canonical registry/pool config.
 */
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import https from 'node:https';
import { URL } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(__dirname, '..');

const raw = (process.env.TESTNET_RPC_URL || process.env.BOING_RPC_URL || 'https://testnet-rpc.boing.network').trim();
const rpcUrl = `${raw.replace(/\/+$/, '')}/`;
const strict =
  process.env.BOING_QA_ALIGNMENT_STRICT === '1' || process.env.BOING_QA_ALIGNMENT_STRICT === 'true';

function rpcPost(urlString, jsonBody) {
  return new Promise((resolve, reject) => {
    const u = new URL(urlString);
    const body = Buffer.from(jsonBody, 'utf8');
    const req = https.request(
      {
        hostname: u.hostname,
        port: u.port || 443,
        path: `${u.pathname}${u.search}`,
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Content-Length': body.length,
          Accept: 'application/json',
          'User-Agent': 'boing-verify-qa-registry-alignment/1',
        },
      },
      (res) => {
        const chunks = [];
        res.on('data', (c) => chunks.push(c));
        res.on('end', () => {
          const text = Buffer.concat(chunks).toString('utf8');
          const code = res.statusCode ?? 0;
          if (code !== 200) {
            reject(new Error(`HTTP ${code} — ${text.replace(/\s+/g, ' ').trim().slice(0, 160)}`));
            return;
          }
          try {
            resolve(JSON.parse(text));
          } catch {
            reject(new Error(`HTTP 200 but invalid JSON: ${text.slice(0, 200)}`));
          }
        });
      },
    );
    req.on('error', reject);
    req.write(body);
    req.end();
  });
}

async function rpc(method, params = []) {
  const envelope = await rpcPost(
    rpcUrl,
    JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
  );
  if (envelope.error) {
    throw new Error(`${method}: ${envelope.error.message ?? JSON.stringify(envelope.error)}`);
  }
  return envelope.result;
}

function loadJson(relPath) {
  return JSON.parse(readFileSync(join(repoRoot, relPath), 'utf8'));
}

function stableStringify(value) {
  return JSON.stringify(value, (_k, v) => {
    if (v && typeof v === 'object' && !Array.isArray(v)) {
      return Object.keys(v)
        .sort()
        .reduce((acc, key) => {
          acc[key] = v[key];
          return acc;
        }, {});
    }
    return v;
  });
}

function diffSummary(label, live, canonical) {
  const liveS = stableStringify(live);
  const canonS = stableStringify(canonical);
  if (liveS === canonS) {
    console.log(`OK    ${label} matches docs/config canonical JSON`);
    return false;
  }
  console.log(`DIFF  ${label} differs from docs/config canonical JSON`);
  console.log(`      canonical keys: ${Object.keys(canonical ?? {}).join(', ') || '(none)'}`);
  console.log(`      live keys:      ${Object.keys(live ?? {}).join(', ') || '(none)'}`);
  return true;
}

let failed = false;

console.log('RPC URL:', rpcUrl);
console.log('Mode:', strict ? 'strict (exit 1 on diff)' : 'report-only');

try {
  const height = await rpc('boing_chainHeight');
  console.log('OK    boing_chainHeight =', height);

  const canonicalRegistry = loadJson('docs/config/qa_registry.canonical.json');
  const canonicalPool = loadJson('docs/config/qa_pool_config.canonical.json');

  const liveRegistry = await rpc('boing_getQaRegistry');
  const livePool = await rpc('boing_qaPoolConfig');

  if (diffSummary('boing_getQaRegistry', liveRegistry, canonicalRegistry)) {
    if (strict) failed = true;
    console.log('      Hint: live policy may differ after governance — compare manually at https://boing.observer/qa');
  }

  if (diffSummary('boing_qaPoolConfig (summary)', livePool, canonicalPool)) {
    if (strict) failed = true;
    console.log('      Hint: pool admins/capacity may be operator-configured — see docs/config/CANONICAL-QA-REGISTRY.md');
  }

  const poolList = await rpc('boing_qaPoolList', [{ limit: 5 }]).catch((e) => {
    console.log('WARN  boing_qaPoolList:', e.message);
    return null;
  });
  if (poolList && typeof poolList === 'object') {
    const pending = poolList.pending?.length ?? poolList.items?.length ?? '?';
    console.log(`OK    boing_qaPoolList reachable (pending sample: ${pending})`);
  }
} catch (e) {
  console.error('FAIL ', e.message ?? e);
  console.error(`
Public RPC must expose QA transparency methods for explorer /qa and this script.
If chain height fails with HTTP 530 / error 1033, restart boing-node + cloudflared (docs/RUNBOOK.md §8.3).
If height works but QA methods 404, upgrade the tunnel node binary (docs/INFRASTRUCTURE-SETUP.md).
`);
  process.exit(1);
}

if (failed) {
  console.error('\nStrict QA alignment check failed (live ≠ canonical).');
  process.exit(1);
}

console.log('\nQA alignment pass complete.');
