#!/usr/bin/env node
/**
 * Apply **`print_native_create2_manifest`** JSON to repo constants (SDK, scripts, examples, website, docs).
 *
 *   node scripts/sync-canonical-testnet-from-manifest.mjs scripts/my-manifest.json
 *
 * **Previous** published values (for string replacement in docs) default to the legacy testnet pair.
 * For a **second** rotation, set:
 *   CANONICAL_SYNC_PREVIOUS_POOL=0x... CANONICAL_SYNC_PREVIOUS_DEPLOYER=0x... node scripts/sync-canonical-testnet-from-manifest.mjs ...
 *
 * Does **not** read or write `BOING_SECRET_HEX`.
 */
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');

const LEGACY_POOL =
  process.env.CANONICAL_SYNC_PREVIOUS_POOL ||
  '0xffaa1290614441902ba813bf3bd8bf057624e0bd4f16160a9d32cd65d3f4d0c2';
const LEGACY_DEPLOYER =
  process.env.CANONICAL_SYNC_PREVIOUS_DEPLOYER ||
  '0xc063512f42868f1278c59a1f61ec0944785c304dbc48dec7e4c41f70f666733f';

function normHex32(h) {
  const s = String(h).trim().toLowerCase().replace(/^0x/, '');
  if (!/^[0-9a-f]{64}$/.test(s)) throw new Error(`invalid 32-byte hex: ${h}`);
  return `0x${s}`;
}

function strip0x(h) {
  return normHex32(h).slice(2);
}

function rustDefaultDeployerFn(deployerHex) {
  const raw = strip0x(deployerHex);
  const bytes = Buffer.from(raw, 'hex');
  const lines = [];
  for (let i = 0; i < 32; i += 8) {
    const chunk = bytes.subarray(i, i + 8);
    lines.push(
      '        ' +
        Array.from(chunk, (b) => `0x${b.toString(16).padStart(2, '0')}`).join(', ') +
        ',',
    );
  }
  return `fn default_canonical_testnet_dex_deployer() -> AccountId {\n    AccountId([\n${lines.join('\n')}\n    ])\n}`;
}

function writeCanonicalTestnetTs(poolHex) {
  const p = join(root, 'boing-sdk/src/canonicalTestnet.ts');
  const body = `/**
 * Well-known **public Boing testnet** (chain id **6913**) identifiers.
 *
 * **Normative source of truth** for the canonical native CP pool address is
 * [RPC-API-SPEC.md](https://github.com/Boing-Network/boing.network/blob/main/docs/RPC-API-SPEC.md) § Native constant-product AMM
 * and [TESTNET.md](https://github.com/Boing-Network/boing.network/blob/main/docs/TESTNET.md) §5.3.
 * This constant is a **convenience mirror** for TypeScript apps and tutorials; it may lag a doc-only update — verify on docs if unsure.
 */

import { validateHex32 } from './hex.js';

/**
 * Canonical **v1** native constant-product pool \`AccountId\` on public Boing testnet (**6913**).
 * Rotations: [OPS-FRESH-TESTNET-BOOTSTRAP.md](../../docs/OPS-FRESH-TESTNET-BOOTSTRAP.md).
 */
export const CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX = validateHex32(
  '${normHex32(poolHex)}',
);
`;
  writeFileSync(p, body, 'utf8');
}

function writeCanonicalTestnetDexTs(m) {
  const p = join(root, 'boing-sdk/src/canonicalTestnetDex.ts');
  const lines = [
    '/**',
    ' * **Predicted CREATE2** addresses for native DEX aux contracts when deployed by the canonical pool deployer.',
    ' * Regenerate: `cargo run -p boing-execution --example print_native_create2_manifest -- <DEPLOYER_HEX>`.',
    ' * Mirror of `scripts/canonical-testnet-dex-predicted.json`.',
    ' */',
    '',
    "import { validateHex32 } from './hex.js';",
    '',
    `export const CANONICAL_BOING_TESTNET_NATIVE_DEX_DEPLOYER_HEX = validateHex32(\n  '${normHex32(m.deployer)}',\n);`,
    '',
    `export const CANONICAL_BOING_TESTNET_NATIVE_DEX_FACTORY_HEX = validateHex32(\n  '${normHex32(m.native_dex_factory)}',\n);`,
    '',
    `export const CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V1_HEX = validateHex32(\n  '${normHex32(m.native_dex_ledger_router_v1)}',\n);`,
    '',
    `export const CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V2_HEX = validateHex32(\n  '${normHex32(m.native_dex_ledger_router_v2)}',\n);`,
    '',
    `export const CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V3_HEX = validateHex32(\n  '${normHex32(m.native_dex_ledger_router_v3)}',\n);`,
    '',
    `export const CANONICAL_BOING_TESTNET_NATIVE_DEX_MULTIHOP_SWAP_ROUTER_HEX = validateHex32(\n  '${normHex32(m.native_dex_multihop_swap_router)}',\n);`,
    '',
    `export const CANONICAL_BOING_TESTNET_NATIVE_AMM_LP_VAULT_HEX = validateHex32(\n  '${normHex32(m.native_amm_lp_vault)}',\n);`,
    '',
    `export const CANONICAL_BOING_TESTNET_NATIVE_LP_SHARE_TOKEN_HEX = validateHex32(\n  '${normHex32(m.native_lp_share_token)}',\n);`,
    '',
  ];
  writeFileSync(p, lines.join('\n'), 'utf8');
}

