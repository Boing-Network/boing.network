#!/usr/bin/env node
/**
 * Audit public (or local) JSON-RPC against **canonical testnet** native DEX expectations.
 *
 * - `boing_chainHeight`, optional `boing_getNetworkInfo`
 * - Canonical CP pool reserve A (`boing_getContractStorage`) — strong signal the pool is live
 * - Predicted pair-directory address: scan **`boing_getLogs`** for `register_pair` **topic0** in block ranges
 *
 * Env:
 *   BOING_RPC_URL — default `https://testnet-rpc.boing.network/`
 *   BOING_AUDIT_MAX_BLOCKS — optional max block index (inclusive) for log scan from genesis; omit = scan through `chainHeight`
 *   BOING_AUDIT_STRICT_POOL — if `1`, exit 1 when pool reserve A is zero
 *
 * Predicted CREATE2 addresses: `scripts/canonical-testnet-dex-predicted.json`
 * Regenerate JSON: `cargo run -p boing-execution --example print_native_create2_manifest -- <DEPLOYER_HEX>`
 */
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const predictedPath = join(root, 'scripts', 'canonical-testnet-dex-predicted.json');
const predicted = JSON.parse(readFileSync(predictedPath, 'utf8'));
delete predicted._comment;

const rpcBase = (process.env.BOING_RPC_URL ?? 'https://testnet-rpc.boing.network/').replace(/\/$/, '');
/** Max block index (inclusive) to scan from genesis for factory logs; `0` or unset uses full `chainHeight`. */
const maxScanBlockInclusive = (() => {
  const raw = process.env.BOING_AUDIT_MAX_BLOCKS;
  if (raw === undefined || raw === '') return null;
  const n = parseInt(raw, 10);
  return Number.isFinite(n) && n >= 0 ? n : null;
})();
const strictPool = process.env.BOING_AUDIT_STRICT_POOL === '1' || process.env.BOING_AUDIT_STRICT_POOL === 'true';

const CANONICAL_POOL =
  predicted.native_cp_pool_v1 ??
  '0xffaa1290614441902ba813bf3bd8bf057624e0bd4f16160a9d32cd65d3f4d0c2';
const RESERVE_A_KEY = `0x${'00'.repeat(31)}01`;

/** Same construction as `boing-sdk` `NATIVE_DEX_FACTORY_TOPIC_REGISTER_HEX`. */
function nativeDexFactoryTopic0Hex() {
  const u = new Uint8Array(32);
  u.set(new TextEncoder().encode('BOING_NATIVE_DEX_FACTORY_REG1'));
  return `0x${Array.from(u, (b) => b.toString(16).padStart(2, '0')).join('')}`;
}

async function rpcCall(method, params) {
  const res = await fetch(rpcBase, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Accept: 'application/json',
      'User-Agent': 'boing.network-audit-native-dex-testnet/1.0',
    },
    body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
  });
  const text = await res.text();
  let j;
  try {
    j = JSON.parse(text);
  } catch {
    throw new Error(`RPC returned non-JSON (HTTP ${res.status}): ${text.slice(0, 400)}`);
  }
  if (j.error) {
    return { ok: false, error: j.error };
  }
  return { ok: true, result: j.result };
}

const MAX_LOG_SPAN = 128;

async function getLogsChunk(filter) {
  const r = await rpcCall('boing_getLogs', [filter]);
  if (!r.ok) return { ok: false, error: r.error, logs: [] };
  return { ok: true, logs: Array.isArray(r.result) ? r.result : [] };
}

async function countFactoryRegisterLogs(factoryHex, toBlock) {
  const topic0 = nativeDexFactoryTopic0Hex();
  let total = 0;
  let fromB = 0;
  while (fromB <= toBlock) {
    const span = Math.min(MAX_LOG_SPAN - 1, toBlock - fromB + 1, MAX_LOG_SPAN);
    const chunkTo = Math.min(fromB + span - 1, toBlock);
    const r = await getLogsChunk({
      fromBlock: fromB,
      toBlock: chunkTo,
      address: factoryHex,
      topics: [topic0],
    });
    if (!r.ok) {
      return { ok: false, error: r.error, count: total };
    }
    total += r.logs.length;
    fromB = chunkTo + 1;
    if (fromB > toBlock) break;
  }
  return { ok: true, count: total };
}

