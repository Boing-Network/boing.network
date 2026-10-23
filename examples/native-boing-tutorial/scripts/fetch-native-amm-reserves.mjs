#!/usr/bin/env node
/**
 * Read MVP native CP pool state via `boing_getContractStorage`: reserves, total LP, optional signer LP (one batched round-trip).
 *
 * Env:
 *   BOING_RPC_URL    — default http://127.0.0.1:8545
 *   BOING_POOL_HEX   — required, 32-byte pool account id
 *   BOING_SIGNER_HEX — optional 32-byte account id; when set, includes that signer's LP balance in the snapshot
 */
import { createClient, fetchNativeConstantProductPoolSnapshot } from '@boing-network/boing-sdk';

const rpc = process.env.BOING_RPC_URL ?? 'http://127.0.0.1:8545';
const pool = process.env.BOING_POOL_HEX;
const signer = process.env.BOING_SIGNER_HEX?.trim() || undefined;

if (!pool) {
  console.error('Set BOING_POOL_HEX (0x + 64 hex chars).');
  process.exit(1);
}

async function main() {
  const client = createClient(rpc);
  const snap = await fetchNativeConstantProductPoolSnapshot(client, pool, {
    signerHex32: signer,
  });
  console.log(
    JSON.stringify(
      {
        ok: true,
        rpc,
        pool: pool.toLowerCase(),
        reserveA: snap.reserveA.toString(),
        reserveB: snap.reserveB.toString(),
        totalLpSupply: snap.totalLpSupply.toString(),
        ...(signer
          ? {
              signer: signer.toLowerCase(),
              signerLpBalance:
                snap.signerLpBalance != null ? snap.signerLpBalance.toString() : null,
            }
          : {}),
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
