#!/usr/bin/env node
/**
 * Regenerates secured fungible hex embeds from
 * `cargo run -p boing-execution --example dump_reference_token_artifacts`:
 * - `defaultReferenceFungibleSecuredTemplateBytecodeHex.ts` — line **4** (full `0xFD` deploy)
 * - `defaultReferenceFungibleSecuredRuntimeBytecodeHex.ts` — line **5** (runtime only, for SDK init)
 */
import { execFileSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const outDeploy = join(
  dirname(fileURLToPath(import.meta.url)),
  '..',
  'src',
  'defaultReferenceFungibleSecuredTemplateBytecodeHex.ts',
);
const outRuntime = join(
  dirname(fileURLToPath(import.meta.url)),
  '..',
  'src',
  'defaultReferenceFungibleSecuredRuntimeBytecodeHex.ts',
);

const raw = execFileSync(
  'cargo',
  ['run', '-q', '-p', 'boing-execution', '--example', 'dump_reference_token_artifacts'],
  { cwd: root, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] },
);
const lines = raw.split('\n').filter((l) => l.startsWith('0x'));
const hexDeploy = lines[3]?.trim();
const hexRuntime = lines[4]?.trim();
if (!hexDeploy?.startsWith('0x')) {
  throw new Error('expected fourth stdout line starting with 0x (secured fungible deploy)');
}
if (!hexRuntime?.startsWith('0x')) {
  throw new Error('expected fifth stdout line starting with 0x (secured fungible runtime)');
}

const tsDeploy =
  '/**\n' +
  ' * Pinned `reference_fungible_secured_pinned_default_deploy_bytecode_v2()` from `boing-execution`.\n' +
  ' * (v1 secured template retired — self-transfer mint bug; audit #61 item 24.)\n' +
  ' * Regenerate: `node boing-sdk/scripts/embed-reference-fungible-secured-template-hex.mjs`\n' +
  ' */\n' +
  `export const DEFAULT_REFERENCE_FUNGIBLE_SECURED_TEMPLATE_BYTECODE_HEX = \`${hexDeploy}\` as const;\n`;

const tsRuntime =
  '/**\n' +
  ' * Pinned `reference_fungible_secured_runtime_bytecode_v2()` from `boing-execution`.\n' +
  ' * Regenerate: `node boing-sdk/scripts/embed-reference-fungible-secured-template-hex.mjs`\n' +
  ' */\n' +
  `export const DEFAULT_REFERENCE_FUNGIBLE_SECURED_RUNTIME_BYTECODE_HEX = \`${hexRuntime}\` as const;\n`;

writeFileSync(outDeploy, tsDeploy, 'utf8');
writeFileSync(outRuntime, tsRuntime, 'utf8');
console.log('wrote', outDeploy);
console.log('wrote', outRuntime);
