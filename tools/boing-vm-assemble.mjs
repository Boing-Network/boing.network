#!/usr/bin/env node
/**
 * Minimal Boing VM assembler: line-oriented mnemonics → contiguous bytecode hex.
 * Opcodes match crates/boing-execution/src/bytecode.rs and docs/TECHNICAL-SPECIFICATION.md §7 (Boing VM only).
 *
 * Usage:
 *   node tools/boing-vm-assemble.mjs program.asm
 *   node tools/boing-vm-assemble.mjs --map=program.boing.map.json program.asm
 *   node tools/boing-vm-assemble.mjs < program.asm
 *
 * Lines: `# comment`, empty, or: MNEMONIC [operands...]
 *
 * PUSH1 <byte>     — decimal 0-255 or 0x..
 * PUSH2 … PUSH31   — next arg: hex string (2×N hex digits, optional 0x)
 * PUSH32 <hex64>   — 32-byte word (64 hex digits)
 * PUSH <n> <hex>   — generic PUSHn (1≤n≤32), hex has 2n digits
 *
 * ## Source map (T2 / debug hook)
 *
 * With `--map=<path>`, writes a small JSON sidecar for line → bytecode offset (0-based),
 * consumable by simulators or tests. Format `version: 1`, `segments: [{ sourceLine, byteOffset, byteLength }]`.
 */

import fs from 'node:fs';
import path from 'node:path';

const SINGLE = new Map(
  Object.entries({
    STOP: 0x00,
    ADD: 0x01,
    SUB: 0x02,
    MUL: 0x03,
    DIV: 0x04,
    MOD: 0x06,
    ADDMOD: 0x08,
    MULMOD: 0x09,
    LT: 0x10,
    GT: 0x11,
    EQ: 0x14,
    ISZERO: 0x15,
    AND: 0x16,
    OR: 0x17,
    XOR: 0x18,
    NOT: 0x19,
    SHL: 0x1b,
    SHR: 0x1c,
    SAR: 0x1d,
    ADDRESS: 0x30,
    CALLER: 0x33,
    DUP1: 0x80,
    LOG0: 0xa0,
    LOG1: 0xa1,
    LOG2: 0xa2,
    LOG3: 0xa3,
    LOG4: 0xa4,
    MLOAD: 0x51,
    MSTORE: 0x52,
    SLOAD: 0x54,
    SSTORE: 0x55,
    JUMP: 0x56,
    JUMPI: 0x57,
    CALL: 0xf1,
    RETURN: 0xf3,
    CREATE2: 0xf5,
  }).map(([k, v]) => [k.toUpperCase(), v])
);

function parseHexNibs(s, nbytes) {
  const t = String(s).replace(/^0x/i, '').replace(/\s/g, '');
  if (t.length !== nbytes * 2 || !/^[0-9a-fA-F]+$/.test(t)) {
    throw new Error(`Expected ${nbytes} bytes (${nbytes * 2} hex chars), got "${s}"`);
  }
  const out = [];
  for (let i = 0; i < t.length; i += 2) {
    out.push(parseInt(t.slice(i, i + 2), 16));
  }
  return out;
}

function parsePush1Byte(tok) {
  if (/^0x[0-9a-fA-F]{1,2}$/i.test(tok)) {
    return parseInt(tok, 16);
  }
  const n = Number(tok);
  if (!Number.isInteger(n) || n < 0 || n > 255) {
    throw new Error(`PUSH1 operand must be 0-255 or 0x00-0xff, got ${tok}`);
  }
  return n;
}

function assembleLine(line, lineNo) {
  const trimmed = line.replace(/#.*$/, '').trim();
  if (!trimmed) return [];
  const parts = trimmed.split(/\s+/);
  const op = parts[0].toUpperCase();
  const out = [];

  if (op === 'PUSH1') {
    if (parts.length < 2) throw new Error(`line ${lineNo}: PUSH1 needs a byte`);
    out.push(0x60, parsePush1Byte(parts[1]));
    return out;
  }

  if (op === 'PUSH') {
    if (parts.length < 3) throw new Error(`line ${lineNo}: PUSH <n> <hex> needs size and hex`);
    const n = parseInt(parts[1], 10);
    if (n < 1 || n > 32) throw new Error(`line ${lineNo}: PUSH size must be 1..32`);
    const bytes = parseHexNibs(parts[2], n);
    out.push(0x5f + n, ...bytes);
    return out;
  }

  const mPush = op.match(/^PUSH(\d+)$/);
  if (mPush) {
    const n = parseInt(mPush[1], 10);
    if (n === 1) throw new Error(`line ${lineNo}: use PUSH1 <byte> for single-byte immediate`);
    if (n < 2 || n > 32) throw new Error(`line ${lineNo}: PUSHn supports n=2..32 (or PUSH1)`);
    if (parts.length < 2) throw new Error(`line ${lineNo}: PUSH${n} needs hex operand`);
    const bytes = parseHexNibs(parts[1], n);
    out.push(0x5f + n, ...bytes);
    return out;
  }

  const code = SINGLE.get(op);
  if (code === undefined) {
    throw new Error(`line ${lineNo}: unknown mnemonic "${parts[0]}"`);
  }
  out.push(code);
  return out;
}

/**
 * @param {string} text
 * @param {string} [sourceFile] logical name for map
 * @returns {{ bytes: number[], segments: { sourceLine: number, byteOffset: number, byteLength: number }[] }}
 */
export function assembleWithMap(text, sourceFile = '(stdin)') {
  const lines = text.split(/\r?\n/);
  const bytes = [];
  /** @type {{ sourceLine: number, byteOffset: number, byteLength: number }[]} */
  const segments = [];
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const trimmed = line.replace(/#.*$/, '').trim();
    if (!trimmed) continue;
    const start = bytes.length;
    const chunk = assembleLine(line, i + 1);
    if (chunk.length > 0) {
      for (const b of chunk) bytes.push(b);
      segments.push({
        sourceLine: i + 1,
        byteOffset: start,
        byteLength: chunk.length,
      });
    }
  }
  return {
    bytes,
    segments,
    sourceFile,
  };
}

function parseArgs(argv) {
  let mapOut = null;
  const rest = [];
  for (const a of argv) {
    if (a.startsWith('--map=')) {
      mapOut = a.slice('--map='.length);
    } else {
      rest.push(a);
    }
  }
  return { mapOut, path: rest[0] };
}

function main() {
  const { mapOut, path: asmPath } = parseArgs(process.argv.slice(2));
  const text = asmPath ? fs.readFileSync(asmPath, 'utf8') : fs.readFileSync(0, 'utf8');
  const logical = asmPath ? path.basename(asmPath) : '(stdin)';
  const { bytes, segments, sourceFile } = assembleWithMap(text, logical);

  if (mapOut) {
    const mapDoc = {
      version: 1,
      format: 'boing-vm-line-map',
      source: sourceFile,
      description:
        'Each segment maps one non-empty assembly source line to a contiguous bytecode range (byteOffset is 0-based in the output blob).',
      segments,
    };
    fs.writeFileSync(mapOut, JSON.stringify(mapDoc, null, 2), 'utf8');
  }

  const hex =
    '0x' +
    bytes
      .map((b) => b.toString(16).padStart(2, '0'))
      .join('');
  console.log(hex);
}

main();
