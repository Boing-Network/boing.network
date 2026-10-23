#!/usr/bin/env node
/**
 * Minimal operator monitor: poll `boing_chainHeight` and `boing_getSyncState` (JSON lines to stdout).
 * Not a hosted explorer — use for tail -f / systemd / Docker health sidecar until boing.observer + indexer are live.
 *
 * Env:
 *   BOING_RPC_URL          — default http://127.0.0.1:8545
 *   BOING_POLL_INTERVAL_SECS — default 15 (minimum 1)
 *   BOING_STALL_WARN_SECS  — if > 0, stderr JSON when height unchanged for this many seconds
 *   BOING_POLL_ONCE        — if 1 / true: single poll, then exit (exit 1 on RPC error); for CI / scripts
 *
 * getSyncState: optional on the node. If the RPC returns -32601 (method not found), `sync` will contain
 * `_error` but the poll still succeeds when chainHeight works — upgrade boing-node or ignore for minimal gateways.
 * On Windows, exiting immediately after fetch can trigger libuv UV_HANDLE_CLOSING; exit is deferred (short timeout).
 */
import { createClient } from '@boing-network/boing-sdk';

const rpc = (process.env.BOING_RPC_URL ?? 'http://127.0.0.1:8545').replace(/\/$/, '');
const pollOnce =
  process.env.BOING_POLL_ONCE === '1' || process.env.BOING_POLL_ONCE === 'true';
const intervalMs = Math.max(1000, Number(process.env.BOING_POLL_INTERVAL_SECS ?? '15') * 1000);
const stallWarnSecs = Math.max(0, Number(process.env.BOING_STALL_WARN_SECS ?? '0'));

const client = createClient(rpc);
let lastHeight = null;
let stalledSince = null;

async function tick() {
  const height = await client.chainHeight();
  let sync = null;
  try {
    sync = await client.getSyncState();
  } catch (e) {
    sync = { _error: String(e) };
  }

  const now = Date.now();
  if (stallWarnSecs > 0 && lastHeight !== null && height === lastHeight) {
    if (stalledSince === null) stalledSince = now;
    else if ((now - stalledSince) / 1000 >= stallWarnSecs) {
      console.error(
        JSON.stringify({
          ok: false,
          warning: 'height_stalled',
          rpc,
          height,
          stallSeconds: Math.floor((now - stalledSince) / 1000),
          at: new Date().toISOString(),
        })
      );
    }
  } else {
    stalledSince = null;
  }
  lastHeight = height;

  console.log(
    JSON.stringify({
      ok: true,
      rpc,
      height,
      at: new Date().toISOString(),
      sync,
    })
  );
}

async function main() {
  if (pollOnce) {
    try {
      await tick();
    } catch (e) {
      console.error(
        JSON.stringify({
          ok: false,
          rpc,
          error: String(e),
          at: new Date().toISOString(),
        })
      );
      process.exitCode = 1;
    }
    return;
  }

  process.stdout.write(
    JSON.stringify({
      ok: true,
      phase: 'start',
      rpc,
      pollIntervalSecs: intervalMs / 1000,
      stallWarnSecs,
      at: new Date().toISOString(),
    }) + '\n'
  );
  for (;;) {
    try {
      await tick();
    } catch (e) {
      console.error(
        JSON.stringify({
          ok: false,
          rpc,
          error: String(e),
          at: new Date().toISOString(),
        })
      );
    }
    await new Promise((r) => setTimeout(r, intervalMs));
  }
}

function scheduleExit(code) {
  setTimeout(() => process.exit(code), 10);
}

main()
  .then(() => {
    if (pollOnce) {
      scheduleExit(process.exitCode ?? 0);
    }
  })
  .catch((e) => {
    console.error(
      JSON.stringify({
        ok: false,
        rpc,
        error: String(e),
        at: new Date().toISOString(),
      })
    );
    if (pollOnce) {
      scheduleExit(1);
    } else {
      process.exit(1);
    }
  });
