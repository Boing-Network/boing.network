/**
 * Custody-pool (v8/v9) swap planning for apps that sign through a wallet (e.g. boing.finance + Boing Express):
 * discover pools from `boing_listDexPools`, tell custody pools apart from ledger-only v1..v7 pools, quote,
 * compute the allowance state and build the exact-amount `approve(router)` + custody-router swap transactions.
 * Signing / submitting is left to the caller (same calldata as `routeBoingSwap`).
 */
import type { BoingClient } from './client.js';
import type { TransactionInput } from './bincode.js';
import { accountIdToHex, bytesToHex, validateHex32 } from './hex.js';
import { buildBoingTokenApproveTransaction, getBoingTokenAllowance } from './boingToken.js';
import {
  boingPoolEffectiveFeeBps,
  boingPoolMinOutWithSlippage,
  getBoingPoolState,
  quoteBoingPoolAmountOut,
  type BoingPoolState,
} from './boingPool.js';
import { boingRouteHop, buildBoingCustodyRouterSwapTransaction, type BoingRouteHop } from './boingRouter.js';
import { CANONICAL_BOING_TESTNET_USDC_BOING_POOL_HEX } from './canonicalTestnetDex.js';

const word = (n: bigint) => n.toString(16).padStart(64, '0');

/**
 * Probe calldata: v8/v9 `swap(dir 0, amount_in 1, min_out 2^100)`. v8+ pools **fault** on any rejection
 * (failed `transfer_from` or unmeetable `min_out`); ledger-only v1..v7 pools silently `STOP` (success).
 */
export const BOING_CUSTODY_POOL_PROBE_CALLDATA_HEX = `0x${word(0x10n)}${word(0n)}${word(1n)}${word(1n << 100n)}`;

/** Pools known to be custody (v9) pools, so discovery does not need to probe them. */
export const BOING_KNOWN_CUSTODY_POOLS_HEX: readonly string[] = [CANONICAL_BOING_TESTNET_USDC_BOING_POOL_HEX.toLowerCase()];

/**
 * `true` when the pool is a custody (v8/v9) pool. Uses an unsigned `boing_simulateContractCall` (no state change):
 * a ledger-only pool returns success for the impossible probe swap, a custody pool faults.
 */
export async function probeBoingPoolCustody(client: Pick<BoingClient, 'simulateContractCall'>, poolHex: string): Promise<boolean> {
  const p = validateHex32(poolHex).toLowerCase();
  if (BOING_KNOWN_CUSTODY_POOLS_HEX.includes(p)) return true;
  const sim = await client.simulateContractCall(p, BOING_CUSTODY_POOL_PROBE_CALLDATA_HEX);
  return sim.success === false && typeof sim.error === 'string' && sim.error.length > 0;
}

export interface BoingCustodyPool {
  poolHex: string;
  tokenAHex: string;
  tokenBHex: string;
  reserveA: bigint;
  reserveB: bigint;
  feeBps: number | null;
  tokenADecimals: number | null;
  tokenBDecimals: number | null;
}

interface DexPoolRow {
  poolHex: string;
  tokenAHex: string;
  tokenBHex: string;
  reserveA?: string | null;
  reserveB?: string | null;
  feeBps?: number | null;
  tokenADecimals?: number | null;
  tokenBDecimals?: number | null;
}

/** Every factory-listed pool that passes {@link probeBoingPoolCustody}. A pool whose probe errors is skipped. */
export async function discoverBoingCustodyPools(
  client: Pick<BoingClient, 'listDexPoolsPage' | 'simulateContractCall'>,
  opts: { maxPages?: number; factory?: string } = {},
): Promise<BoingCustodyPool[]> {
  const rows: DexPoolRow[] = [];
  let cursor: string | null | undefined = null;
  for (let i = 0; i < (opts.maxPages ?? 10); i++) {
    const page = (await client.listDexPoolsPage({ cursor, light: true, ...(opts.factory ? { factory: opts.factory } : {}) })) as unknown as {
      pools?: DexPoolRow[];
      nextCursor?: string | null;
    };
    rows.push(...(page.pools ?? []));
    cursor = page.nextCursor;
    if (!cursor) break;
  }
  const out: BoingCustodyPool[] = [];
  for (const r of rows) {
    let custody = false;
    try {
      custody = await probeBoingPoolCustody(client, r.poolHex);
    } catch {
      custody = false;
    }
    if (!custody) continue;
    out.push({
      poolHex: r.poolHex.toLowerCase(),
      tokenAHex: r.tokenAHex.toLowerCase(),
      tokenBHex: r.tokenBHex.toLowerCase(),
      reserveA: BigInt(r.reserveA ?? '0'),
      reserveB: BigInt(r.reserveB ?? '0'),
      feeBps: r.feeBps ?? null,
      tokenADecimals: r.tokenADecimals ?? null,
      tokenBDecimals: r.tokenBDecimals ?? null,
    });
  }
  return out;
}

/** `sufficient`: allowance already covers the swap; `needs_approval`: approve exactly `amountIn` first. */
export type BoingCustodyApprovalState = 'sufficient' | 'needs_approval';

