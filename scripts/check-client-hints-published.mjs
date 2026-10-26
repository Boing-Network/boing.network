#!/usr/bin/env node
/**
 * Online guard for client_hints defaults: fails when the defaults in crates/boing-node/src/rpc.rs
 * reference a release that has not been published.
 *   - recommended_sdk must exist on the npm registry (@boing-network/boing-sdk) and equal boing-sdk/package.json
 *   - recommended_wallet_version / min_wallet_version must be <= the extension version on boing.express main
 *     (override the source with BOING_EXPRESS_MANIFEST_URL)
 * Chrome Web Store propagation is NOT checked (no public API); confirm the store listing by hand.
 *
 *   node scripts/check-client-hints-published.mjs
 */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), '..');
const rs = fs.readFileSync(path.join(root, 'crates/boing-node/src/rpc.rs'), 'utf8');
const konst = (n) => rs.match(new RegExp(`const ${n}: &str = "([^"]+)";`))?.[1];
const min = konst('DEFAULT_MIN_WALLET_VERSION');
const rec = konst('DEFAULT_RECOMMENDED_WALLET_VERSION');
const sdk = konst('DEFAULT_RECOMMENDED_SDK_VERSION');
const cmp = (a, b) => {
  const x = a.split('.').map(Number);
  const y = b.split('.').map(Number);
  for (let i = 0; i < 3; i += 1) if (x[i] !== y[i]) return x[i] < y[i] ? -1 : 1;
  return 0;
};
const problems = [];
const pkg = JSON.parse(fs.readFileSync(path.join(root, 'boing-sdk/package.json'), 'utf8'));
if (sdk !== pkg.version) problems.push(`recommended_sdk ${sdk} != boing-sdk/package.json ${pkg.version}`);

const getJson = async (url) => {
  const r = await fetch(url, { headers: { 'User-Agent': 'boing-check-client-hints' } });
  if (!r.ok) throw new Error(`${url}: HTTP ${r.status}`);
  return r.json();
};
try {
  const reg = await getJson('https://registry.npmjs.org/@boing-network%2Fboing-sdk');
  if (!reg.versions?.[sdk]) problems.push(`recommended_sdk ${sdk} is not published on npm (latest: ${reg['dist-tags']?.latest})`);
  else console.log(`ok  npm has @boing-network/boing-sdk@${sdk} (latest ${reg['dist-tags']?.latest})`);
} catch (e) {
  problems.push(`npm lookup failed: ${e.message}`);
}
try {
  const url = process.env.BOING_EXPRESS_MANIFEST_URL || 'https://raw.githubusercontent.com/Boing-Network/boing.express/main/extension/manifest.json';
  const { version } = await getJson(url);
  if (cmp(rec, version) > 0) problems.push(`recommended_wallet_version ${rec} is newer than boing.express main (${version})`);
  if (cmp(min, rec) > 0) problems.push(`min_wallet_version ${min} > recommended_wallet_version ${rec}`);
  else console.log(`ok  wallet hints min ${min} / recommended ${rec} <= boing.express main ${version}`);
} catch (e) {
  problems.push(`boing.express manifest lookup failed: ${e.message}`);
}
if (problems.length) {
  for (const p of problems) console.error(`FAIL ${p}`);
  process.exit(1);
}