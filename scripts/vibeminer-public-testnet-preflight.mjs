#!/usr/bin/env node
/**
 * Automated checks for VibeMiner-style public testnet nodes (see docs/VIBEMINER-INTEGRATION.md).
 *
 *   npm run vibeminer-public-testnet-preflight
 *
 * Env:
 *   BOING_LOCAL_RPC_URL       — default http://127.0.0.1:8545
 *   BOING_PUBLIC_RPC_URL      — default https://testnet-rpc.boing.network
 *   BOING_BOOTNODES           — comma-separated multiaddrs (default: testnet bootnodes from website fallbacks)
 *   BOING_OFFICIAL_NETWORKS_URL — default https://boing.network/api/networks
 *   BOING_SYNC_MAX_LAG        — same as compare-local-public-tip (default 256)
 *   BOING_PREFLIGHT_SKIP_TCP  — set to 1 to skip outbound TCP checks to bootnodes
 *   BOING_PREFLIGHT_BOOTNODE_TCP_STRICT — set to 1 to fail (exit 3) when no raw TCP connect succeeds; default is warn-only (libp2p can work when this probe times out)
 *   BOING_BOOTNODE_TCP_TIMEOUT_MS — per-probe timeout (default 12000)
 *   BOING_PROBE_LOCAL_P2P     — set to 1 to try TCP connect 127.0.0.1:BOING_LOCAL_P2P_PORT (default 4001)
 */
import net from 'node:net';
import {
  chainHeight,
  clientVersion,
  networkInfo,
  parseIp4TcpMultiaddrs,
  syncState,
} from './lib/boing-json-rpc-fetch.mjs';

const localUrl = (process.env.BOING_LOCAL_RPC_URL ?? 'http://127.0.0.1:8545').replace(/\/$/, '');
const publicUrl = (process.env.BOING_PUBLIC_RPC_URL ?? 'https://testnet-rpc.boing.network').replace(/\/$/, '');
const networksUrl = (process.env.BOING_OFFICIAL_NETWORKS_URL ?? 'https://boing.network/api/networks').replace(
  /\/$/,
  ''
);
const maxLag = Math.max(0, parseInt(process.env.BOING_SYNC_MAX_LAG ?? '256', 10) || 256);
const okLag = Math.max(0, parseInt(process.env.BOING_SYNC_OK_LAG ?? '32', 10) || 32);
const skipTcp = process.env.BOING_PREFLIGHT_SKIP_TCP === '1' || process.env.BOING_PREFLIGHT_SKIP_TCP === 'true';
const tcpStrict =
  process.env.BOING_PREFLIGHT_BOOTNODE_TCP_STRICT === '1' ||
  process.env.BOING_PREFLIGHT_BOOTNODE_TCP_STRICT === 'true';
const tcpTimeoutMs = Math.max(
  1000,
  parseInt(process.env.BOING_BOOTNODE_TCP_TIMEOUT_MS ?? '12000', 10) || 12000
);
const probeLocalP2p =
  process.env.BOING_PROBE_LOCAL_P2P === '1' || process.env.BOING_PROBE_LOCAL_P2P === 'true';
const localP2pPort = Math.max(1, parseInt(process.env.BOING_LOCAL_P2P_PORT ?? '4001', 10) || 4001);

const DEFAULT_BOOTNODES = '/ip4/169.155.48.188/tcp/4001,/ip4/109.105.220.118/tcp/4001';

/** @param {{ host: string, port: number }[]} targets */
function dedupeTargets(targets) {
  const seen = new Set();
  const out = [];
  for (const t of targets) {
    const k = `${t.host}:${t.port}`;
    if (seen.has(k)) continue;
    seen.add(k);
    out.push(t);
  }
  return out;
}

function scheduleExit(code) {
  setTimeout(() => process.exit(code), 20);
}

