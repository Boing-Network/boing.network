#!/usr/bin/env node
/**
 * Print JSON for native AMM **LP vault** `contract_call` (Boing Express / dApp shape). Read-only.
 *
 * Env:
 *   BOING_SENDER_HEX              — signer account (0x + 64 hex)
 *   BOING_VAULT_HEX               — vault contract
 *   BOING_LP_VAULT_ACTION         — configure | deposit (default configure)
 *
 * configure:
 *   BOING_POOL_HEX, BOING_SHARE_HEX — CP pool + LP share token accounts
 *
 * deposit:
 *   BOING_POOL_HEX, BOING_SHARE_HEX — same (for access list)
 *   BOING_AMOUNT_A, BOING_AMOUNT_B  — inner add_liquidity amounts
 *   BOING_MIN_LIQUIDITY             — inner add_liquidity min (default 0)
 *   BOING_VAULT_MIN_LP             — vault outer min_lp (default 0)
 *
 * See [NATIVE-AMM-LP-VAULT.md](../../docs/NATIVE-AMM-LP-VAULT.md) and [NATIVE-LP-SHARE-TOKEN.md](../../docs/NATIVE-LP-SHARE-TOKEN.md).
 */
import {
  buildNativeAmmLpVaultConfigureContractCallTx,
  buildNativeAmmLpVaultDepositAddContractCallTx,
  encodeNativeAmmAddLiquidityCalldata,
  encodeNativeAmmLpVaultConfigureCalldataHex,
  encodeNativeAmmLpVaultDepositAddCalldataHex,
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

const sender = requireEnv('BOING_SENDER_HEX');
const vault = requireEnv('BOING_VAULT_HEX');
const pool = requireEnv('BOING_POOL_HEX');
const share = requireEnv('BOING_SHARE_HEX');
const action = (process.env.BOING_LP_VAULT_ACTION || 'configure').toLowerCase().replace(/-/g, '_');

let tx;
if (action === 'configure') {
  const calldataHex = encodeNativeAmmLpVaultConfigureCalldataHex(pool, share);
  tx = buildNativeAmmLpVaultConfigureContractCallTx(sender, vault, calldataHex);
} else if (action === 'deposit' || action === 'deposit_add') {
  const amountA = parseBigintEnv('BOING_AMOUNT_A');
  const amountB = parseBigintEnv('BOING_AMOUNT_B');
  const minLiq = parseBigintEnv('BOING_MIN_LIQUIDITY', 0n);
  const vaultMinLp = parseBigintEnv('BOING_VAULT_MIN_LP', 0n);
  const inner = encodeNativeAmmAddLiquidityCalldata(amountA, amountB, minLiq);
  const calldataHex = encodeNativeAmmLpVaultDepositAddCalldataHex(inner, vaultMinLp);
  tx = buildNativeAmmLpVaultDepositAddContractCallTx(sender, vault, pool, share, calldataHex);
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

console.log(JSON.stringify({ ok: true, action, tx }, null, 2));
