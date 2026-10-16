import { accountIdToHex, bytesToHex, validateHex32 } from './hex.js';
import { buildBoingTokenApproveTransaction, getBoingTokenAllowance } from './boingToken.js';
import { boingPoolEffectiveFeeBps, boingPoolMinOutWithSlippage, getBoingPoolState, quoteBoingPoolAmountOut, } from './boingPool.js';
import { boingRouteHop, buildBoingCustodyRouterSwapTransaction } from './boingRouter.js';
import { CANONICAL_BOING_TESTNET_USDC_BOING_POOL_HEX } from './canonicalTestnetDex.js';
const word = (n) => n.toString(16).padStart(64, '0');
/**
 * Probe calldata: v8/v9 `swap(dir 0, amount_in 1, min_out 2^100)`. v8+ pools **fault** on any rejection
 * (failed `transfer_from` or unmeetable `min_out`); ledger-only v1..v7 pools silently `STOP` (success).
 */
export const BOING_CUSTODY_POOL_PROBE_CALLDATA_HEX = `0x${word(0x10n)}${word(0n)}${word(1n)}${word(1n << 100n)}`;
/** Pools known to be custody (v9) pools, so discovery does not need to probe them. */
export const BOING_KNOWN_CUSTODY_POOLS_HEX = [CANONICAL_BOING_TESTNET_USDC_BOING_POOL_HEX.toLowerCase()];
/**
 * `true` when the pool is a custody (v8/v9) pool. Uses an unsigned `boing_simulateContractCall` (no state change):
 * a ledger-only pool returns success for the impossible probe swap, a custody pool faults.
 */
export async function probeBoingPoolCustody(client, poolHex) {
    const p = validateHex32(poolHex).toLowerCase();
    if (BOING_KNOWN_CUSTODY_POOLS_HEX.includes(p))
        return true;
    const sim = await client.simulateContractCall(p, BOING_CUSTODY_POOL_PROBE_CALLDATA_HEX);
    return sim.success === false && typeof sim.error === 'string' && sim.error.length > 0;
}
/** Every factory-listed pool that passes {@link probeBoingPoolCustody}. A pool whose probe errors is skipped. */
export async function discoverBoingCustodyPools(client, opts = {}) {
    const rows = [];
    let cursor = null;
    for (let i = 0; i < (opts.maxPages ?? 10); i++) {
        const page = (await client.listDexPoolsPage({ cursor, light: true, ...(opts.factory ? { factory: opts.factory } : {}) }));
        rows.push(...(page.pools ?? []));
        cursor = page.nextCursor;
        if (!cursor)
            break;
    }
    const out = [];
    for (const r of rows) {
        let custody = false;
        try {
            custody = await probeBoingPoolCustody(client, r.poolHex);
        }
        catch {
            custody = false;
        }
        if (!custody)
            continue;
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
/** Pure planner: no RPC. `state` from `getBoingPoolState`, `allowance` from `getBoingTokenAllowance(token, sender, router)`. */
export function planBoingCustodySwap(input) {
    if (input.amountIn <= 0n)
        throw new Error('amountIn must be > 0');
    if (input.slippageBps < 0n || input.slippageBps >= 10000n)
        throw new Error('slippageBps must be in [0, 10000)');
    const tin = validateHex32(input.tokenInHex).toLowerCase();
    const ta = input.state.tokenAHex.toLowerCase().slice(-64);
    const tb = input.state.tokenBHex.toLowerCase().slice(-64);
    let direction;
    if (tin.slice(-64) === ta)
        direction = 0;
    else if (tin.slice(-64) === tb)
        direction = 1;
    else
        throw new Error('token is not in this pool');
    const hop = boingRouteHop(input.poolHex, direction, `0x${ta}`, `0x${tb}`);
    const [ri, ro] = direction === 0 ? [input.state.reserveA, input.state.reserveB] : [input.state.reserveB, input.state.reserveA];
    const feeBps = boingPoolEffectiveFeeBps(9, input.state.storedFeeBps);
    const amountOut = quoteBoingPoolAmountOut(9, ri, ro, input.amountIn, feeBps);
    if (amountOut <= 0n)
        throw new Error('quote is 0 (amount too small or pool empty)');
    let minOut = boingPoolMinOutWithSlippage(amountOut, input.slippageBps);
    if (minOut > amountOut)
        minOut = amountOut;
    const approvalState = input.allowance >= input.amountIn ? 'sufficient' : 'needs_approval';
    let nonce = input.nonce;
    let approveTx = null;
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
export async function quoteBoingCustodySwap(client, input) {
    const [state, allowance] = await Promise.all([
        getBoingPoolState(client, input.poolHex),
        getBoingTokenAllowance(client, input.tokenInHex, input.senderHex, input.routerHex),
    ]);
    let nonce = input.nonce;
    if (nonce == null) {
        const acct = await client.getAccount(validateHex32(input.senderHex));
        nonce = BigInt(acct?.nonce ?? 0);
    }
    return planBoingCustodySwap({ ...input, state, allowance, nonce });
}
/** Boing Express `contract_call` object for a contract-call `TransactionInput` (hex fields, snake_case). */
export function boingContractCallToExpressTx(tx) {
    if (tx.payload.kind !== 'contractCall')
        throw new Error('not a contract call');
    return {
        type: 'contract_call',
        contract: accountIdToHex(tx.payload.contract),
        calldata: bytesToHex(tx.payload.calldata),
        access_list: { read: tx.accessList.read.map(accountIdToHex), write: tx.accessList.write.map(accountIdToHex) },
    };
}
