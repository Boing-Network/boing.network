#!/usr/bin/env node
/**
 * Submit native **LP share token** `contract_call` with simulate → merge access list → submit.
 *
 * Env (same as native-lp-share-print-contract-call-tx.mjs, plus):
 *   BOING_RPC_URL, BOING_SECRET_HEX — required
 */
import {
  buildLpShareTokenAccessList,
  createClient,
  encodeLpShareMintCalldataHex,
  encodeLpShareSetMinterOnceCalldataHex,
  encodeLpShareTransferCalldataHex,
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

async function main() {
  const secret = hexToBytes(secretHex);
  const client = createClient(rpc);
  const senderHex = await senderHexFromSecretKey(secret);
  const accessList = buildLpShareTokenAccessList(senderHex, share);
  const calldata = hexToBytes(calldataHex);

  const out = await submitContractCallWithSimulationRetry({
    client,
    secretKey32: secret,
    senderHex,
    contractHex: share,
    calldata,
    accessList,
  });

  console.log(JSON.stringify({ ok: true, action, senderHex, ...out }, null, 2));
}

main().catch((e) => {
  console.error(explainBoingRpcError(e));
  process.exit(1);
});
