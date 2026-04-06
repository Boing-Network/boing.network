#!/usr/bin/env node
/**
 * Submit native AMM **LP vault** `contract_call` (`configure` / `deposit_add`) with simulate → merge access list → submit.
 *
 * Env (same shape as native-amm-lp-vault-print-contract-call-tx.mjs, plus secret):
 *   BOING_RPC_URL, BOING_SECRET_HEX — required
 *   BOING_VAULT_HEX, BOING_POOL_HEX, BOING_SHARE_HEX — required
 *   BOING_LP_VAULT_ACTION — configure | deposit (default configure)
 *   (per-action vars identical to print script)
 */
import {
  buildNativeAmmLpVaultConfigureAccessList,
  buildNativeAmmLpVaultDepositAddAccessList,
  createClient,
  encodeNativeAmmAddLiquidityCalldata,
  encodeNativeAmmLpVaultConfigureCalldataHex,
  encodeNativeAmmLpVaultDepositAddCalldataHex,
  explainBoingRpcError,
  hexToBytes,
  senderHexFromSecretKey,
  submitContractCallWithSimulationRetry,
} from 'boing-sdk';

function requireEnv(name) {
  const v = process.env[name];
  if (v == null || !String(v).trim()) {
    console.error(JSON.stringify({ ok: false, error: `missing_env:${name}` }, null, 2));
    process.exit(1);
  }
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

const rpc = process.env.BOING_RPC_URL ?? 'http://127.0.0.1:8545';
const secretHex = requireEnv('BOING_SECRET_HEX');
const vault = requireEnv('BOING_VAULT_HEX');
const pool = requireEnv('BOING_POOL_HEX');
const share = requireEnv('BOING_SHARE_HEX');
const action = (process.env.BOING_LP_VAULT_ACTION || 'configure').toLowerCase().replace(/-/g, '_');

let calldataHex;
let accessList;
if (action === 'configure') {
  calldataHex = encodeNativeAmmLpVaultConfigureCalldataHex(pool, share);
} else if (action === 'deposit' || action === 'deposit_add') {
  const amountA = parseBigintEnv('BOING_AMOUNT_A');
  const amountB = parseBigintEnv('BOING_AMOUNT_B');
  const minLiq = parseBigintEnv('BOING_MIN_LIQUIDITY', 0n);
  const vaultMinLp = parseBigintEnv('BOING_VAULT_MIN_LP', 0n);
  const inner = encodeNativeAmmAddLiquidityCalldata(amountA, amountB, minLiq);
  calldataHex = encodeNativeAmmLpVaultDepositAddCalldataHex(inner, vaultMinLp);
} else {
  console.error(
    JSON.stringify(
      {
        ok: false,
        error: 'invalid_BOING_LP_VAULT_ACTION',
        hint: 'use configure | deposit (or deposit_add)',
      },
      null,
      2
    )
  );
  process.exit(1);
}

async function main() {
  const secret = hexToBytes(secretHex);
  const client = createClient(rpc);
  const senderHex = await senderHexFromSecretKey(secret);
  if (action === 'configure') {
    accessList = buildNativeAmmLpVaultConfigureAccessList(senderHex, vault);
  } else {
    accessList = buildNativeAmmLpVaultDepositAddAccessList(senderHex, vault, pool, share);
  }
  const calldata = hexToBytes(calldataHex);

  const out = await submitContractCallWithSimulationRetry({
    client,
    secretKey32: secret,
    senderHex,
    contractHex: vault,
    calldata,
    accessList,
  });

  console.log(JSON.stringify({ ok: true, action, senderHex, ...out }, null, 2));
}

main().catch((e) => {
  console.error(explainBoingRpcError(e));
  process.exit(1);
});