async function main() {
  const topic0 = nativeDexFactoryTopic0Hex();

  const h = await rpcCall('boing_chainHeight', []);
  const ni = await rpcCall('boing_getNetworkInfo', []);

  const poolSt = await rpcCall('boing_getContractStorage', [CANONICAL_POOL, RESERVE_A_KEY]);

  let reserveNonZero = false;
  let poolStorageError = null;
  if (poolSt.ok && poolSt.result?.value && typeof poolSt.result.value === 'string') {
    const v = poolSt.result.value.replace(/^0x/i, '');
    reserveNonZero = !/^0+$/.test(v);
  } else if (!poolSt.ok) {
    poolStorageError = poolSt.error;
  }

  const head = h.ok && typeof h.result === 'number' ? h.result : 0;
  const scanTo =
    maxScanBlockInclusive === null ? head : Math.min(head, maxScanBlockInclusive);

  let factoryLogProbe = { ok: false, count: 0, error: null, note: null };
  if (head >= 0 && predicted.native_dex_factory) {
    if (scanTo < 0) {
      factoryLogProbe = { ok: true, count: 0, error: null, note: 'no_blocks_to_scan' };
    } else {
      factoryLogProbe = await countFactoryRegisterLogs(predicted.native_dex_factory, scanTo);
      if (!factoryLogProbe.ok) {
        factoryLogProbe = {
          ok: false,
          count: 0,
          error: factoryLogProbe.error,
          note: 'getLogs_failed',
        };
      } else {
        factoryLogProbe = {
          ok: true,
          count: factoryLogProbe.count,
          error: null,
          note:
            factoryLogProbe.count > 0
              ? 'register_pair_logs_seen'
              : 'no_register_pair_logs_in_range (directory may be undeployed or empty)',
        };
      }
    }
  }

  const out = {
    ok: true,
    rpc: rpcBase,
    why_gaps_occur: {
      summary:
        'Aux DEX contracts (factory, routers, vault, LP share) are optional product deploys: ops must submit matching CREATE2 deploy txs. Until then, addresses are predictions only. Public RPC edges may omit boing_getNetworkInfo (proxy allowlist or older node), so canonical hints must come from embedded SDK constants and/or node env BOING_CANONICAL_NATIVE_*.',
      doc: 'docs/OPS-CANONICAL-TESTNET-NATIVE-DEX-AUX.md',
    },
    probes: {
      boing_chainHeight: h.ok ? { ok: true, height: h.result } : { ok: false, error: h.error },
      boing_getNetworkInfo: ni.ok
        ? {
            ok: true,
            has_end_user_factory_hint: Boolean(ni.result?.end_user?.canonical_native_dex_factory),
            end_user_aux_hints: {
              multihop_router: Boolean(ni.result?.end_user?.canonical_native_dex_multihop_swap_router),
              ledger_router_v2: Boolean(ni.result?.end_user?.canonical_native_dex_ledger_router_v2),
              ledger_router_v3: Boolean(ni.result?.end_user?.canonical_native_dex_ledger_router_v3),
              lp_vault: Boolean(ni.result?.end_user?.canonical_native_amm_lp_vault),
              lp_share_token: Boolean(ni.result?.end_user?.canonical_native_lp_share_token),
            },
          }
        : { ok: false, error: ni.error },
      canonical_pool: {
        account: CANONICAL_POOL,
        reserve_a_key: RESERVE_A_KEY,
        storage_ok: poolSt.ok,
        reserve_a_nonzero: reserveNonZero,
        error: poolStorageError,
      },
      predicted_create2: predicted,
      factory_register_logs: {
        factory: predicted.native_dex_factory,
        topic0,
        scanned_blocks_inclusive: [0, scanTo],
        ...factoryLogProbe,
      },
    },
  };

  console.log(JSON.stringify(out, null, 2));

  if (strictPool && !reserveNonZero) {
    console.error('BOING_AUDIT_STRICT_POOL: canonical pool reserve A is zero or storage read failed.');
    process.exit(1);
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