export interface BoingCustodySwapPlan {
  hop: BoingRouteHop;
  amountIn: bigint;
  amountOut: bigint;
  minOut: bigint;
  feeBps: bigint;
  allowance: bigint;
  approvalState: BoingCustodyApprovalState;
  /** Exact-amount `approve(router, amountIn)`; `null` when the allowance already covers it. */
  approveTx: TransactionInput | null;
  /** Custody-router swap (nonce = approve nonce + 1 when an approval is needed). */
  swapTx: TransactionInput;
}

/** Pure planner: no RPC. `state` from `getBoingPoolState`, `allowance` from `getBoingTokenAllowance(token, sender, router)`. */
export function planBoingCustodySwap(input: {
  poolHex: string;
  state: Pick<BoingPoolState, 'reserveA' | 'reserveB' | 'storedFeeBps' | 'tokenAHex' | 'tokenBHex'>;
  tokenInHex: string;
  amountIn: bigint;
  slippageBps: bigint;
  senderHex: string;
  routerHex: string;
  allowance: bigint;
  nonce: bigint;
  recipientHex?: string;
  deadlineHeight?: bigint;
}): BoingCustodySwapPlan {
  if (input.amountIn <= 0n) throw new Error('amountIn must be > 0');
  if (input.slippageBps < 0n || input.slippageBps >= 10_000n) throw new Error('slippageBps must be in [0, 10000)');
  const tin = validateHex32(input.tokenInHex).toLowerCase();
  const ta = input.state.tokenAHex.toLowerCase().slice(-64);
  const tb = input.state.tokenBHex.toLowerCase().slice(-64);
  let direction: 0 | 1;
  if (tin.slice(-64) === ta) direction = 0;
  else if (tin.slice(-64) === tb) direction = 1;
  else throw new Error('token is not in this pool');
  const hop = boingRouteHop(input.poolHex, direction, `0x${ta}`, `0x${tb}`);
  const [ri, ro] = direction === 0 ? [input.state.reserveA, input.state.reserveB] : [input.state.reserveB, input.state.reserveA];
  const feeBps = boingPoolEffectiveFeeBps(9, input.state.storedFeeBps);
  const amountOut = quoteBoingPoolAmountOut(9, ri, ro, input.amountIn, feeBps);
  if (amountOut <= 0n) throw new Error('quote is 0 (amount too small or pool empty)');
  let minOut = boingPoolMinOutWithSlippage(amountOut, input.slippageBps);
  if (minOut > amountOut) minOut = amountOut;
  const approvalState: BoingCustodyApprovalState = input.allowance >= input.amountIn ? 'sufficient' : 'needs_approval';
  let nonce = input.nonce;
  let approveTx: TransactionInput | null = null;
  if (approvalState === 'needs_approval') {
    approveTx = buildBoingTokenApproveTransaction({ nonce, senderHex: input.senderHex, tokenHex: hop.tokenInHex, toHex: input.routerHex, amount: input.amountIn });
    nonce += 1n;
  }
  const swapTx = buildBoingCustodyRouterSwapTransaction({
    nonce,
    senderHex: input.senderHex,
    routerHex: input.routerHex,
    hops: [hop],
    amountIn: input.amountIn,
    minOut,
    recipientHex: input.recipientHex ?? input.senderHex,
    deadline: input.deadlineHeight ?? 0n,
  });
  return { hop, amountIn: input.amountIn, amountOut, minOut, feeBps, allowance: input.allowance, approvalState, approveTx, swapTx };
}

/** Live reads + {@link planBoingCustodySwap} (fresh reserves, allowance, next nonce). */
export async function quoteBoingCustodySwap(
  client: BoingClient,
  input: Omit<Parameters<typeof planBoingCustodySwap>[0], 'state' | 'allowance' | 'nonce'> & { nonce?: bigint },
): Promise<BoingCustodySwapPlan> {
  const [state, allowance] = await Promise.all([
    getBoingPoolState(client, input.poolHex),
    getBoingTokenAllowance(client, input.tokenInHex, input.senderHex, input.routerHex),
  ]);
  let nonce = input.nonce;
  if (nonce == null) {
    const acct = await client.getAccount(validateHex32(input.senderHex));
    nonce = BigInt((acct as { nonce?: number | string } | null)?.nonce ?? 0);
  }
  return planBoingCustodySwap({ ...input, state, allowance, nonce });
}

/** Boing Express `contract_call` object for a contract-call `TransactionInput` (hex fields, snake_case). */
export function boingContractCallToExpressTx(tx: TransactionInput): {
  type: 'contract_call';
  contract: string;
  calldata: string;
  access_list: { read: string[]; write: string[] };
} {
  if (tx.payload.kind !== 'contractCall') throw new Error('not a contract call');
  return {
    type: 'contract_call',
    contract: accountIdToHex(tx.payload.contract),
    calldata: bytesToHex(tx.payload.calldata),
    access_list: { read: tx.accessList.read.map(accountIdToHex), write: tx.accessList.write.map(accountIdToHex) },
  };
}