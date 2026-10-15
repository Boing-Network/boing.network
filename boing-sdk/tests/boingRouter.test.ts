import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import {
  BOING_CUSTODY_ROUTER_BYTECODE_HEX,
  boingRouteHop,
  buildBoingCustodyRouterDeployTransaction,
  checkBoingRoutePath,
  routeBoingSwap,
} from '../src/boingRouter.js';
import { boingPoolBytecodeHex, buildBoingPoolDeployTransaction } from '../src/boingPool.js';
import { signTransactionInput } from '../src/transactionBuilder.js';
import { hexToBytes } from '../src/hex.js';

const V = JSON.parse(readFileSync(new URL('./fixtures/pool-cli-vectors.json', import.meta.url), 'utf8'));
const SEED = hexToBytes(V.secretHex);
const R = V.router;

describe('custody router SDK: byte-identical to the Rust CLI', () => {
  it('v9 pool deploy and router deploy match', async () => {
    expect(boingPoolBytecodeHex(9)).not.toBe(boingPoolBytecodeHex(8));
    expect(BOING_CUSTODY_ROUTER_BYTECODE_HEX.startsWith('0x')).toBe(true);
    const p = await signTransactionInput(buildBoingPoolDeployTransaction({ nonce: 0n, senderHex: V.senderHex, version: 9 }), SEED);
    expect(p).toBe(R.deployV9Pool.signedHex);
    const d = await signTransactionInput(buildBoingCustodyRouterDeployTransaction({ nonce: 1n, senderHex: V.senderHex }), SEED);
    expect(d).toBe(R.deploy.signedHex);
  });

  it('approve + 2-hop route match', async () => {
    const hops = [
      { poolHex: V.tokenAHex, direction: 0 as const, tokenInHex: V.tokenBHex, tokenOutHex: V.senderHex },
      { poolHex: V.tokenBHex, direction: 1 as const, tokenInHex: V.senderHex, tokenOutHex: V.recipientHex },
    ];
    // The CLI vector approves token A to `poolHex`; reproduce via a router at poolHex with token_in = token A.
    const r = await routeBoingSwap(null, SEED, {
      routerHex: V.poolHex,
      hops: [{ ...hops[0]!, tokenInHex: V.tokenAHex }],
      amountIn: 5_000n,
      minOut: 1n,
      nonce: 2n,
      dryRun: true,
    });
    expect(r.approvals[0]!.signedHex).toBe(R.approveRouter.signedHex);
    const s = await routeBoingSwap(null, SEED, {
      routerHex: V.poolHex,
      hops,
      amountIn: 5_000n,
      minOut: 4_000n,
      toHex: V.recipientHex,
      deadlineHeight: 321n,
      noApprove: true,
      nonce: 3n,
      dryRun: true,
    });
    expect(s.signedHex).toBe(R.swap2.signedHex);
    expect(s.txId).toBe(R.swap2.txId);
  });

  it('path checks and hop resolution', () => {
    const a = boingRouteHop(V.poolHex, 0, V.tokenAHex, V.tokenBHex);
    const b = boingRouteHop(V.poolHex, 1, V.recipientHex, V.tokenBHex);
    expect(a.tokenOutHex).toBe(V.tokenBHex);
    expect(b.tokenInHex).toBe(V.tokenBHex);
    expect(() => checkBoingRoutePath([a, b])).not.toThrow();
    expect(() => checkBoingRoutePath([b, a])).toThrow(/not connected/);
    expect(() => checkBoingRoutePath([])).toThrow();
  });
});
