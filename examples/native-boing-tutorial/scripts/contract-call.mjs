#!/usr/bin/env node
import {
  createClient,
  encodeReferenceTransferCalldata,
  explainBoingRpcError,
  hexToBytes,
  senderHexFromSecretKey,
  submitContractCallWithSimulationRetry,
} from 'boing-sdk';

const rpc = process.env.BOING_RPC_URL ?? 'http://127.0.0.1:8545';
const secretHex = process.env.BOING_SECRET_HEX;
const contractHex = process.env.BOING_CONTRACT_HEX;
const transferTo = process.env.BOING_TRANSFER_TO_HEX;

if (!secretHex || !contractHex || !transferTo) {
  console.error(
    'Need BOING_SECRET_HEX, BOING_CONTRACT_HEX, BOING_TRANSFER_TO_HEX (all 32-byte accounts where applicable).',
  );
  process.exit(1);
}

const secret = hexToBytes(secretHex);
const amount = BigInt(process.env.BOING_TRANSFER_AMOUNT ?? '1');
const calldata = encodeReferenceTransferCalldata(transferTo, amount);

async function main() {
  const client = createClient(rpc);
  const senderHex = await senderHexFromSecretKey(secret);
  const out = await submitContractCallWithSimulationRetry({
    client,
    secretKey32: secret,
    senderHex,
    contractHex,
    calldata,
  });
  console.log(JSON.stringify({ ok: true, ...out }, null, 2));
}

main().catch((e) => {
  console.error(explainBoingRpcError(e));
  process.exit(1);
});
