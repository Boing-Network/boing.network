/**
 * Custody-pool (v8/v9) swap planning for apps that sign through a wallet (e.g. boing.finance + Boing Express):
 * discover pools from `boing_listDexPools`, tell custody pools apart from ledger-only v1..v7 pools, quote,
 * compute the allowance state and build the exact-amount `approve(router)` + custody-router swap transactions.
 * Signing / submitting is left to the caller (same calldata as `routeBoingSwap`).
 */
import type { BoingClient } from './client.js';
import type { TransactionInput } from './bincode.js';
import { type BoingPoolState } from './boingPool.js';
import { type BoingRouteHop } from './boingRouter.js';
/**
 * Probe calldata: v8/v9 `swap(dir 0, amount_in 1, min_out 2^100)`. v8+ pools **fault** on any rejection
 * (failed `transfer_from` or unmeetable `min_out`); ledger-only v1..v7 pools silently `STOP` (success).
 */
export declare const BOING_CUSTODY_POOL_PROBE_CALLDATA_HEX: string;
/** Pools known to be custody (v9) pools, so discovery does not need to probe them. */
export declare const BOING_KNOWN_CUSTODY_POOLS_HEX: readonly string[];
/**
 * `true` when the pool is a custody (v8/v9) pool. Uses an unsigned `boing_simulateContractCall` (no state change):
 * a ledger-only pool returns success for the impossible probe swap, a custody pool faults.
 */
export declare function probeBoingPoolCustody(client: Pick<BoingClient, 'simulateContractCall'>, poolHex: string): Promise<boolean>;
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
/** Every factory-listed pool that passes {@link probeBoingPoolCustody}. A pool whose probe errors is skipped. */
export declare function discoverBoingCustodyPools(client: Pick<BoingClient, 'listDexPoolsPage' | 'simulateContractCall'>, opts?: {
    maxPages?: number;
    factory?: string;
}): Promise<BoingCustodyPool[]>;
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
export declare function planBoingCustodySwap(input: {
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
}): BoingCustodySwapPlan;
/** Live reads + {@link planBoingCustodySwap} (fresh reserves, allowance, next nonce). */
export declare function quoteBoingCustodySwap(client: BoingClient, input: Omit<Parameters<typeof planBoingCustodySwap>[0], 'state' | 'allowance' | 'nonce'> & {
    nonce?: bigint;
}): Promise<BoingCustodySwapPlan>;
/** Boing Express `contract_call` object for a contract-call `TransactionInput` (hex fields, snake_case). */
export declare function boingContractCallToExpressTx(tx: TransactionInput): {
    type: 'contract_call';
    contract: string;
    calldata: string;
    access_list: {
        read: string[];
        write: string[];
    };
};
//# sourceMappingURL=boingCustodySwap.d.ts.map