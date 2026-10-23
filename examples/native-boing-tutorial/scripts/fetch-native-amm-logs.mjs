#!/usr/bin/env node
/**
 * Chunked `boing_getLogs` for one native CP pool address, then parse `Log2` rows with `tryParseNativeAmmRpcLogEntry`.
 *
 * Env:
 *   BOING_RPC_URL          — default http://127.0.0.1:8545
 *   BOING_POOL_HEX         — required, 32-byte pool account id
 *   BOING_FROM_BLOCK       — optional integer (with BOING_TO_BLOCK); if both omitted, uses chain tip ± lookback
 *   BOING_TO_BLOCK         — optional integer
 *   BOING_LOOKBACK_BLOCKS  — optional default 50 (used when from/to omitted): fromBlock = max(0, tip - lookback), toBlock = tip
 *   BOING_MAX_CONCURRENT   — optional default 1 for getLogsChunked
 */
import {
  createClient,
  filterMapNativeAmmRpcLogs,
  getLogsChunked,
} from '@boing-network/boing-sdk';

const rpc = process.env.BOING_RPC_URL ?? 'http://127.0.0.1:8545';
const pool = process.env.BOING_POOL_HEX;
const fromStr = process.env.BOING_FROM_BLOCK;
const toStr = process.env.BOING_TO_BLOCK;
const lookbackRaw = process.env.BOING_LOOKBACK_BLOCKS;
const concurrentRaw = process.env.BOING_MAX_CONCURRENT;

if (!pool) {
  console.error('Set BOING_POOL_HEX (0x + 64 hex chars).');
  process.exit(1);
}

function parsePositiveInt(name, s) {
  const n = Number(s);
  if (!Number.isInteger(n) || n < 0) {
    console.error(`${name} must be a non-negative integer.`);
    process.exit(1);
  }
  return n;
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
  let fromBlock;
  let toBlock;

  if (fromStr != null && toStr != null) {
    fromBlock = parsePositiveInt('BOING_FROM_BLOCK', fromStr);
    toBlock = parsePositiveInt('BOING_TO_BLOCK', toStr);
    if (fromBlock > toBlock) {
      console.error('BOING_FROM_BLOCK must be <= BOING_TO_BLOCK.');
      process.exit(1);
    }
  } else if (fromStr == null && toStr == null) {
    let lookback = 50;
    if (lookbackRaw != null && lookbackRaw !== '') {
      lookback = parsePositiveInt('BOING_LOOKBACK_BLOCKS', lookbackRaw);
    }
    const tip = await client.chainHeight();
    toBlock = tip;
    fromBlock = Math.max(0, tip - lookback);
  } else {
    console.error('Set both BOING_FROM_BLOCK and BOING_TO_BLOCK, or neither (for tip ± BOING_LOOKBACK_BLOCKS).');
    process.exit(1);
  }

  const rawLogs = await getLogsChunked(
    client,
    { fromBlock, toBlock, address: pool },
    { maxConcurrent }
  );
  const parsed = filterMapNativeAmmRpcLogs(rawLogs);

  console.log(
    JSON.stringify(
      {
        ok: true,
        rpc,
        pool: pool.toLowerCase(),
        fromBlock,
        toBlock,
        maxConcurrent,
        rawLogCount: rawLogs.length,
        parsedCount: parsed.length,
        events: parsed.map((p) => ({
          block_height: p.block_height,
          tx_index: p.tx_index,
          tx_id: p.tx_id,
          log_index: p.log_index,
          address: p.address,
          kind: p.kind,
          callerHex: p.callerHex,
          ...(p.kind === 'swap'
            ? {
                direction: p.direction.toString(),
                amountIn: p.amountIn.toString(),
                amountOutAfterFee: p.amountOutAfterFee.toString(),
              }
            : p.kind === 'addLiquidity'
              ? {
                  amountA: p.amountA.toString(),
                  amountB: p.amountB.toString(),
                  lpMinted: p.lpMinted.toString(),
                }
              : {
                  liquidityBurned: p.liquidityBurned.toString(),
                  amountAOut: p.amountAOut.toString(),
                  amountBOut: p.amountBOut.toString(),
                }),
        })),
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
