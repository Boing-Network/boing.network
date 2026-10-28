#!/usr/bin/env node
/**
 * Propose a storage-tree v2 activation height from the live testnet head. READ-ONLY: it only issues
 * `boing_health`, `boing_getNetworkInfo` and `boing_getBlockByHeight`; it never changes a node.
 *
 * Usage:
 *   node scripts/compute-activation-height.mjs
 *   node scripts/compute-activation-height.mjs --hours 48 --min-burst-hours 2 --window 500
 *   node scripts/compute-activation-height.mjs --rpc https://testnet-rpc.boing.network --json
 *
 * The result is only valid for the moment it was computed. Re-run it on the day of the rollout and
 * announce the NEW number to every operator (docs/STORAGE-TREE-V2-ACTIVATION.md).
 * Mainnet (chain 6914) is refused on purpose: the mainnet height is a human decision behind the gate
 * criteria in that document.
 */
import https from 'node:https';
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { computeActivationHeight } from './lib/activation-height.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));

function arg(name, fallback) {
  const i = process.argv.indexOf(`--${name}`);
  return i >= 0 && process.argv[i + 1] !== undefined ? process.argv[i + 1] : fallback;
}
const flag = (name) => process.argv.includes(`--${name}`);

if (flag('help') || flag('h')) {
  console.log(fs.readFileSync(fileURLToPath(import.meta.url), 'utf8').split('*/')[0]);
  process.exit(0);
}

function defaultRpc() {
  try {
    const meta = JSON.parse(fs.readFileSync(path.join(here, '..', 'docs', 'chain-metadata.json'), 'utf8'));
    return meta.networks.find((n) => n.key === 'testnet').rpc_urls[0];
  } catch {
    return 'https://testnet-rpc.boing.network';
  }
}

const rpcUrl = (arg('rpc', process.env.TESTNET_RPC_URL || defaultRpc())).replace(/\/+$/, '');
const hours = Number(arg('hours', '48'));
const minBurstHours = Number(arg('min-burst-hours', '2'));
const windowBlocks = Math.max(2, Number(arg('window', '500')));
const burstWindow = Math.max(2, Number(arg('burst-window', '50')));
const round = Number(arg('round', '100'));
const asJson = flag('json');

/** node:http(s) instead of fetch: Undici has crashed on Windows after bursts of requests. */
function post(url, payload) {
  return new Promise((resolve, reject) => {
    const u = new URL(url);
    const body = Buffer.from(JSON.stringify(payload), 'utf8');
    const lib = u.protocol === 'http:' ? http : https;
    const req = lib.request(
      u,
      { method: 'POST', headers: { 'Content-Type': 'application/json', 'Content-Length': body.length, 'User-Agent': 'boing-compute-activation-height/1' }, timeout: 30_000 },
      (res) => {
        const chunks = [];
        res.on('data', (c) => chunks.push(c));
        res.on('end', () => {
          const text = Buffer.concat(chunks).toString('utf8');
          if (res.statusCode < 200 || res.statusCode >= 300) return reject(new Error(`HTTP ${res.statusCode}: ${text.slice(0, 160)}`));
          try {
            resolve(JSON.parse(text));
          } catch {
            reject(new Error(`non-JSON response: ${text.slice(0, 160)}`));
          }
        });
      },
    );
    req.on('timeout', () => req.destroy(new Error('timeout')));
    req.on('error', reject);
    req.end(body);
  });
}

async function call(method, params = []) {
  const j = await post(rpcUrl, { jsonrpc: '2.0', id: 1, method, params });
  if (j.error) throw new Error(`${method}: ${j.error.message}`);
  return j.result;
}

/**
 * Blocks carry their transactions (some are large), and the public gateway rejects responses over
 * ~6 MiB, so fetch in small batches and fall back to single requests when a batch is refused.
 */
