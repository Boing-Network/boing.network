#!/usr/bin/env node
/**
 * Regenerates `defaultReferenceNftCollectionTemplateBytecodeHex.ts` from
 * `cargo run -p boing-execution --example dump_reference_token_artifacts` (3rd 0x line).
 */
import { execFileSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const out = join(
  dirname(fileURLToPath(import.meta.url)),
  '..',
  'src',
  'defaultReferenceNftCollectionTemplateBytecodeHex.ts',
);

const raw = execFileSync(
  'cargo',
  ['run', '-q', '--locked', '-p', 'boing-execution', '--example', 'dump_reference_token_artifacts'],
  { cwd: root, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] },
);
const lines = raw.split('\n').filter((l) => l.startsWith('0x'));
const hex = lines[2]?.trim();
if (!hex?.startsWith('0x')) {
  throw new Error('expected third stdout line starting with 0x (NFT collection template)');
}

const ts =
  '/**\n' +
  ' * Pinned `reference_nft_collection_template_bytecode()` from `boing-execution`.\n' +
  ' * Regenerate: `node boing-sdk/scripts/embed-reference-nft-collection-template-hex.mjs`\n' +
  ' * Must stay aligned with `REFERENCE_NFT_COLLECTION_TEMPLATE_VERSION`.\n' +
  ' */\n' +
  `export const DEFAULT_REFERENCE_NFT_COLLECTION_TEMPLATE_BYTECODE_HEX = \`${hex}\` as const;\n`;

writeFileSync(out, ts, 'utf8');
console.log('wrote', out, `(${hex.length} chars)`);
