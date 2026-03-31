#!/usr/bin/env node
/**
 * Minimal contract deploy with purpose (QA). Default bytecode: tiny return payload.
 * Purpose category default: tooling (see QUALITY-ASSURANCE-NETWORK.md).
 */
import {
  createClient,
  explainBoingRpcError,
  hexToBytes,
  senderHexFromSecretKey,
  submitDeployWithPurposeFlow,
} from 'boing-sdk';

const rpc = process.env.BOING_RPC_URL ?? 'http://127.0.0.1:8545';
const secretHex = process.env.BOING_SECRET_HEX;
const bytecodeHex =
  process.env.BOING_DEPLOY_BYTECODE_HEX ?? '0x600160005260206000f3';

if (!secretHex) {
  console.error('Missing BOING_SECRET_HEX.');
  process.exit(1);
}

const secret = hexToBytes(secretHex);
const bytecode = hexToBytes(bytecodeHex);
const purposeCategory = process.env.BOING_PURPOSE ?? 'tooling';

async function main() {
  const client = createClient(rpc);
  const senderHex = await senderHexFromSecretKey(secret);
  const out = await submitDeployWithPurposeFlow({
    client,
    secretKey32: secret,
    senderHex,
    bytecode,
    purposeCategory,
  });
  console.log(JSON.stringify({ ok: true, ...out }, null, 2));
}

main().catch((e) => {
  console.error(explainBoingRpcError(e));
  process.exit(1);
});
