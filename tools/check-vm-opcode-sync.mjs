#!/usr/bin/env node
/**
 * Fail if JS opcode tables drift from crates/boing-execution/src/bytecode.rs
 * (audit #61 item 53). Run: `node tools/check-vm-opcode-sync.mjs`
 */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const rust = fs.readFileSync(
  path.join(root, 'crates/boing-execution/src/bytecode.rs'),
  'utf8'
);

/** @type {Map<string, number>} */
const rustOps = new Map();
for (const m of rust.matchAll(/^\s+([A-Za-z][A-Za-z0-9]*)\s*=\s*0x([0-9a-fA-F]+)/gm)) {
  rustOps.set(m[1].toUpperCase(), parseInt(m[2], 16));
}

function loadJsOps(rel, pattern) {
  const src = fs.readFileSync(path.join(root, rel), 'utf8');
  /** @type {Map<string, number>} */
  const out = new Map();
  for (const m of src.matchAll(pattern)) {
    out.set(m[1].toUpperCase().replace(/_/g, ''), parseInt(m[2], 16));
  }
  return out;
}

// assemble: ADDRESS: 0x30
const assemble = loadJsOps(
  'tools/boing-vm-assemble.mjs',
  /^\s+([A-Z][A-Z0-9_]*):\s*0x([0-9a-fA-F]+)/gm
);
// transpile: address: 0x30
const transpile = loadJsOps(
  'tools/boing-vm-transpile-ir.mjs',
  /^\s+([a-z][a-z0-9_]*):\s*0x([0-9a-fA-F]+)/gm
);

function normalizeRustName(n) {
  // NativeTransfer -> NATIVETRANSFER; BlockHeight -> BLOCKHEIGHT
  return n.toUpperCase();
}

const required = [
  'BLOCKHEIGHT',
  'TIMESTAMP',
  'BALANCE',
  'CALLVALUE',
  'SELFBALANCE',
  'NATIVETRANSFER',
];

let failed = false;
for (const name of required) {
  const rustName = [...rustOps.keys()].find((k) => normalizeRustName(k) === name);
  if (!rustName) {
    console.error(`missing in Rust: ${name}`);
    failed = true;
    continue;
  }
  const byte = rustOps.get(rustName);
  const a = assemble.get(name);
  const t = transpile.get(name);
  if (a !== byte) {
    console.error(`assemble.mjs ${name}: got ${a} want 0x${byte.toString(16)}`);
    failed = true;
  }
  if (t !== byte) {
    console.error(`transpile-ir.mjs ${name}: got ${t} want 0x${byte.toString(16)}`);
    failed = true;
  }
}

if (failed) {
  process.exit(1);
}
console.log('ok: JS opcode tables match Rust for BLOCKHEIGHT/TIMESTAMP/native-value ops');
