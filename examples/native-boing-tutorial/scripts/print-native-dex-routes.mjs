#!/usr/bin/env node
/**
 * Off-chain native DEX route finder: directory register logs → hydrate pool reserves → rank CP routes.
 * Boing JSON-RPC only. Does not submit transactions.
 *
 * Env:
 *   BOING_RPC_URL           — default https://testnet-rpc.boing.network
 *   TOKEN_IN                — required, 32-byte account id hex (token A or B)
 *   TOKEN_OUT               — required
 *   AMOUNT_IN               — integer string, default 1000000 (token smallest units)
 *   BOING_FROM_BLOCK        — inclusive, default 0
 *   BOING_TO_BLOCK          — inclusive; omit = chain head from getNetworkInfo
 *   BOING_MAX_HOPS          — default 3
 *   BOING_MAX_ROUTES        — default 16
 *   BOING_HYDRATE_CONCURRENCY — default 8
 *   BOING_DEX_FACTORY_HEX   — optional override when node does not advertise factory
 *   BOING_OVERRIDE_CANONICAL_POOL_HEX — optional pool hint override (mergeNativeDexIntegrationDefaults)
 */
import {
  createClient,
  explainBoingRpcError,
  fetchCpRoutingFromDirectoryLogs,
} from '@boing-network/boing-sdk';

const rpc = process.env.BOING_RPC_URL ?? 'https://testnet-rpc.boing.network';
const tokenIn = process.env.TOKEN_IN?.trim();
const tokenOut = process.env.TOKEN_OUT?.trim();
const amountInStr = process.env.AMOUNT_IN ?? '1000000';
const fromBlock = parseInt(process.env.BOING_FROM_BLOCK ?? '0', 10);
const toBlockRaw = process.env.BOING_TO_BLOCK?.trim();
const maxHops = parseInt(process.env.BOING_MAX_HOPS ?? '3', 10);
const maxRoutes = parseInt(process.env.BOING_MAX_ROUTES ?? '16', 10);
const hydrateConcurrency = parseInt(process.env.BOING_HYDRATE_CONCURRENCY ?? '8', 10);

function scheduleExit(code) {
  setTimeout(() => process.exit(code), 10);
}

function serializeRoute(r) {
  return {
    amountOut: r.amountOut.toString(),
    hops: r.hops.map((h) => ({
      poolHex: h.venue.poolHex,
      tokenInHex: h.tokenInHex,
      tokenOutHex: h.tokenOutHex,
      amountIn: h.amountIn.toString(),
      amountOut: h.amountOut.toString(),
      directionForSwapCalldata: h.directionForSwapCalldata.toString(),
    })),
  };
}

async function main() {
  if (!tokenIn || !tokenOut) {
    console.error(
      JSON.stringify(
        {
          ok: false,
          error: 'TOKEN_IN and TOKEN_OUT are required (0x + 64 hex chars each).',
        },
        null,
        2
      )
    );
    scheduleExit(1);
    return;
  }

  let amountIn;
  try {
    amountIn = BigInt(amountInStr);
  } catch {
    console.error(JSON.stringify({ ok: false, error: 'AMOUNT_IN must be an integer string.' }, null, 2));
    scheduleExit(1);
    return;
  }

  const overrides = {};
  const fac = process.env.BOING_DEX_FACTORY_HEX?.trim();
  if (fac) overrides.nativeDexFactoryAccountHex = fac;
  const poolO = process.env.BOING_OVERRIDE_CANONICAL_POOL_HEX?.trim();
  if (poolO) overrides.nativeCpPoolAccountHex = poolO;

  const registerLogs = {
    fromBlock,
    ...(toBlockRaw != null && toBlockRaw !== '' ? { toBlock: parseInt(toBlockRaw, 10) } : {}),
  };

  const client = createClient(rpc);

  try {
    const { snapshot, venues, routes } = await fetchCpRoutingFromDirectoryLogs(
      client,
      tokenIn,
      tokenOut,
      amountIn,
      {
        overrides: Object.keys(overrides).length ? overrides : undefined,
        registerLogs,
        maxHops,
        maxRoutes,
        hydrateConcurrency,
      }
    );

    const registerLogCount = snapshot.registerLogs?.length ?? 0;

    console.log(
      JSON.stringify(
        {
          ok: true,
          rpc,
          tokenIn,
          tokenOut,
          amountIn: amountInStr,
          snapshot: {
            headHeight: snapshot.headHeight,
            chainId: snapshot.chainId,
            pairsCount: snapshot.pairsCount != null ? snapshot.pairsCount.toString() : null,
            poolSource: snapshot.defaults.poolSource,
            factorySource: snapshot.defaults.factorySource,
            registerLogCount,
            venueCount: venues.length,
          },
          routes: routes.map(serializeRoute),
          note:
            'Off-chain constant-product quotes only. Execute with per-hop or multihop router calldata from boing-sdk; see docs/NATIVE-DEX-MULTIHOP-SWAP-ROUTER.md.',
        },
        null,
        2
      )
    );
    scheduleExit(0);
  } catch (e) {
    console.error(
      JSON.stringify(
        {
          ok: false,
          rpc,
          phase: 'fetchCpRoutingFromDirectoryLogs',
          error: explainBoingRpcError(e),
        },
        null,
        2
      )
    );
    scheduleExit(1);
  }
}

main();
