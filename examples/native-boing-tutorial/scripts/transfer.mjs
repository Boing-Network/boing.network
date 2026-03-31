#!/usr/bin/env node
import {
  createClient,
  explainBoingRpcError,
  hexToBytes,
  senderHexFromSecretKey,
  submitTransferWithSimulationRetry,
} from 'boing-sdk';

const rpc = process.env.BOING_RPC_URL ?? 'http://127.0.0.1:8545';
const secretHex = process.env.BOING_SECRET_HEX;
const toHex = process.env.BOING_TO_HEX;

if (!secretHex) {
  console.error('Missing BOING_SECRET_HEX (0x + 64 hex chars).');
  process.exit(1);
}
if (!toHex) {
  console.error('Missing BOING_TO_HEX (recipient 32-byte account).');
  process.exit(1);
}

const secret = hexToBytes(secretHex);
const amount = BigInt(process.env.BOING_AMOUNT ?? '1');

async function main() {
  const client = createClient(rpc);
  const senderHex = await senderHexFromSecretKey(secret);
  const out = await submitTransferWithSimulationRetry({
    client,
    secretKey32: secret,
    senderHex,
    toHex,
    amount,
  });
  console.log(JSON.stringify({ ok: true, ...out }, null, 2));
}

main().catch((e) => {
  console.error(explainBoingRpcError(e));
  process.exit(1);
});
