/**
 * Constant-product **routing** and **quote aggregation** over Boing native pools (pure math + optional RPC hydrate).
 * No external chains. Execution still uses **`contract_call`** / multihop router encoders elsewhere in the SDK.
 */
import { fetchNativeDexDirectorySnapshot } from './nativeDexDirectory.js';
import { validateHex32 } from './hex.js';
import { mapWithConcurrencyLimit } from './indexerBatch.js';
import { NATIVE_CP_SWAP_FEE_BPS, constantProductAmountOutWithFeeBps } from './nativeAmm.js';
import { fetchNativeConstantProductPoolSnapshot, fetchNativeConstantProductSwapFeeBps, } from './nativeAmmPool.js';
function normHex32(h) {
    return validateHex32(h).toLowerCase();
}
function effectiveFeeBps(raw) {
    return raw === 0n ? BigInt(NATIVE_CP_SWAP_FEE_BPS) : raw;
}
/**
 * Exact output quote for **`tokenIn` → opposite side** on one venue (fails if **`tokenIn`** is not **`tokenA`** or **`tokenB`**).
 */
export function quoteCpPoolSwap(venue, tokenInHex, amountIn) {
    if (amountIn < 0n)
        throw new RangeError('amountIn must be non-negative');
    const a = normHex32(venue.tokenAHex);
    const b = normHex32(venue.tokenBHex);
    const tin = normHex32(tokenInHex);
    if (tin === a) {
        const amountOut = constantProductAmountOutWithFeeBps(venue.reserveA, venue.reserveB, amountIn, venue.feeBps);
        return { amountOut, tokenOutHex: b, directionForSwapCalldata: 0n };
    }
    if (tin === b) {
        const amountOut = constantProductAmountOutWithFeeBps(venue.reserveB, venue.reserveA, amountIn, venue.feeBps);
        return { amountOut, tokenOutHex: a, directionForSwapCalldata: 1n };
    }
    throw new Error('quoteCpPoolSwap: tokenIn is not a pool token');
}
/** Rank venues that list **`(tokenIn, tokenOut)`** by **`amountOut`** for a given **`amountIn`** (best first). */
export function rankDirectCpPools(venues, tokenInHex, tokenOutHex, amountIn) {
    const tout = normHex32(tokenOutHex);
    const out = [];
    for (const v of venues) {
        let q;
        try {
            q = quoteCpPoolSwap(v, tokenInHex, amountIn);
        }
        catch {
            continue;
        }
        if (q.tokenOutHex === tout && q.amountOut > 0n) {
            out.push({ venue: v, amountOut: q.amountOut, directionForSwapCalldata: q.directionForSwapCalldata });
        }
    }
    out.sort((x, y) => (x.amountOut > y.amountOut ? -1 : x.amountOut < y.amountOut ? 1 : 0));
    return out;
}
function edgesFromToken(venues, currentToken) {
    const t = normHex32(currentToken);
    const hit = [];
    for (const v of venues) {
        const a = normHex32(v.tokenAHex);
        const b = normHex32(v.tokenBHex);
        if (a === t || b === t)
            hit.push(v);
    }
    return hit;
}
/**
 * Enumerate simple CP paths (**no** same-pool reuse) up to **`maxHops`** pools; returns routes sorted by **`amountOut`** (best first).
 * For production UIs, cap **`maxHops`** at **2–3** and keep **`venues`** small (indexed subgraph).
 */
export function findBestCpRoutes(venues, tokenInHex, tokenOutHex, amountIn, options) {
    const maxHops = options?.maxHops ?? 3;
    const maxRoutes = options?.maxRoutes ?? 32;
    if (maxHops < 1)
        throw new RangeError('maxHops must be >= 1');
    const tin = normHex32(tokenInHex);
    const tout = normHex32(tokenOutHex);
    if (tin === tout) {
        return [{ hops: [], tokenInHex: tin, tokenOutHex: tout, amountIn, amountOut: amountIn }];
    }
    const routes = [];
    function dfs(currentToken, amount, path, visitedPools) {
        if (routes.length >= maxRoutes)
            return;
        for (const v of edgesFromToken(venues, currentToken)) {
            const ph = normHex32(v.poolHex);
            if (visitedPools.has(ph))
                continue;
            let q;
            try {
                q = quoteCpPoolSwap(v, currentToken, amount);
            }
            catch {
                continue;
            }
            if (q.amountOut === 0n)
                continue;
            const hop = {
                venue: v,
                tokenInHex: currentToken,
                tokenOutHex: q.tokenOutHex,
                amountIn: amount,
                amountOut: q.amountOut,
                directionForSwapCalldata: q.directionForSwapCalldata,
            };
            if (normHex32(q.tokenOutHex) === tout) {
                routes.push({
                    hops: [...path, hop],
                    tokenInHex: tin,
                    tokenOutHex: tout,
                    amountIn,
                    amountOut: q.amountOut,
                });
                continue;
            }
            if (path.length + 1 >= maxHops)
                continue;
            visitedPools.add(ph);
            dfs(q.tokenOutHex, q.amountOut, [...path, hop], visitedPools);
            visitedPools.delete(ph);
            if (routes.length >= maxRoutes)
                return;
        }
    }
    dfs(tin, amountIn, [], new Set());
    routes.sort((a, b) => (a.amountOut > b.amountOut ? -1 : a.amountOut < b.amountOut ? 1 : 0));
    return routes.slice(0, maxRoutes);
}
/** Best route or **`undefined`** when none. */
export function findBestCpRoute(venues, tokenInHex, tokenOutHex, amountIn, options) {
    return findBestCpRoutes(venues, tokenInHex, tokenOutHex, amountIn, options)[0];
}
/**
 * Even-split **aggregation heuristic**: divide **`totalAmountIn`** across the top **`poolCount`** direct pools (by full-size quote rank), sum outputs.
 * Not an optimal CEX splitter; useful for UI estimates and incremental liquidity use.
 */