function writeDexPredictedJson(m) {
  const out = {
    _comment:
      'Regenerate: cargo run -p boing-execution --example print_native_create2_manifest -- <DEPLOYER_HEX>',
    deployer: normHex32(m.deployer),
    native_cp_pool_v1: normHex32(m.native_cp_pool_v1),
    native_dex_factory: normHex32(m.native_dex_factory),
    native_dex_ledger_router_v1: normHex32(m.native_dex_ledger_router_v1),
    native_dex_ledger_router_v2: normHex32(m.native_dex_ledger_router_v2),
    native_dex_ledger_router_v3: normHex32(m.native_dex_ledger_router_v3),
    native_dex_multihop_swap_router: normHex32(m.native_dex_multihop_swap_router),
    native_amm_lp_vault: normHex32(m.native_amm_lp_vault),
    native_lp_share_token: normHex32(m.native_lp_share_token),
  };
  writeFileSync(
    join(root, 'scripts/canonical-testnet-dex-predicted.json'),
    JSON.stringify(out, null, 2) + '\n',
    'utf8',
  );
}

function patchVerifyDriftRs(m) {
  const p = join(root, 'crates/boing-execution/examples/verify_canonical_cp_pool_create2_drift.rs');
  let s = readFileSync(p, 'utf8');
  s = s.replace(
    /const OPS_DEPLOYER_HEX: &str = "[0-9a-f]{64}";/,
    `const OPS_DEPLOYER_HEX: &str = "${strip0x(m.deployer)}";`,
  );
  s = s.replace(
    /const PUBLISHED_POOL_HEX: &str = "[0-9a-f]{64}";/,
    `const PUBLISHED_POOL_HEX: &str = "${strip0x(m.native_cp_pool_v1)}";`,
  );
  writeFileSync(p, s, 'utf8');
}

function patchDexPrintExample(m) {
  const p = join(root, 'crates/boing-execution/examples/print_canonical_testnet_dex_create2_addresses.rs');
  let s = readFileSync(p, 'utf8');
  const re =
    /fn default_canonical_testnet_dex_deployer\(\) -> AccountId \{\s*AccountId\(\[[\s\S]*?\]\)\s*\}/m;
  if (!re.test(s)) throw new Error('Could not find default_canonical_testnet_dex_deployer in dex print example');
  s = s.replace(re, rustDefaultDeployerFn(m.deployer));
  writeFileSync(p, s, 'utf8');
}

function patchCheckPoolMjs(poolHex) {
  const p = join(root, 'scripts/check-canonical-native-amm-pool.mjs');
  let s = readFileSync(p, 'utf8');
  s = s.replace(
    /const DEFAULT_POOL =\s*'0x[0-9a-f]{64}'/i,
    `const DEFAULT_POOL =\n  '${normHex32(poolHex)}'`,
  );
  writeFileSync(p, s, 'utf8');
}

function patchAuditDexMjs(poolHex) {
  const p = join(root, 'scripts/audit-native-dex-testnet.mjs');
  let s = readFileSync(p, 'utf8');
  s = s.replace(/const CANONICAL_POOL =\s*'0x[0-9a-f]{64}'/i, `const CANONICAL_POOL =\n  '${normHex32(poolHex)}'`);
  writeFileSync(p, s, 'utf8');
}

function patchWebsiteTestnet(poolHex) {
  const p = join(root, 'website/src/config/testnet.ts');
  let s = readFileSync(p, 'utf8');
  s = s.replace(
    /export const CANONICAL_NATIVE_CP_POOL_ACCOUNT_ID_HEX =\s*'0x[0-9a-f]{64}'/i,
    `export const CANONICAL_NATIVE_CP_POOL_ACCOUNT_ID_HEX =\n  '${normHex32(poolHex)}' as const`,
  );
  writeFileSync(p, s, 'utf8');
}

function patchEnvExample(poolHex, factoryHex) {
  const p = join(root, 'tools/boing-node-public-testnet.env.example');
  let s = readFileSync(p, 'utf8');
  s = s.replace(
    /# BOING_CANONICAL_NATIVE_CP_POOL=0x[0-9a-f]{64}/i,
    `# BOING_CANONICAL_NATIVE_CP_POOL=${normHex32(poolHex)}`,
  );
  s = s.replace(
    /# BOING_CANONICAL_NATIVE_DEX_FACTORY=.*/,
    `# BOING_CANONICAL_NATIVE_DEX_FACTORY=${normHex32(factoryHex)}`,
  );
  writeFileSync(p, s, 'utf8');
}

