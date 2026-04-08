#!/usr/bin/env node
/**
 * Regenerate native CP pool + native DEX factory bytecode hex files under `artifacts/`.
 *
 * Runs `cargo` from the monorepo root (three levels above this file). Requires Rust toolchain.
 *
 * Output:
 *   artifacts/pool-lines.hex                 — dump_native_amm_pool (v1–v5 lines)
 *   artifacts/pool-dump-meta.txt             — stderr meta
 *   artifacts/native-dex-factory.hex           — pair directory
 *   artifacts/native-dex-ledger-router-v1.hex … v3.hex — ledger routers
 *   artifacts/native-dex-swap2-router.hex    — multihop / swap2 router
 *   artifacts/native-amm-lp-vault.hex          — LP vault
 *   artifacts/native-lp-share-token.hex      — LP share token
 *
 * Env:
 *   BOING_CARGO — override cargo binary (default `cargo`)
 */
import { spawnSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const cargoBin = process.env.BOING_CARGO ?? 'cargo';
const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const tutorialRoot = path.resolve(scriptDir, '..');
const repoRoot = path.resolve(tutorialRoot, '..', '..');
const artifactsDir = path.join(tutorialRoot, 'artifacts');

function runCargoExample(example) {
  const r = spawnSync(
    cargoBin,
    ['run', '-q', '-p', 'boing-execution', '--example', example],
    {
      cwd: repoRoot,
      encoding: 'utf8',
      maxBuffer: 64 * 1024 * 1024,
    }
  );
  if (r.error) {
    console.error(String(r.error));
    process.exit(1);
  }
  if (r.status !== 0) {
    console.error(r.stderr || r.stdout || `cargo exited ${r.status}`);
    process.exit(r.status ?? 1);
  }
  return { stdout: r.stdout ?? '', stderr: r.stderr ?? '' };
}

mkdirSync(artifactsDir, { recursive: true });

const pool = runCargoExample('dump_native_amm_pool');
writeFileSync(path.join(artifactsDir, 'pool-lines.hex'), pool.stdout, 'utf8');
writeFileSync(path.join(artifactsDir, 'pool-dump-meta.txt'), pool.stderr, 'utf8');

const factory = runCargoExample('dump_native_dex_factory');
const factoryLine =
  factory.stdout
    .split(/\r?\n/)
    .map((l) => l.trim())
    .find((l) => l.startsWith('0x')) ?? factory.stdout.trim();
writeFileSync(path.join(artifactsDir, 'native-dex-factory.hex'), `${factoryLine}\n`, 'utf8');

function writeSingleLineExample(example, relativeName) {
  const r = runCargoExample(example);
  const line =
    r.stdout
      .split(/\r?\n/)
      .map((l) => l.trim())
      .find((l) => l.startsWith('0x')) ?? r.stdout.trim();
  writeFileSync(path.join(artifactsDir, relativeName), `${line}\n`, 'utf8');
}

for (const [example, file] of [
  ['dump_native_dex_ledger_router', 'native-dex-ledger-router-v1.hex'],
  ['dump_native_dex_ledger_router_v2', 'native-dex-ledger-router-v2.hex'],
  ['dump_native_dex_ledger_router_v3', 'native-dex-ledger-router-v3.hex'],
  ['dump_native_dex_swap2_router', 'native-dex-swap2-router.hex'],
  ['dump_native_amm_lp_vault', 'native-amm-lp-vault.hex'],
  ['dump_native_lp_share_token', 'native-lp-share-token.hex'],
]) {
  writeSingleLineExample(example, file);
}

/** @type {Record<string, string>} */
const wrote = {
  poolLines: path.join('artifacts', 'pool-lines.hex'),
  poolMeta: path.join('artifacts', 'pool-dump-meta.txt'),
  dexFactory: path.join('artifacts', 'native-dex-factory.hex'),
  nativeDexLedgerRouterV1: path.join('artifacts', 'native-dex-ledger-router-v1.hex'),
  nativeDexLedgerRouterV2: path.join('artifacts', 'native-dex-ledger-router-v2.hex'),
  nativeDexLedgerRouterV3: path.join('artifacts', 'native-dex-ledger-router-v3.hex'),
  nativeDexSwap2Router: path.join('artifacts', 'native-dex-swap2-router.hex'),
  nativeAmmLpVault: path.join('artifacts', 'native-amm-lp-vault.hex'),
  nativeLpShareToken: path.join('artifacts', 'native-lp-share-token.hex'),
};

console.log(
  JSON.stringify(
    {
      ok: true,
      repoRoot,
      wrote,
      hint: 'Pool/factory: BOING_NATIVE_AMM_BYTECODE_FILE=artifacts/pool-lines.hex, BOING_DEX_FACTORY_BYTECODE_FILE=artifacts/native-dex-factory.hex. Other contracts: npm run deploy-native-purpose-contract + npm run print-native-dex-deploy-salts.',
    },
    null,
    2
  )
);
