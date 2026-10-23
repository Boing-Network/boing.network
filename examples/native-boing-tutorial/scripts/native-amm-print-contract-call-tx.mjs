#!/usr/bin/env node
/**
 * Print JSON for a native constant-product pool `contract_call` (Boing Express / dApp shape).
 * Read-only — no signing, no RPC. Use with Boing Express `boing_sendTransaction` or SDK submit flows.
 *
 * Env:
 *   BOING_SENDER_HEX     — 32-byte account (0x + 64 hex), matches tx signer
 *   BOING_POOL_HEX       — pool contract account
 *   BOING_NATIVE_AMM_ACTION — swap | add | remove (default swap)
 *
 * swap:
 *   BOING_SWAP_DIRECTION — 0 = A→B, 1 = B→A (default 0)
 *   BOING_AMOUNT_IN      — integer string → bigint
 *   BOING_MIN_OUT        — default 0
 *
 * add:
 *   BOING_AMOUNT_A, BOING_AMOUNT_B — required
 *   BOING_MIN_LIQUIDITY — default 0
 *
 * remove:
 *   BOING_LIQUIDITY_BURN, BOING_MIN_A, BOING_MIN_B — required
 *
 * Optional (future / hybrid pools that CALL token contracts):
 *   BOING_TOKEN_A_HEX, BOING_TOKEN_B_HEX — merged into access_list via additionalAccountsHex32
 */
import {
  buildNativeConstantProductContractCallTx,
  encodeNativeAmmAddLiquidityCalldataHex,
  encodeNativeAmmRemoveLiquidityCalldataHex,
  encodeNativeAmmSwapCalldataHex,
} from '@boing-network/boing-sdk';

function requireEnv(name) {
  const v = process.env[name];
  if (v == null || !String(v).trim()) {
    console.error(JSON.stringify({ ok: false, error: `missing_env:${name}` }, null, 2));
    process.exit(1);
  }
  return String(v).trim();
}

function optEnv(name) {
  const v = process.env[name];
  if (v == null || !String(v).trim()) return undefined;
  return String(v).trim();
}

function parseBigintEnv(name, defaultValue) {
  const raw = process.env[name];
  if (raw == null || !String(raw).trim()) {
    if (defaultValue === undefined) {
      console.error(JSON.stringify({ ok: false, error: `missing_env:${name}` }, null, 2));
      process.exit(1);
    }
    return defaultValue;
  }
  try {
    return BigInt(String(raw).trim());
  } catch {
    console.error(JSON.stringify({ ok: false, error: `invalid_bigint:${name}` }, null, 2));
    process.exit(1);
  }
}

const sender = requireEnv('BOING_SENDER_HEX');
const pool = requireEnv('BOING_POOL_HEX');
const action = (process.env.BOING_NATIVE_AMM_ACTION || 'swap').toLowerCase().replace(/-/g, '_');

let calldataHex;
if (action === 'swap') {
  const direction = parseBigintEnv('BOING_SWAP_DIRECTION', 0n);
  const amountIn = parseBigintEnv('BOING_AMOUNT_IN');
  const minOut = parseBigintEnv('BOING_MIN_OUT', 0n);
  calldataHex = encodeNativeAmmSwapCalldataHex(direction, amountIn, minOut);
} else if (action === 'add' || action === 'add_liquidity') {
  const amountA = parseBigintEnv('BOING_AMOUNT_A');
  const amountB = parseBigintEnv('BOING_AMOUNT_B');
  const minLiq = parseBigintEnv('BOING_MIN_LIQUIDITY', 0n);
  calldataHex = encodeNativeAmmAddLiquidityCalldataHex(amountA, amountB, minLiq);
} else if (action === 'remove' || action === 'remove_liquidity') {
  const burn = parseBigintEnv('BOING_LIQUIDITY_BURN');
  const minA = parseBigintEnv('BOING_MIN_A');
  const minB = parseBigintEnv('BOING_MIN_B');
  calldataHex = encodeNativeAmmRemoveLiquidityCalldataHex(burn, minA, minB);
} else {
  console.error(
    JSON.stringify(
      {
        ok: false,
        error: 'invalid_BOING_NATIVE_AMM_ACTION',
        hint: 'use swap | add | remove (or add_liquidity | remove_liquidity)',
      },
      null,
      2
    )
  );
  process.exit(1);
}

const extras = [];
const ta = optEnv('BOING_TOKEN_A_HEX');
const tb = optEnv('BOING_TOKEN_B_HEX');
if (ta) extras.push(ta);
if (tb) extras.push(tb);

const tx = buildNativeConstantProductContractCallTx(
  sender,
  pool,
  calldataHex,
  extras.length ? { additionalAccountsHex32: extras } : undefined
);

console.log(JSON.stringify({ ok: true, action, tx }, null, 2));
