/**
 * Custody router v1 (`boing pool router-deploy | route`): pulls the swap input from the signer with
 * reference-token v2 `transfer_from`, then chains 1..=4 v8/v9 `swap_to` hops. Intermediate hops must be
 * **v9** pools (their swap returns `amount_out`); the last hop may be v8 or v9. Mirrors
 * `crates/boing-cli/src/router.rs` byte-for-byte (see `tests/boingRouter.test.ts`).
 */
import type { BoingClient } from './client.js';
import type { TransactionInput } from './bincode.js';
import { transactionIdFromSignedTransactionHex } from './bincode.js';
import { NATIVE_DEX_CUSTODY_ROUTER_BYTECODE_HEX } from './defaultNativeCpPoolBytecodeHex.js';
import { hexToAccountId, hexToBytes, validateHex32 } from './hex.js';
import { calldataAccountIdWord, calldataSelectorLastByte, calldataU128BeWord, concatCalldata } from './calldata.js';
import { buildBoingTokenApproveTransaction } from './boingToken.js';
import {
  BOING_POOL_PURPOSE,
  boingPoolEffectiveFeeBps,
  boingPoolMinOutWithSlippage,
  buildBoingPoolCallTransaction,
  getBoingPoolState,
  quoteBoingPoolAmountOut,
  type BoingPoolSignedTx,
  type BoingPoolSendOptions,
} from './boingPool.js';
import { fetchNextNonce, senderHexFromSecretKey, signTransactionInput, type Ed25519SecretKey32 } from './transactionBuilder.js';

export const BOING_CUSTODY_ROUTER_SELECTOR_SWAP = 0xf1;
export const BOING_CUSTODY_ROUTER_MAX_HOPS = 4;
export const BOING_CUSTODY_ROUTER_BYTECODE_HEX = NATIVE_DEX_CUSTODY_ROUTER_BYTECODE_HEX;

/** `direction`: `0` = A->B, `1` = B->A. `tokenOutHex` is only used for the access list / path check. */
export interface BoingRouteHop {
  poolHex: string;
  direction: 0 | 1;
  tokenInHex: string;
  tokenOutHex: string;
}

export function encodeBoingCustodyRouterSwapCalldata(
  hops: readonly Pick<BoingRouteHop, 'poolHex' | 'direction' | 'tokenInHex'>[],
  amountIn: bigint,
  minOut: bigint,
  recipientHex: string,
  deadline = 0n,
): Uint8Array {
  if (hops.length < 1 || hops.length > BOING_CUSTODY_ROUTER_MAX_HOPS) throw new RangeError('route needs 1..=4 hops');
  if (deadline < 0n || deadline >= 1n << 64n) throw new RangeError('deadline must fit in u64');
  const parts = [
    calldataSelectorLastByte(BOING_CUSTODY_ROUTER_SELECTOR_SWAP),
    calldataU128BeWord(BigInt(hops.length)),
    calldataU128BeWord(amountIn),
    calldataU128BeWord(minOut),
    calldataAccountIdWord(recipientHex),
    calldataU128BeWord(deadline),
  ];
  for (const h of hops) {
    parts.push(calldataAccountIdWord(h.poolHex), calldataU128BeWord(BigInt(h.direction)), calldataAccountIdWord(h.tokenInHex));
  }
  return concatCalldata(parts);
}

/** Same check as the CLI: 1..=4 hops and `tokenOut[i] == tokenIn[i+1]`. */
export function checkBoingRoutePath(hops: readonly BoingRouteHop[]): void {
  if (hops.length < 1 || hops.length > BOING_CUSTODY_ROUTER_MAX_HOPS) throw new Error('route needs 1..=4 hops');
  for (let i = 0; i + 1 < hops.length; i++) {
    if (validateHex32(hops[i]!.tokenOutHex).toLowerCase() !== validateHex32(hops[i + 1]!.tokenInHex).toLowerCase()) {
      throw new Error(`path is not connected at hop ${i}`);
    }
  }
}

/** Resolve a hop from the pool's token pair. */
export function boingRouteHop(poolHex: string, direction: 0 | 1, tokenAHex: string, tokenBHex: string): BoingRouteHop {
  return direction === 0
    ? { poolHex, direction, tokenInHex: tokenAHex, tokenOutHex: tokenBHex }
    : { poolHex, direction, tokenInHex: tokenBHex, tokenOutHex: tokenAHex };
}

export function buildBoingCustodyRouterDeployTransaction(input: { nonce: bigint; senderHex: string }): TransactionInput {
  const sender = hexToAccountId(validateHex32(input.senderHex));
  return {
    nonce: input.nonce,
    sender,
    payload: {
      kind: 'contractDeployWithPurpose',
      bytecode: hexToBytes(NATIVE_DEX_CUSTODY_ROUTER_BYTECODE_HEX),
      purposeCategory: BOING_POOL_PURPOSE,
      descriptionHash: null,
      create2Salt: null,
    },
    accessList: { read: [sender], write: [sender] },
  };
}