export function quoteCpEvenSplitAcrossDirectPools(venues, tokenInHex, tokenOutHex, totalAmountIn, poolCount) {
    if (poolCount < 1)
        throw new RangeError('poolCount must be >= 1');
    if (totalAmountIn < 0n)
        throw new RangeError('totalAmountIn must be non-negative');
    const ranked = rankDirectCpPools(venues, tokenInHex, tokenOutHex, totalAmountIn);
    const n = Math.min(poolCount, ranked.length);
    if (n === 0 || totalAmountIn === 0n) {
        return { rankedPools: ranked, allocations: [], totalOut: 0n };
    }
    const base = totalAmountIn / BigInt(n);
    const rem = Number(totalAmountIn % BigInt(n));
    const allocations = [];
    let totalOut = 0n;
    for (let i = 0; i < n; i++) {
        const row = ranked[i];
        const ai = base + (i < rem ? 1n : 0n);
        if (ai === 0n)
            continue;
        const q = quoteCpPoolSwap(row.venue, tokenInHex, ai);
        allocations.push({
            venue: row.venue,
            amountIn: ai,
            amountOut: q.amountOut,
            directionForSwapCalldata: q.directionForSwapCalldata,
        });
        totalOut += q.amountOut;
    }
    return { rankedPools: ranked, allocations, totalOut };
}
function dedupePoolRows(rows) {
    const m = new Map();
    for (const r of rows) {
        const k = normHex32(r.poolHex);
        if (!m.has(k))
            m.set(k, r);
    }
    return [...m.values()];
}
/**
 * Hydrate **`CpPoolVenue`** rows from Boing RPC (reserves + fee bps per pool). **Boing-only.**
 */
export async function hydrateCpPoolVenuesFromRpc(client, rows, options) {
    const uniq = dedupePoolRows(rows);
    const concurrency = options?.concurrency ?? 8;
    return mapWithConcurrencyLimit(uniq, concurrency, async (r) => {
        const poolHex = validateHex32(r.poolHex);
        const tokenAHex = validateHex32(r.tokenAHex);
        const tokenBHex = validateHex32(r.tokenBHex);
        const [snap, feeRaw] = await Promise.all([
            fetchNativeConstantProductPoolSnapshot(client, poolHex),
            fetchNativeConstantProductSwapFeeBps(client, poolHex),
        ]);
        return {
            poolHex,
            tokenAHex,
            tokenBHex,
            reserveA: snap.reserveA,
            reserveB: snap.reserveB,
            feeBps: effectiveFeeBps(feeRaw),
        };
    });
}
/**
 * **Boing-only** pipeline: directory **`register_pair`** log range → hydrate venues → best CP route(s).
 * Pair with **`encodeNativeDexSwap3RouterCalldata128`** etc. when executing multihop on-chain.
 */
export async function fetchCpRoutingFromDirectoryLogs(client, tokenInHex, tokenOutHex, amountIn, options) {
    const snapshot = await fetchNativeDexDirectorySnapshot(client, {
        overrides: options.overrides,
        registerLogs: options.registerLogs,
    });
    const logs = snapshot.registerLogs ?? [];
    const rows = logs.map((l) => ({
        poolHex: l.poolHex,
        tokenAHex: l.tokenAHex,
        tokenBHex: l.tokenBHex,
    }));
    const venues = await hydrateCpPoolVenuesFromRpc(client, rows, {
        concurrency: options.hydrateConcurrency,
    });
    const routes = findBestCpRoutes(venues, tokenInHex, tokenOutHex, amountIn, {
        maxHops: options.maxHops,
        maxRoutes: options.maxRoutes,
    });
    return { snapshot, venues, routes };
}
