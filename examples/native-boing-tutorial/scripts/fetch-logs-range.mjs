#!/usr/bin/env node
/**
 * Example: chunked `boing_getLogs` over a height range (optional parallel chunks).
 *
 * Env:
 *   BOING_RPC_URL          — default http://127.0.0.1:8545
 *   BOING_FROM_BLOCK       — required (integer)
 *   BOING_TO_BLOCK         — required (integer)
 *   BOING_ADDRESS          — optional contract account id (32-byte hex)
 *   BOING_MAX_CONCURRENT   — optional default 1 (parallel getLogs chunks)
 */
import { createClient, getLogsChunked } from '@boing-network/boing-sdk';

const rpc = process.env.BOING_RPC_URL ?? 'http://127.0.0.1:8545';
const fromStr = process.env.BOING_FROM_BLOCK;
const toStr = process.env.BOING_TO_BLOCK;
const address = process.env.BOING_ADDRESS;
const concurrentRaw = process.env.BOING_MAX_CONCURRENT;

if (fromStr == null || toStr == null) {
  console.error('Set BOING_FROM_BLOCK and BOING_TO_BLOCK (integers).');
  process.exit(1);
}

const fromBlock = Number(fromStr);
const toBlock = Number(toStr);
if (!Number.isInteger(fromBlock) || !Number.isInteger(toBlock)) {
  console.error('BOING_FROM_BLOCK and BOING_TO_BLOCK must be integers.');
  process.exit(1);
}

let maxConcurrent = 1;
if (concurrentRaw != null && concurrentRaw !== '') {
  maxConcurrent = Number(concurrentRaw);
  if (!Number.isInteger(maxConcurrent) || maxConcurrent < 1) {
    console.error('BOING_MAX_CONCURRENT must be an integer >= 1.');
    process.exit(1);
  }
}

async function main() {
  const client = createClient(rpc);
  const filter = {
    fromBlock,
    toBlock,
    ...(address ? { address } : {}),
  };
  const logs = await getLogsChunked(client, filter, { maxConcurrent });
  console.log(
    JSON.stringify(
      {
        ok: true,
        rpc,
        fromBlock,
        toBlock,
        maxConcurrent,
        count: logs.length,
        sample: logs.slice(0, 5),
      },
      null,
      2
    )
  );
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