/** Router call; access list = sender, router, every pool / token on the path, plus extras. */
export function buildBoingCustodyRouterSwapTransaction(input: {
  nonce: bigint;
  senderHex: string;
  routerHex: string;
  hops: readonly BoingRouteHop[];
  amountIn: bigint;
  minOut: bigint;
  recipientHex: string;
  deadline?: bigint;
  extraAccountsHex?: readonly string[];
}): TransactionInput {
  const calldata = encodeBoingCustodyRouterSwapCalldata(input.hops, input.amountIn, input.minOut, input.recipientHex, input.deadline ?? 0n);
  const extras = [...(input.extraAccountsHex ?? [])];
  for (const h of input.hops) extras.push(h.poolHex, h.tokenInHex, h.tokenOutHex);
  return buildBoingPoolCallTransaction({ nonce: input.nonce, senderHex: input.senderHex, poolHex: input.routerHex, calldata, extraAccountsHex: extras });
}

/** Chained live quote (v8/v9 fee-on-input math). */
export async function quoteBoingRoute(client: BoingClient, hops: readonly BoingRouteHop[], amountIn: bigint): Promise<bigint> {
  let amt = amountIn;
  for (const h of hops) {
    const s = await getBoingPoolState(client, h.poolHex);
    const [ri, ro] = h.direction === 0 ? [s.reserveA, s.reserveB] : [s.reserveB, s.reserveA];
    amt = quoteBoingPoolAmountOut(9, ri, ro, amt, boingPoolEffectiveFeeBps(9, s.storedFeeBps));
    if (amt === 0n) throw new Error(`quote is 0 at pool ${h.poolHex}`);
  }
  return amt;
}

async function sign(client: BoingClient | null, tx: TransactionInput, key: Ed25519SecretKey32, dry: boolean): Promise<BoingPoolSignedTx> {
  const signedHex = await signTransactionInput(tx, key);
  const txId = transactionIdFromSignedTransactionHex(signedHex);
  if (!dry) {
    if (!client) throw new Error('a BoingClient is required to submit');
    await client.submitTransaction(signedHex);
  }
  return { txId, signedHex, submitted: !dry };
}

/** `boing pool route`: approve the router for exactly `amountIn` (unless `noApprove`), then route. */
export async function routeBoingSwap(
  client: BoingClient | null,
  secretKey32: Ed25519SecretKey32,
  input: {
    routerHex: string;
    hops: readonly BoingRouteHop[];
    amountIn: bigint;
    minOut?: bigint;
    slippageBps?: bigint;
    toHex?: string;
    noApprove?: boolean;
    deadlineHeight?: bigint;
    extraAccountsHex?: readonly string[];
  } & BoingPoolSendOptions,
): Promise<BoingPoolSignedTx & { minOut: bigint; approvals: BoingPoolSignedTx[] }> {
  if (input.amountIn <= 0n) throw new Error('amountIn must be > 0');
  checkBoingRoutePath(input.hops);
  let minOut = input.minOut;
  if (minOut == null) {
    if (!client || (input.dryRun && input.nonce != null)) throw new Error('offline dry-run needs an explicit minOut');
    minOut = boingPoolMinOutWithSlippage(await quoteBoingRoute(client, input.hops, input.amountIn), input.slippageBps ?? 50n);
  }
  if (minOut <= 0n) throw new Error('minOut must be > 0');
  const senderHex = await senderHexFromSecretKey(secretKey32);
  let nonce = input.nonce;
  if (nonce == null) {
    if (!client) throw new Error('pass a nonce or a BoingClient to fetch it');
    nonce = await fetchNextNonce(client, senderHex);
  }
  const dry = !!input.dryRun;
  const approvals: BoingPoolSignedTx[] = [];
  if (!input.noApprove) {
    const tx = buildBoingTokenApproveTransaction({ nonce, senderHex, tokenHex: input.hops[0]!.tokenInHex, toHex: input.routerHex, amount: input.amountIn });
    approvals.push(await sign(client, tx, secretKey32, dry));
    nonce += 1n;
  }
  const tx = buildBoingCustodyRouterSwapTransaction({
    nonce,
    senderHex,
    routerHex: input.routerHex,
    hops: input.hops,
    amountIn: input.amountIn,
    minOut,
    recipientHex: input.toHex ?? senderHex,
    deadline: input.deadlineHeight ?? 0n,
    extraAccountsHex: input.extraAccountsHex,
  });
  return { ...(await sign(client, tx, secretKey32, dry)), minOut, approvals };
}
