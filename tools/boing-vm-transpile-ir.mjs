#!/usr/bin/env node
/**
 * Boing mini-IR → VM bytecode (hex on stdout).
 * Spec: docs/BOING-MINI-IR.md — opcode bytes in sync with crates/boing-execution/src/bytecode.rs (Boing VM; see docs/BOING-VM-INDEPENDENCE.md).
 *
 * Usage:
 *   node tools/boing-vm-transpile-ir.mjs program.json
 *   node tools/boing-vm-transpile-ir.mjs < program.json
 *   node tools/boing-vm-transpile-ir.mjs --self-test
 */

import fs from 'node:fs';

/** @type {Record<string, number>} */
const ZERO_ARG = {
  stop: 0x00,
  add: 0x01,
  sub: 0x02,
  mul: 0x03,
  div: 0x04,
  mod: 0x06,
  addmod: 0x08,
  mulmod: 0x09,
  lt: 0x10,
  gt: 0x11,
  eq: 0x14,
  iszero: 0x15,
  and: 0x16,
  or: 0x17,
  xor: 0x18,
  not: 0x19,
  shl: 0x1b,
  shr: 0x1c,
  sar: 0x1d,
  address: 0x30,
  caller: 0x33,
  dup1: 0x80,
  log0: 0xa0,
  log1: 0xa1,
  log2: 0xa2,
  log3: 0xa3,
  log4: 0xa4,
  mload: 0x51,
  mstore: 0x52,
  sload: 0x54,
  sstore: 0x55,
  jump: 0x56,
  jumpi: 0x57,
  call: 0xf1,
  create2: 0xf5,
  return: 0xf3,
};

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

function u256BytesFromOffset(offset) {
  if (!Number.isInteger(offset) || offset < 0) {
    throw new Error(`jumpdest offset must be non-negative integer: ${offset}`);
  }
  if (offset > 0xffffffff) {
    throw new Error(`jumpdest offset too large for v1 (max 4GiB bytecode): ${offset}`);
  }
  const out = new Array(32).fill(0);
  let x = offset;
  for (let i = 31; i >= 0 && x > 0; i--) {
    out[i] = x & 0xff;
    x >>= 8;
  }
  return out;
}

/**
 * @param {unknown} doc
 * @returns {Uint8Array}
 */
export function transpileIr(doc) {
  if (!doc || typeof doc !== 'object') throw new Error('IR root must be an object');
  const version = doc.version;
  if (version !== 1) throw new Error(`Unsupported IR version: ${version} (expected 1)`);
  const ops = doc.ops;
  if (!Array.isArray(ops)) throw new Error('IR must contain ops: array');

  /** @type {Record<string, number>} */
  const labels = {};
  const out = [];
  /** @type {{ at: number, label: string }[]} */
  const patches = [];

  for (let i = 0; i < ops.length; i++) {
    const op = ops[i];
    if (!op || typeof op !== 'object' || Array.isArray(op)) {
      throw new Error(`ops[${i}] must be a non-array object`);
    }
    const keys = Object.keys(op);
    if (keys.length !== 1) {
      throw new Error(`ops[${i}] must have exactly one key, got: ${keys.join(', ')}`);
    }
    const k = keys[0];
    const v = op[k];

    if (k === 'label') {
      if (typeof v !== 'string' || !v) throw new Error(`ops[${i}]: label name must be non-empty string`);
      labels[v] = out.length;
      continue;
    }

    if (k === 'push_jumpdest') {
      if (typeof v !== 'string' || !v) throw new Error(`ops[${i}]: push_jumpdest needs label string`);
      out.push(0x7f);
      patches.push({ at: out.length, label: v });
      for (let j = 0; j < 32; j++) out.push(0);
      continue;
    }

    if (k === 'push1') {
      const n = v;
      if (!Number.isInteger(n) || n < 0 || n > 255) {
        throw new Error(`ops[${i}]: push1 expects integer 0..255`);
      }
      out.push(0x60, n);
      continue;
    }

    if (k === 'push32') {
      const bytes = parseHexNibs(v, 32);
      out.push(0x7f, ...bytes);
      continue;
    }

    if (k === 'push') {
      if (!v || typeof v !== 'object') throw new Error(`ops[${i}]: push needs { n, hex }`);
      const n = v.n;
      const hex = v.hex;
      if (!Number.isInteger(n) || n < 2 || n > 31) {
        throw new Error(`ops[${i}]: push.n must be 2..31`);
      }
      const bytes = parseHexNibs(hex, n);
      out.push(0x5f + n, ...bytes);
      continue;
    }

    const code = ZERO_ARG[k];
    if (code !== undefined) {
      if (v !== true && v !== undefined && Object.keys(v || {}).length !== 0) {
        throw new Error(`ops[${i}]: opcode "${k}" expects true or empty object`);
      }
      out.push(code);
      continue;
    }

    throw new Error(`ops[${i}]: unknown opcode "${k}"`);
  }

  for (const p of patches) {
    const dest = labels[p.label];
    if (dest === undefined) {
      throw new Error(`Undefined label "${p.label}" for push_jumpdest`);
    }
    const word = u256BytesFromOffset(dest);
    for (let i = 0; i < 32; i++) {
      out[p.at + i] = word[i];
    }
  }

  return Uint8Array.from(out);
}

function hexOf(bytes) {
  return (
    '0x' +
    [...bytes]
      .map((b) => b.toString(16).padStart(2, '0'))
      .join('')
  );
}

function selfTest() {
  const stopIr = { version: 1, ops: [{ stop: true }] };
  const h = hexOf(transpileIr(stopIr));
  if (h !== '0x00') throw new Error(`self-test stop: got ${h}`);

  const jumpIr = {
    version: 1,
    ops: [
      { label: 'start' },
      { push1: 0 },
      { push_jumpdest: 'start' },
      { jump: true },
    ],
  };
  const jb = transpileIr(jumpIr);
  if (jb.length !== 2 + 33 + 1) throw new Error(`jump program length ${jb.length}`);

  const callerIr = JSON.parse(
    fs.readFileSync(new URL('./examples/mini-ir-caller-return.json', import.meta.url), 'utf8')
  );
  const cb = transpileIr(callerIr);
  const expectedSmoke =
    '33807f01010101010101010101010101010101010101010101010101010101010101015560046000a03360205260206020f300';
  if (hexOf(cb).slice(2) !== expectedSmoke) {
    throw new Error(`mini-ir-caller-return mismatch:\n${hexOf(cb)}\nvs\n0x${expectedSmoke}`);
  }

  console.error('boing-vm-transpile-ir: self-test ok');
}

function main() {
  const argv = process.argv.slice(2);
  if (argv.includes('--self-test')) {
    selfTest();
    return;
  }

  const path = argv[0];
  const text = path ? fs.readFileSync(path, 'utf8') : fs.readFileSync(0, 'utf8');
  let doc;
  try {
    doc = JSON.parse(text);
  } catch (e) {
    throw new Error(`Invalid JSON: ${e.message}`);
  }
  const bytes = transpileIr(doc);
  console.log(hexOf(bytes));
}

main();