function patchCanonicalTestnetTest(poolHex) {
  const p = join(root, 'boing-sdk/tests/canonicalTestnet.test.ts');
  let s = readFileSync(p, 'utf8');
  s = s.replace(
    /expect\(CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX\)\.toBe\(\s*\n\s*'0x[0-9a-f]{64}',/i,
    `expect(CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX).toBe(\n      '${normHex32(poolHex)}',`,
  );
  writeFileSync(p, s, 'utf8');
}

/** Replace previous published hex (any case) with new lowercase hex in text files. */
function replaceHexInFiles(relPaths, prevFull, nextFull) {
  const prevLower = normHex32(prevFull);
  const nextLower = normHex32(nextFull);
  const variants = new Set([
    prevLower,
    prevLower.toUpperCase(),
    prevLower.replace(/^0x/, ''),
    prevLower.replace(/^0x/, '').toUpperCase(),
  ]);
  for (const rel of relPaths) {
    const p = join(root, rel);
    let s = readFileSync(p, 'utf8');
    let changed = false;
    for (const v of variants) {
      if (s.includes(v)) {
        s = s.split(v).join(nextLower);
        changed = true;
      }
    }
    if (changed) writeFileSync(p, s, 'utf8');
  }
}

const DOC_PATHS = [
  'docs/RPC-API-SPEC.md',
  'docs/TESTNET.md',
  'docs/OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md',
  'docs/OPS-CANONICAL-TESTNET-NATIVE-DEX-AUX.md',
  'docs/TESTNET-RPC-INFRA.md',
  'docs/TESTNET-RPC-INFRA.md',
  'docs/VIBEMINER-INTEGRATION.md',
  'docs/NATIVE-AMM-INTEGRATION-CHECKLIST.md',
  'docs/NATIVE-AMM-INTEGRATION-CHECKLIST.md',
  'docs/THREE-CODEBASE-ALIGNMENT.md',
  'docs/README.md',
  'examples/native-boing-tutorial/README.md',
];

function main() {
  const manifestPath = process.argv[2];
  if (!manifestPath) {
    console.error('Usage: node scripts/sync-canonical-testnet-from-manifest.mjs <manifest.json>');
    process.exit(1);
  }
  const mp = resolve(process.cwd(), manifestPath);
  const raw = JSON.parse(readFileSync(mp, 'utf8'));
  const m = { ...raw };
  delete m._comment;

  const need = [
    'deployer',
    'native_cp_pool_v1',
    'native_dex_factory',
    'native_dex_ledger_router_v1',
    'native_dex_ledger_router_v2',
    'native_dex_ledger_router_v3',
    'native_dex_multihop_swap_router',
    'native_amm_lp_vault',
    'native_lp_share_token',
  ];
  for (const k of need) {
    if (!m[k]) throw new Error(`manifest missing "${k}"`);
    m[k] = normHex32(m[k]);
  }

  writeCanonicalTestnetTs(m.native_cp_pool_v1);
  writeCanonicalTestnetDexTs(m);
  writeDexPredictedJson(m);
  patchVerifyDriftRs(m);
  patchDexPrintExample(m);
  patchCheckPoolMjs(m.native_cp_pool_v1);
  patchAuditDexMjs(m.native_cp_pool_v1);
  patchWebsiteTestnet(m.native_cp_pool_v1);
  patchEnvExample(m.native_cp_pool_v1, m.native_dex_factory);
  patchCanonicalTestnetTest(m.native_cp_pool_v1);

  replaceHexInFiles(DOC_PATHS, LEGACY_POOL, m.native_cp_pool_v1);
  replaceHexInFiles(DOC_PATHS, LEGACY_DEPLOYER, m.deployer);

  console.log(
    JSON.stringify(
      {
        ok: true,
        updated: [
          'boing-sdk/src/canonicalTestnet.ts',
          'boing-sdk/src/canonicalTestnetDex.ts',
          'scripts/canonical-testnet-dex-predicted.json',
          'crates/boing-execution/examples/verify_canonical_cp_pool_create2_drift.rs',
          'crates/boing-execution/examples/print_canonical_testnet_dex_create2_addresses.rs',
          'scripts/check-canonical-native-amm-pool.mjs',
          'scripts/audit-native-dex-testnet.mjs',
          'website/src/config/testnet.ts',
          'tools/boing-node-public-testnet.env.example',
          'boing-sdk/tests/canonicalTestnet.test.ts',
          ...DOC_PATHS,
        ],
        pool: m.native_cp_pool_v1,
        deployer: m.deployer,
        next: [
          'cd boing-sdk && npm run build && npm test -- tests/canonicalTestnet.test.ts',
          'cargo run -p boing-execution --example verify_canonical_cp_pool_create2_drift   # should report create2_matches_published: true',
          'Deploy contracts on-chain, then npm run check-canonical-pool',
        ],
      },
      null,
      2,
    ),
  );
}

main();
