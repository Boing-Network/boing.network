/**
 * Native DEX pair directory — single Boing RPC surface (no external chains).
 *
 * Composes {@link fetchNativeDexIntegrationDefaults}, factory storage reads, and optional
 * **`register_pair`** log backfill into one snapshot; helpers resolve **`(tokenA, tokenB)` → pool**.
 */

import type { BoingClient } from './client.js';
import {
  fetchNativeDexFactoryRegisterLogs,
  fetchNativeDexIntegrationDefaults,
  mergeNativeDexIntegrationDefaults,
  type NativeDexIntegrationDefaults,
  type NativeDexIntegrationOverrides,
} from './dexIntegration.js';
import { validateHex32 } from './hex.js';
import {
  fetchNativeDexFactoryPairsCount,
  findNativeDexFactoryPoolByTokens,
  type FindNativeDexFactoryPoolOptions,
} from './nativeDexFactoryPool.js';
import type { NativeDexFactoryRegisterRpcParsed } from './nativeDexFactoryLogs.js';

/** Snapshot of operator hints + on-chain directory state (Boing RPC only). */
export type NativeDexDirectorySnapshot = {
  chainId: number | null;
  headHeight: number;
  defaults: NativeDexIntegrationDefaults;
  /** Factory storage **`pairs_count`** when factory address is known; otherwise **`null`**. */
  pairsCount: bigint | null;
  /**
   * Parsed **`register_pair`** logs when {@link FetchNativeDexDirectorySnapshotOptions.registerLogs} was set
   * and a factory address was resolved; otherwise **`null`** (not fetched).
   */
  registerLogs: NativeDexFactoryRegisterRpcParsed[] | null;
};

export type FetchNativeDexDirectorySnapshotOptions = {
  overrides?: NativeDexIntegrationOverrides;
  /**
   * Inclusive block range for **`boing_getLogs`** (**`register_pair`** on the factory).
   * **`toBlock`** defaults to the chain head from the same **`getNetworkInfo`** snapshot.
   */
  registerLogs?: { fromBlock: number; toBlock?: number };
};

function clampBlock(n: number, lo: number, hi: number): number {
  if (n < lo) return lo;
  if (n > hi) return hi;
  return n;
}

/** Canonical map key for an unordered token pair (lowercased **32-byte** hex ids). */
export function nativeDexPairKey(tokenAHex32: string, tokenBHex32: string): string {
  const a = validateHex32(tokenAHex32).toLowerCase();
  const b = validateHex32(tokenBHex32).toLowerCase();
  return a < b ? `${a}:${b}` : `${b}:${a}`;
}

/**
 * Build **`pairKey → poolHex`** from register logs. Later log entries win (on-chain re-register / ordering).
 */
export function buildNativeDexRegisterLogPoolIndex(
  logs: readonly NativeDexFactoryRegisterRpcParsed[]
): ReadonlyMap<string, `0x${string}`> {
  const m = new Map<string, `0x${string}`>();
  for (const row of logs) {
    const k = nativeDexPairKey(row.tokenAHex, row.tokenBHex);
    m.set(k, validateHex32(row.poolHex) as `0x${string}`);
  }
  return m;
}

/** Resolve pool for **`(tokenA, tokenB)`** in either order using a register-log index. */
export function pickNativeDexPoolFromRegisterLogs(
  logs: readonly NativeDexFactoryRegisterRpcParsed[],
  tokenAHex32: string,
  tokenBHex32: string
): `0x${string}` | null {
  const idx = buildNativeDexRegisterLogPoolIndex(logs);
  return idx.get(nativeDexPairKey(tokenAHex32, tokenBHex32)) ?? null;
}

/**
 * Fetch network hints, optional factory pair count, and optional **`register_pair`** logs — **Boing RPC only**.
 */
