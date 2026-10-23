#!/usr/bin/env node
/**
 * Submit native CP pool `contract_call` (swap / add_liquidity / remove_liquidity) with simulate → merge access list → submit.
 *
 * Env (same shape as native-amm-print-contract-call-tx.mjs, plus secret):
 *   BOING_RPC_URL, BOING_SECRET_HEX — required
 *   BOING_POOL_HEX — required
 *   BOING_NATIVE_AMM_ACTION — swap | add | remove (default swap)
 *   (per-action vars identical to print script)
 *   BOING_TOKEN_A_HEX, BOING_TOKEN_B_HEX — optional, for v2 access list widening
 */
import {
  buildNativeConstantProductPoolAccessList,
  createClient,
  encodeNativeAmmAddLiquidityCalldataHex,
  encodeNativeAmmRemoveLiquidityCalldataHex,
  encodeNativeAmmSwapCalldataHex,
  explainBoingRpcError,
  hexToBytes,
  senderHexFromSecretKey,
  submitContractCallWithSimulationRetry,
  validateHex32,
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

/**
 * @param {string} envName
 * @param {string} label
 */
function requireAccountIdHex(envName, label) {
  const raw = requireEnv(envName);
  try {
    return validateHex32(raw);
  } catch (e) {
    const msg = String(e?.message ?? e);
    const inner = raw.replace(/^0x/i, '');
    const odd = inner.length % 2 === 1;
    console.error(
      JSON.stringify(
        {
          ok: false,
          error: `invalid_${envName}`,
          message: msg,
          hint: odd
            ? `${label}: hex has an odd number of digits (common: truncated key, typo, or Unicode "…" instead of real hex). Use 0x + exactly 64 characters 0-9 a-f.`
            : `${label}: must be 0x + exactly 64 hex characters (32 bytes).`,
        },
        null,
        2
      )
    );
    process.exit(1);
  }
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

const rpc = process.env.BOING_RPC_URL ?? 'http://127.0.0.1:8545';
const secretHex = requireAccountIdHex('BOING_SECRET_HEX', 'BOING_SECRET_HEX (Ed25519 seed)');
const pool = requireAccountIdHex('BOING_POOL_HEX', 'BOING_POOL_HEX');
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
      2,
    ),
  );
  process.exit(1);
}

const extras = [];
const ta = optEnv('BOING_TOKEN_A_HEX');
const tb = optEnv('BOING_TOKEN_B_HEX');
if (ta) extras.push(ta);
if (tb) extras.push(tb);

const poolOpts = extras.length ? { additionalAccountsHex32: extras } : undefined;

async function main() {
  const secret = hexToBytes(secretHex);
  const client = createClient(rpc);
  const senderHex = await senderHexFromSecretKey(secret);
  const accessList = buildNativeConstantProductPoolAccessList(senderHex, pool, poolOpts);
  const calldata = hexToBytes(calldataHex);

  const out = await submitContractCallWithSimulationRetry({
    client,
    secretKey32: secret,
    senderHex,
    contractHex: pool,
    calldata,
    accessList,
  });

  console.log(JSON.stringify({ ok: true, action, senderHex, ...out }, null, 2));
}

main().catch((e) => {
  console.error(explainBoingRpcError(e));
  process.exit(1);
});
