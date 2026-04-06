#!/usr/bin/env node
/**
 * Print JSON for native **LP share token** `contract_call` (Boing Express / dApp shape). Read-only.
 *
 * Env:
 *   BOING_SENDER_HEX           — signer account (0x + 64 hex)
 *   BOING_LP_SHARE_HEX         — share token contract
 *   BOING_LP_SHARE_ACTION      — transfer | mint | set_minter_once (default transfer)
 *
 * transfer: BOING_TRANSFER_TO_HEX, BOING_TRANSFER_AMOUNT (default 1)
 * mint:     BOING_MINT_TO_HEX, BOING_MINT_AMOUNT
 * set_minter_once: BOING_MINTER_HEX
 *
 * See [NATIVE-LP-SHARE-TOKEN.md](../../docs/NATIVE-LP-SHARE-TOKEN.md).
 */
import {
  buildLpShareTokenContractCallTx,
  encodeLpShareMintCalldataHex,
  encodeLpShareSetMinterOnceCalldataHex,
  encodeLpShareTransferCalldataHex,
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
const share = requireEnv('BOING_LP_SHARE_HEX');
const action = (process.env.BOING_LP_SHARE_ACTION || 'transfer').toLowerCase().replace(/-/g, '_');

let calldataHex;
if (action === 'transfer') {
  const to = requireEnv('BOING_TRANSFER_TO_HEX');
  const amount = parseBigintEnv('BOING_TRANSFER_AMOUNT', 1n);
  calldataHex = encodeLpShareTransferCalldataHex(to, amount);
} else if (action === 'mint') {
  const to = requireEnv('BOING_MINT_TO_HEX');
  const amount = parseBigintEnv('BOING_MINT_AMOUNT');
  calldataHex = encodeLpShareMintCalldataHex(to, amount);
} else if (action === 'set_minter_once' || action === 'set_minter') {
  const minter = requireEnv('BOING_MINTER_HEX');
  calldataHex = encodeLpShareSetMinterOnceCalldataHex(minter);
} else {
  console.error(
    JSON.stringify(
      {
        ok: false,
        error: 'invalid_BOING_LP_SHARE_ACTION',
        hint: 'use transfer | mint | set_minter_once',
      },
      null,
      2
    )
  );
  process.exit(1);
}

const tx = buildLpShareTokenContractCallTx(sender, share, calldataHex);
console.log(JSON.stringify({ ok: true, action, tx }, null, 2));
