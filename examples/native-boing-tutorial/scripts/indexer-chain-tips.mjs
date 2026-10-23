#!/usr/bin/env node
/**
 * Print `boing_getSyncState` via SDK — head vs finalized vs durable index bound.
 *
 * Env: BOING_RPC_URL (default http://127.0.0.1:8545)
 */
import { createClient, clampIndexerHeightRange, getIndexerChainTips } from '@boing-network/boing-sdk';

const rpc = process.env.BOING_RPC_URL ?? 'http://127.0.0.1:8545';
const fromStr = process.env.BOING_FROM_HEIGHT;
const toStr = process.env.BOING_TO_HEIGHT;

async function main() {
  const client = createClient(rpc);
  const tips = await getIndexerChainTips(client);
  const out = { ok: true, rpc, tips };

  if (fromStr != null && toStr != null) {
    const fromHeight = Number(fromStr);
    const toHeight = Number(toStr);
    if (!Number.isInteger(fromHeight) || !Number.isInteger(toHeight)) {
      throw new Error('BOING_FROM_HEIGHT and BOING_TO_HEIGHT must be integers when set.');
    }
    Object.assign(out, {
      clampedRange: clampIndexerHeightRange(fromHeight, toHeight, tips.durableIndexThrough),
    });
  }

  console.log(JSON.stringify(out, null, 2));
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
