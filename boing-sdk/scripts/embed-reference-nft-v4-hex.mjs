#!/usr/bin/env node
/**
 * Regenerates `defaultReferenceNftV4BytecodeHex.ts` from
 * `cargo run -p boing-execution --example dump_reference_token_artifacts` (8th + 9th 0x lines).
 */
import { execFileSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..', '..');
const out = join(here, '..', 'src', 'defaultReferenceNftV4BytecodeHex.ts');

const raw = execFileSync(
  'cargo',
  ['run', '-q', '--locked', '-p', 'boing-execution', '--example', 'dump_reference_token_artifacts'],
  { cwd: root, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] },
);
const lines = raw.split(/\r?\n/).filter((l) => l.startsWith('0x'));
const deploy = lines[7]?.trim();
const runtime = lines[8]?.trim();
if (!deploy?.startsWith('0x') || !runtime?.startsWith('0x')) {
  throw new Error('expected 8th/9th stdout lines starting with 0x (NFT v4 deploy / runtime)');
}

const ts =
  '/**\n' +
  ' * Pinned reference NFT collection **v4** bytecode from `boing-execution`.\n' +
  ' * Regenerate: `node boing-sdk/scripts/embed-reference-nft-v4-hex.mjs`\n' +
  ' * Must stay aligned with `REFERENCE_NFT_V4_TEMPLATE_VERSION` and docs/artifacts/reference-nft-collection-v4-*.hex.\n' +
  ' */\n' +
  '/** Deploy payload (`0xFD` init: admin = deployer, returns the runtime). */\n' +
  `export const DEFAULT_REFERENCE_NFT_V4_DEPLOY_BYTECODE_HEX = \`${deploy}\` as const;\n` +
  '/** Installed runtime (what `boing_getContractCode` returns) — identity for indexers / explorers. */\n' +
  `export const DEFAULT_REFERENCE_NFT_V4_RUNTIME_BYTECODE_HEX = \`${runtime}\` as const;\n`;

writeFileSync(out, ts, 'utf8');
console.log('wrote', out, `(deploy ${deploy.length} chars, runtime ${runtime.length} chars)`);
