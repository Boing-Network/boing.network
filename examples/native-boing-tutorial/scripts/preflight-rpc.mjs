#!/usr/bin/env node
/**
 * Single-process preflight (avoids Windows libuv issues from back-to-back spawnSync + fetch teardown).
 *
 * 1) Same checks as check-testnet-rpc.mjs — boing_chainHeight (optional BOING_PROBE_FULL)
 * 2) One line like observer-chain-tip-poll with BOING_POLL_ONCE — height + getSyncState (errors → sync._error)
 *
 * Env: BOING_RPC_URL (default https://testnet-rpc.boing.network), BOING_PROBE_FULL
 */
import {
  countAvailableBoingRpcMethods,
  createClient,
  explainBoingRpcError,
  explainBoingRpcProbeGaps,
  probeBoingRpcCapabilities,
} from '@boing-network/boing-sdk';

const defaultRpc = 'https://testnet-rpc.boing.network';
const rpc = (process.env.BOING_RPC_URL?.trim() || defaultRpc).replace(/\/$/, '');
const fullProbe = process.env.BOING_PROBE_FULL === '1' || process.env.BOING_PROBE_FULL === 'true';

function scheduleExit(code) {
  setTimeout(() => process.exit(code), 50);
}

async function run() {
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
  } else {
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
  }

  let sync = null;
  try {
    sync = await client.getSyncState();
  } catch (e) {
    sync = { _error: String(e) };
  }

  console.log(
    JSON.stringify({
      ok: true,
      rpc,
      height,
      at: new Date().toISOString(),
      sync,
    })
  );

  scheduleExit(0);
}

run().catch((e) => {
  console.error(explainBoingRpcError(e));
  scheduleExit(1);
});