export async function fetchNativeDexDirectorySnapshot(
  client: BoingClient,
  options?: FetchNativeDexDirectorySnapshotOptions
): Promise<NativeDexDirectorySnapshot> {
  const info = await client.getNetworkInfo();
  const headHeight = info.head_height;
  const defaults = mergeNativeDexIntegrationDefaults(info, options?.overrides);
  const factoryHex = defaults.nativeDexFactoryAccountHex;

  let pairsCount: bigint | null = null;
  if (factoryHex != null) {
    pairsCount = await fetchNativeDexFactoryPairsCount(client, factoryHex);
  }

  let registerLogs: NativeDexFactoryRegisterRpcParsed[] | null = null;
  const rl = options?.registerLogs;
  if (rl != null && factoryHex != null) {
    const fromBlock = rl.fromBlock;
    const toBlock = clampBlock(rl.toBlock ?? headHeight, 0, headHeight);
    if (fromBlock <= toBlock) {
      registerLogs = await fetchNativeDexFactoryRegisterLogs(client, {
        factoryAccountHex: factoryHex,
        fromBlock,
        toBlock,
      });
    } else {
      registerLogs = [];
    }
  }

  return {
    chainId: info.chain_id ?? null,
    headHeight,
    defaults,
    pairsCount,
    registerLogs,
  };
}

export type ResolveNativeDexPoolForTokensResult = {
  poolHex: `0x${string}` | null;
  factoryHex: `0x${string}` | null;
  via: 'logs' | 'simulate' | 'none';
};

export type ResolveNativeDexPoolForTokensOptions =
  | {
      kind: 'logs';
      overrides?: NativeDexIntegrationOverrides;
      fromBlock: number;
      toBlock?: number;
    }
  | {
      kind: 'simulate';
      overrides?: NativeDexIntegrationOverrides;
      find: FindNativeDexFactoryPoolOptions;
    }
  | {
      kind: 'auto';
      overrides?: NativeDexIntegrationOverrides;
      fromBlock: number;
      toBlock?: number;
      find: FindNativeDexFactoryPoolOptions;
    };

/**
 * Resolve **`tokenA` / `tokenB` → pool** using logs and/or directory simulation (**Boing-only**).
 */
export async function resolveNativeDexPoolForTokens(
  client: BoingClient,
  tokenAHex32: string,
  tokenBHex32: string,
  options: ResolveNativeDexPoolForTokensOptions
): Promise<ResolveNativeDexPoolForTokensResult> {
  switch (options.kind) {
    case 'logs': {
      const snap = await fetchNativeDexDirectorySnapshot(client, {
        overrides: options.overrides,
        registerLogs: { fromBlock: options.fromBlock, toBlock: options.toBlock },
      });
      const fac = snap.defaults.nativeDexFactoryAccountHex;
      if (fac == null || snap.registerLogs == null) {
        return { poolHex: null, factoryHex: fac, via: 'none' };
      }
      const poolHex = pickNativeDexPoolFromRegisterLogs(snap.registerLogs, tokenAHex32, tokenBHex32);
      return { poolHex, factoryHex: fac, via: poolHex != null ? 'logs' : 'none' };
    }
    case 'simulate': {
      const d = await fetchNativeDexIntegrationDefaults(client, options.overrides);
      const fac = d.nativeDexFactoryAccountHex;
      if (fac == null) {
        return { poolHex: null, factoryHex: null, via: 'none' };
      }
      const poolHexRaw = await findNativeDexFactoryPoolByTokens(client, fac, tokenAHex32, tokenBHex32, options.find);
      const poolHex =
        poolHexRaw != null ? (validateHex32(poolHexRaw) as `0x${string}`) : null;
      return { poolHex, factoryHex: fac, via: poolHex != null ? 'simulate' : 'none' };
    }
    case 'auto': {
      const snap = await fetchNativeDexDirectorySnapshot(client, {
        overrides: options.overrides,
        registerLogs: { fromBlock: options.fromBlock, toBlock: options.toBlock },
      });
      const fac = snap.defaults.nativeDexFactoryAccountHex;
      if (fac == null) {
        return { poolHex: null, factoryHex: null, via: 'none' };
      }
      if (snap.registerLogs != null) {
        const fromLogs = pickNativeDexPoolFromRegisterLogs(snap.registerLogs, tokenAHex32, tokenBHex32);
        if (fromLogs != null) {
          return { poolHex: fromLogs, factoryHex: fac, via: 'logs' };
        }
      }
      const poolHexRaw = await findNativeDexFactoryPoolByTokens(client, fac, tokenAHex32, tokenBHex32, options.find);
      const poolHex =
        poolHexRaw != null ? (validateHex32(poolHexRaw) as `0x${string}`) : null;
      return { poolHex, factoryHex: fac, via: poolHex != null ? 'simulate' : 'none' };
    }
    default: {
      const _exhaustive: never = options;
      return _exhaustive;
    }
  }
}
