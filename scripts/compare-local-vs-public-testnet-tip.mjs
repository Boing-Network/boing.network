#!/usr/bin/env node
/**
 * Compare a local JSON-RPC node tip vs public testnet — no boing-sdk build.
 *
 *   npm run compare-local-public-tip
 *
 * Env:
 *   BOING_LOCAL_RPC_URL   — default http://127.0.0.1:8545
 *   BOING_PUBLIC_RPC_URL  — default https://testnet-rpc.boing.network/
 *   BOING_SYNC_MAX_LAG    — max allowed blocks behind public tip (default 256); exit 2 if exceeded
 *   BOING_SYNC_OK_LAG     — "good enough" lag for messaging only (default 32)
 */
import { chainHeight, networkInfo } from './lib/boing-json-rpc-fetch.mjs';

const localUrl = (process.env.BOING_LOCAL_RPC_URL ?? 'http://127.0.0.1:8545').replace(/\/$/, '');
const publicUrl = (process.env.BOING_PUBLIC_RPC_URL ?? 'https://testnet-rpc.boing.network').replace(/\/$/, '');
const maxLag = Math.max(0, parseInt(process.env.BOING_SYNC_MAX_LAG ?? '256', 10) || 256);
const okLag = Math.max(0, parseInt(process.env.BOING_SYNC_OK_LAG ?? '32', 10) || 32);

function scheduleExit(code) {
  setTimeout(() => process.exit(code), 15);
}

async function main() {
  let localH;
  let publicH;
  try {
    localH = await chainHeight(localUrl);
  } catch (e) {
    console.error(
      JSON.stringify(
        {
          ok: false,
          phase: 'local_chainHeight',
          url: localUrl,
          message: e instanceof Error ? e.message : String(e),
          hint: 'Start a node on this machine or set BOING_LOCAL_RPC_URL to your full node (http://IP:8545).',
        },
        null,
        2
      )
    );
    scheduleExit(1);
    return;
  }

  try {
    publicH = await chainHeight(publicUrl);
  } catch (e) {
    console.error(
      JSON.stringify(
        {
          ok: false,
          phase: 'public_chainHeight',
          url: publicUrl,
          message: e instanceof Error ? e.message : String(e),
          hint: 'Public RPC unreachable — tunnel or network issue; see docs/RUNBOOK.md §8.3.',
        },
        null,
        2
      )
    );
    scheduleExit(1);
    return;
  }

  const [localInfo, publicInfo] = await Promise.all([networkInfo(localUrl), networkInfo(publicUrl)]);
  const lag = publicH - localH;

  const out = {
    ok: true,
    local: { url: localUrl, chainHeight: localH, chain_id: localInfo?.chain_id ?? null },
    public: { url: publicUrl, chainHeight: publicH, chain_id: publicInfo?.chain_id ?? null },
    lag_blocks_public_minus_local: lag,
    max_lag_allowed: maxLag,
    ok_lag_hint: okLag,
  };

  if (localInfo?.chain_id != null && publicInfo?.chain_id != null && localInfo.chain_id !== publicInfo.chain_id) {
    out.ok = false;
    out.reason = 'chain_id_mismatch';
    console.error(JSON.stringify(out, null, 2));
    scheduleExit(4);
    return;
  }

  if (lag > maxLag) {
    out.ok = false;
    out.reason = 'local_too_far_behind';
    out.hint =
      'Node may still be syncing, on wrong network/genesis, or disconnected from bootnodes. Check firewall P2P (TCP 4001) and bootnode list.';
    console.error(JSON.stringify(out, null, 2));
    scheduleExit(2);
    return;
  }

  if (localH === 0 && publicH > 100) {
    out.warning =
      'Local tip is 0 while public testnet is far ahead — if this persists, you are likely not on the public testnet (wrong data dir / genesis / bootnodes).';
  } else if (lag > okLag) {
    out.note = `Within tolerance (${maxLag}) but more than ${okLag} blocks behind — sync may still be catching up.`;
  } else {
    out.note = 'Local tip is close to public testnet — good sign you are joined.';
  }

  console.log(JSON.stringify(out, null, 2));
  scheduleExit(0);
}

main().catch((e) => {
  console.error(JSON.stringify({ ok: false, message: e instanceof Error ? e.message : String(e) }, null, 2));
  scheduleExit(1);
});