async function blocks(heights) {
  const out = [];
  const take = (item) => {
    const header = item?.result?.header;
    if (header && Number.isFinite(header.timestamp)) out.push({ height: header.height, timestamp: header.timestamp });
  };
  const single = async (h) => {
    for (let attempt = 0; attempt < 4; attempt += 1) {
      try {
        const r = await post(rpcUrl, { jsonrpc: '2.0', id: 1, method: 'boing_getBlockByHeight', params: [h, false] });
        take(r);
        return;
      } catch {
        await new Promise((r) => setTimeout(r, 300 * (attempt + 1)));
      }
    }
  };
  for (let i = 0; i < heights.length; i += 8) {
    const slice = heights.slice(i, i + 8);
    const batch = slice.map((h, k) => ({ jsonrpc: '2.0', id: k, method: 'boing_getBlockByHeight', params: [h, false] }));
    let res = null;
    try {
      res = await post(rpcUrl, batch);
    } catch {
      res = null;
    }
    if (Array.isArray(res)) res.forEach(take);
    else for (const h of slice) await single(h);
  }
  return out.sort((a, b) => a.height - b.height);
}
const health = await call('boing_health');
if (health.chain_id === 6914) {
  console.error('Refusing to propose a MAINNET height. Follow the mainnet gate in docs/STORAGE-TREE-V2-ACTIVATION.md.');
  process.exit(2);
}
const info = await call('boing_getNetworkInfo').catch(() => ({}));
const head = health.head_height;
const from = Math.max(1, head - windowBlocks);
const stride = Math.max(1, Math.ceil((head - from) / 60));
const heights = [];
for (let h = from; h < head; h += stride) heights.push(h);
heights.push(head);
const samples = await blocks(heights);
if (samples.length < 2) throw new Error('could not read enough blocks to estimate the block time');
const headBlock = samples[samples.length - 1];

const result = computeActivationHeight({
  head: headBlock.height,
  headTimestamp: headBlock.timestamp,
  samples,
  hours,
  minBurstHours,
  burstWindow,
  round,
  targetBlockTimeSecs: Number(info.target_block_time_secs) || 2,
  nowUnix: Math.floor(Date.now() / 1000),
});

const configured = info.storage_tree_v2_activation_height;
const report = {
  rpc: rpcUrl,
  chain_id: health.chain_id,
  client_version: health.client_version,
  computed_at: new Date().toISOString(),
  node_publishes_activation_field: Object.prototype.hasOwnProperty.call(info, 'storage_tree_v2_activation_height'),
  node_activation_height: configured ?? null,
  ...result,
  headTimestampIso: new Date(result.headTimestamp * 1000).toISOString(),
  eta: {
    atAverageRate: new Date(result.etaUnix.atAverageRate * 1000).toISOString(),
    atBurstRate: new Date(result.etaUnix.atBurstRate * 1000).toISOString(),
    atTargetRate: new Date(result.etaUnix.atTargetRate * 1000).toISOString(),
  },
};

if (asJson) {
  console.log(JSON.stringify(report, null, 2));
} else {
  const f = (n) => n.toFixed(1);
  console.log(`RPC                     ${rpcUrl} (chain ${health.chain_id}, ${health.client_version})`);
  console.log(`Head                    ${result.head} (block timestamp ${report.headTimestampIso}; ETAs below count from now, ${report.computed_at})`);
  console.log(`Block time              avg ${f(result.avgBlockSecs)} s over ${result.windowBlocks} blocks; fastest ${burstWindow}-block burst ${f(result.burstBlockSecs)} s; target ${result.targetBlockSecs} s`);
  console.log(`Blocks for ${hours} h        ${result.expectedBlocks} at the average rate`);
  console.log(`Blocks for ${minBurstHours} h burst  ${result.burstFloorBlocks} at the fastest recent burst rate (floor)`);
  console.log(`Binding constraint      ${result.binding}`);
  console.log('');
  console.log(`PROPOSED ACTIVATION HEIGHT  ${result.height}   (head + ${result.blocksAway})`);
  console.log(`  reached at avg rate    ${report.eta.atAverageRate}`);
  console.log(`  reached at burst rate  ${report.eta.atBurstRate}   <- earliest realistic`);
  console.log(`  reached at target rate ${report.eta.atTargetRate}   <- only if the chain suddenly runs at ${result.targetBlockSecs} s/block`);
  console.log('');
  if (!report.node_publishes_activation_field) {
    console.log('NOTE: this node does not publish storage_tree_v2_activation_height yet, so it is running a binary older than 9933787. Roll out the new binary FIRST (step 1 of the runbook).');
  } else if (configured !== null && configured !== undefined) {
    console.log(`WARNING: this node already reports storage_tree_v2_activation_height=${configured}.`);
  }
  console.log('Valid only for this head. Recompute on the day of rollout, announce the new value, then set');
  console.log('BOING_STORAGE_TREE_V2_ACTIVATION_HEIGHT=<height> on EVERY testnet node BEFORE the chain reaches it.');
}