/**
 * Custody router v1 (`boing pool router-deploy | route`): pulls the swap input from the signer with
 * reference-token v2 `transfer_from`, then chains 1..=4 v8/v9 `swap_to` hops. Intermediate hops must be
 * **v9** pools (their swap returns `amount_out`); the last hop may be v8 or v9. Mirrors
 * `crates/boing-cli/src/router.rs` byte-for-byte (see `tests/boingRouter.test.ts`).
 */
import type { BoingClient } from './client.js';
import type { TransactionInput } from './bincode.js';
import { type BoingPoolSignedTx, type BoingPoolSendOptions } from './boingPool.js';
import { type Ed25519SecretKey32 } from './transactionBuilder.js';
export declare const BOING_CUSTODY_ROUTER_SELECTOR_SWAP = 241;
export declare const BOING_CUSTODY_ROUTER_MAX_HOPS = 4;
export declare const BOING_CUSTODY_ROUTER_BYTECODE_HEX: `0x${string}`;
/** `direction`: `0` = A->B, `1` = B->A. `tokenOutHex` is only used for the access list / path check. */
export interface BoingRouteHop {
    poolHex: string;
    direction: 0 | 1;
    tokenInHex: string;
    tokenOutHex: string;
}
export declare function encodeBoingCustodyRouterSwapCalldata(hops: readonly Pick<BoingRouteHop, 'poolHex' | 'direction' | 'tokenInHex'>[], amountIn: bigint, minOut: bigint, recipientHex: string, deadline?: bigint): Uint8Array;
/** Same check as the CLI: 1..=4 hops and `tokenOut[i] == tokenIn[i+1]`. */
export declare function checkBoingRoutePath(hops: readonly BoingRouteHop[]): void;
/** Resolve a hop from the pool's token pair. */
export declare function boingRouteHop(poolHex: string, direction: 0 | 1, tokenAHex: string, tokenBHex: string): BoingRouteHop;
export declare function buildBoingCustodyRouterDeployTransaction(input: {
    nonce: bigint;
    senderHex: string;
}): TransactionInput;
/** Router call; access list = sender, router, every pool / token on the path, plus extras. */
export declare function buildBoingCustodyRouterSwapTransaction(input: {
    nonce: bigint;
    senderHex: string;
    routerHex: string;
    hops: readonly BoingRouteHop[];
    amountIn: bigint;
    minOut: bigint;
    recipientHex: string;
    deadline?: bigint;
    extraAccountsHex?: readonly string[];
}): TransactionInput;
/** Chained live quote (v8/v9 fee-on-input math). */
export declare function quoteBoingRoute(client: BoingClient, hops: readonly BoingRouteHop[], amountIn: bigint): Promise<bigint>;
/** `boing pool route`: approve the router for exactly `amountIn` (unless `noApprove`), then route. */
export declare function routeBoingSwap(client: BoingClient | null, secretKey32: Ed25519SecretKey32, input: {
    routerHex: string;
    hops: readonly BoingRouteHop[];
    amountIn: bigint;
    minOut?: bigint;
    slippageBps?: bigint;
    toHex?: string;
    noApprove?: boolean;
    deadlineHeight?: bigint;
    extraAccountsHex?: readonly string[];
} & BoingPoolSendOptions): Promise<BoingPoolSignedTx & {
    minOut: bigint;
    approvals: BoingPoolSignedTx[];
}>;
//# sourceMappingURL=boingRouter.d.ts.map