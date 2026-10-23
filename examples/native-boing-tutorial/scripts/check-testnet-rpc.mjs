#!/usr/bin/env node
/**
 * Preflight: confirm JSON-RPC is reachable and returns a sane chain height (no secrets).
 * Use before deploy / faucet / wallet flows when the public URL may be behind Cloudflare Tunnel.
 *
 * Env:
 *   BOING_RPC_URL     — default https://testnet-rpc.boing.network
 *   BOING_PROBE_FULL  — set to 1 to also run probeBoingRpcCapabilities (longer JSON)
 */
import {
  countAvailableBoingRpcMethods,
  createClient,
  explainBoingRpcError,
  explainBoingRpcProbeGaps,
  probeBoingRpcCapabilities,
} from '@boing-network/boing-sdk';

const rpc = process.env.BOING_RPC_URL ?? 'https://testnet-rpc.boing.network';
const fullProbe = process.env.BOING_PROBE_FULL === '1' || process.env.BOING_PROBE_FULL === 'true';

/** Avoid Windows libuv UV_HANDLE_CLOSING when exiting right after fetch. */
function scheduleExit(code) {
  setTimeout(() => process.exit(code), 10);
}

async function main() {
  const client = createClient(rpc);

  let height;
  try {
    height = await client.chainHeight();
  } catch (e) {
    console.error(
      JSON.stringify(
        {
          ok: false,
          rpc,
          phase: 'boing_chainHeight',
          error: explainBoingRpcError(e),
          hint:
            'Non-2xx (e.g. HTTP 530 / error code 1033) usually means Cloudflare Tunnel origin is down. See docs/RUNBOOK.md § 8.3. Try BOING_RPC_URL=http://127.0.0.1:8545 with a local boing-node.',
        },
        null,
        2
      )
    );
    scheduleExit(1);
    return;
  }

  const base = {
    ok: true,
    rpc,
    chainHeight: height,
    note: 'RPC answered boing_chainHeight; node is reachable at HTTP level.',
  };

  if (!fullProbe) {
    console.log(JSON.stringify(base, null, 2));
    scheduleExit(0);
    return;
  }

  const probe = await probeBoingRpcCapabilities(client);
  const diagnosis = explainBoingRpcProbeGaps(probe);
  console.log(
    JSON.stringify(
      {
        ...base,
        clientVersion: probe.clientVersion,
        availableMethodCount: countAvailableBoingRpcMethods(probe),
        methods: probe.methods,
        ...(diagnosis != null ? { diagnosis } : {}),
      },
      null,
      2
    )
  );
  scheduleExit(0);
}

main().catch((e) => {
  console.error(explainBoingRpcError(e));
  scheduleExit(1);
});