/** @param {string} host @param {number} port @param {number} ms */
function tcpProbe(host, port, ms = 6000) {
  return new Promise((resolve) => {
    const sock = net.createConnection({ host, port }, () => {
      sock.destroy();
      resolve({ ok: true, host, port });
    });
    sock.setTimeout(ms);
    sock.on('timeout', () => {
      sock.destroy();
      resolve({ ok: false, host, port, reason: 'timeout' });
    });
    sock.on('error', (e) => {
      resolve({ ok: false, host, port, reason: e instanceof Error ? e.message : String(e) });
    });
  });
}

async function fetchOfficialNetworksMeta() {
  try {
    const res = await fetch(networksUrl);
    const j = await res.json();
    if (!j || typeof j !== 'object') return { ok: false, reason: 'invalid_json' };
    const tag = j.meta?.boing_testnet_download_tag;
    const rpc = j.meta?.public_testnet_rpc_url;
    return {
      ok: true,
      boing_testnet_download_tag: typeof tag === 'string' ? tag : null,
      public_testnet_rpc_url: typeof rpc === 'string' ? rpc : null,
    };
  } catch (e) {
    return { ok: false, reason: e instanceof Error ? e.message : String(e) };
  }
}

async function main() {
  const report = {
    ok: true,
    steps: {},
    hints: [],
    warnings: [],
  };

  /** 1–2: TCP to bootnodes (best-effort; libp2p may work when raw TCP times out — see bootnode_tcp.mode) */
  const bootRaw = (process.env.BOING_BOOTNODES ?? DEFAULT_BOOTNODES).trim();
  const targets = dedupeTargets(parseIp4TcpMultiaddrs(bootRaw));
  if (!skipTcp) {
    if (targets.length === 0) {
      report.steps.bootnode_tcp = { skipped: true, reason: 'no parseable /ip4/.../tcp/... in BOING_BOOTNODES' };
      report.hints.push('Set BOING_BOOTNODES to comma-separated /ip4/x.x.x.x/tcp/4001 multiaddrs.');
    } else {
      const results = [];
      for (const t of targets) {
        results.push(await tcpProbe(t.host, t.port, tcpTimeoutMs));
      }
      const anyOk = results.some((r) => r.ok);
      report.steps.bootnode_tcp = {
        targets,
        results,
        any_ok: anyOk,
        timeout_ms: tcpTimeoutMs,
        mode: tcpStrict ? 'strict' : 'warn_only',
      };
      if (!anyOk) {
        const msg =
          'Raw TCP to bootnode :4001 did not complete within timeout — often a false negative (ISP/NAT, libp2p still dials). For CI hard-gate set BOING_PREFLIGHT_BOOTNODE_TCP_STRICT=1; to probe longer set BOING_BOOTNODE_TCP_TIMEOUT_MS.';
        if (tcpStrict) {
          report.ok = false;
          report.hints.push(msg);
        } else {
          report.warnings.push(msg);
        }
      }
    }
  } else {
    report.steps.bootnode_tcp = { skipped: true, reason: 'BOING_PREFLIGHT_SKIP_TCP' };
  }

  /** Optional: local P2P listener (validator/full node should listen if P2P enabled) */
  if (probeLocalP2p) {
    const r = await tcpProbe('127.0.0.1', localP2pPort, 3000);
    report.steps.local_p2p_listen = { port: localP2pPort, ...r };
    if (!r.ok) {
      report.hints.push(
        `No TCP accept on 127.0.0.1:${localP2pPort} — if the node is running with P2P, check --p2p-listen / firewall.`
      );
    }
  }

  /** 3: official networks meta vs local client string */
  const meta = await fetchOfficialNetworksMeta();
  report.steps.official_networks = meta;
  let localVer = null;
  try {
    localVer = await clientVersion(localUrl);
  } catch {
    report.steps.local_client_version = null;
  }
  report.steps.local_client_version = localVer;
  if (meta.ok && meta.boing_testnet_download_tag && localVer) {
    report.steps.binary_hint = {
      official_zip_tag: meta.boing_testnet_download_tag,
      local_boing_clientVersion: localVer,
      note: 'Compare your VibeMiner / listing zip tag with official_zip_tag; clientVersion is the running binary.',
    };
  }

  /** 4: chain tip + chain_id + sync (same as compare-local-public-tip) */
  let localH;
  let publicH;
  try {
    localH = await chainHeight(localUrl);
  } catch (e) {
    report.ok = false;
    report.steps.chain_tip = {
      ok: false,
      phase: 'local_chainHeight',
      url: localUrl,
      message: e instanceof Error ? e.message : String(e),
    };
    report.hints.push('Start the VibeMiner node or set BOING_LOCAL_RPC_URL.');
    console.error(JSON.stringify(report, null, 2));
    scheduleExit(1);
    return;
  }

  try {
    publicH = await chainHeight(publicUrl);
  } catch (e) {
    report.ok = false;
    report.steps.chain_tip = {
      ok: false,
      phase: 'public_chainHeight',
      url: publicUrl,
      message: e instanceof Error ? e.message : String(e),
    };
    report.hints.push('Public RPC unreachable — see docs/RUNBOOK.md §8.3.');
    console.error(JSON.stringify(report, null, 2));
    scheduleExit(1);
    return;
  }

  const [localInfo, publicInfo, localSync] = await Promise.all([
    networkInfo(localUrl),
    networkInfo(publicUrl),
    syncState(localUrl),
  ]);

  const lag = publicH - localH;
  report.steps.chain_tip = {
    local: { url: localUrl, chainHeight: localH, chain_id: localInfo?.chain_id ?? null },
    public: { url: publicUrl, chainHeight: publicH, chain_id: publicInfo?.chain_id ?? null },
    lag_blocks_public_minus_local: lag,
    max_lag_allowed: maxLag,
    sync_state: localSync,
  };

  if (localInfo?.chain_id != null && publicInfo?.chain_id != null && localInfo.chain_id !== publicInfo.chain_id) {
    report.ok = false;
    report.steps.chain_tip.reason = 'chain_id_mismatch';
    report.hints.push('Local and public chain_id differ — wrong network, genesis, or misconfigured BOING_CHAIN_ID on the node process.');
    console.error(JSON.stringify(report, null, 2));
    scheduleExit(4);
    return;
  }

  if (lag > maxLag) {
    report.ok = false;
    report.steps.chain_tip.reason = 'local_too_far_behind';
    report.hints.push(
      'See docs/VIBEMINER-INTEGRATION.md § Manual follow-ups — bootnodes, binary tag, P2P reachability.'
    );
    console.error(JSON.stringify(report, null, 2));
    scheduleExit(2);
    return;
  }

  if (localH === 0 && publicH > 100) {
    report.steps.chain_tip.warning =
      'Local tip is 0 while public is far ahead — likely not synced to public testnet yet (or isolated dev data dir).';
  } else if (localH === 0 && publicH === 0) {
    report.warnings.push(
      'Both local and public boing_chainHeight are 0 — tips match but this does not prove sync to a live public testnet (fresh genesis, idle endpoints, or wrong network). Verify with an explorer or npm run check-testnet-rpc from repo root (set BOING_RPC_URL if not using the default).'
    );
    report.steps.chain_tip.note =
      'Local and public both report height 0 — not evidence of healthy mesh sync; see warnings.';
  } else if (lag > okLag) {
    report.steps.chain_tip.note = `Within max lag (${maxLag}) but > ${okLag} blocks behind — may still be catching up.`;
  } else {
    report.steps.chain_tip.note = 'Local tip is close to public testnet.';
  }

  if (localInfo?.chain_id == null || publicInfo?.chain_id == null) {
    report.warnings.push(
      'boing_getNetworkInfo chain_id is null on at least one endpoint — set BOING_CHAIN_ID (and BOING_CHAIN_NAME) on the boing-node process for clearer verification (docs/RPC-API-SPEC.md).'
    );
  }

  const stream = report.ok ? console.log : console.error;
  stream(JSON.stringify(report, null, 2));
  scheduleExit(report.ok ? 0 : 3);
}

main().catch((e) => {
  console.error(JSON.stringify({ ok: false, message: e instanceof Error ? e.message : String(e) }, null, 2));
  scheduleExit(1);
});
