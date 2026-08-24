/**
 * One-call defaults for native Boing DEX wiring: merge **`boing_getNetworkInfo.end_user`**
 * hints with app overrides. Historical embedded 6913 ids are used only when network info
 * is omitted (offline). Live RPC nulls mean the hosted chain has not published those contracts.
 */

import type { BoingClient } from './client.js';
import { CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX } from './canonicalTestnet.js';
import {
  CANONICAL_BOING_TESTNET_NATIVE_AMM_LP_VAULT_HEX,
  CANONICAL_BOING_TESTNET_NATIVE_DEX_FACTORY_HEX,
  CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V2_HEX,
  CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V3_HEX,
  CANONICAL_BOING_TESTNET_NATIVE_DEX_MULTIHOP_SWAP_ROUTER_HEX,
  CANONICAL_BOING_TESTNET_NATIVE_LP_SHARE_TOKEN_HEX,
} from './canonicalTestnetDex.js';
import { validateHex32 } from './hex.js';
import { getLogsChunked } from './indexerBatch.js';
import { NATIVE_DEX_FACTORY_TOPIC_REGISTER_HEX } from './nativeDexFactory.js';
import {
  tryParseNativeDexFactoryRegisterRpcLogEntry,
  type NativeDexFactoryRegisterRpcParsed,
} from './nativeDexFactoryLogs.js';
import type { NetworkInfo } from './types.js';

function parseOptionalHex32(v: string | null | undefined): `0x${string}` | null {
  if (v == null || typeof v !== 'string') return null;
  const t = v.trim();
  if (!t) return null;
  try {
    return validateHex32(t) as `0x${string}`;
  } catch {
    return null;
  }
}

export type NativeDexDefaultSource = 'rpc_end_user' | 'sdk_testnet_embedded' | 'override' | 'none';

/** Resolved pool / factory / router / LP helper accounts for native DEX UIs and calldata builders. */
export type NativeDexIntegrationDefaults = {
  nativeCpPoolAccountHex: `0x${string}` | null;
  nativeDexFactoryAccountHex: `0x${string}` | null;
  poolSource: NativeDexDefaultSource;
  factorySource: NativeDexDefaultSource;
  nativeDexMultihopSwapRouterAccountHex: `0x${string}` | null;
  nativeDexMultihopSwapRouterSource: NativeDexDefaultSource;
  nativeDexLedgerRouterV2AccountHex: `0x${string}` | null;
  nativeDexLedgerRouterV2Source: NativeDexDefaultSource;
  nativeDexLedgerRouterV3AccountHex: `0x${string}` | null;
  nativeDexLedgerRouterV3Source: NativeDexDefaultSource;
  nativeAmmLpVaultAccountHex: `0x${string}` | null;
  nativeAmmLpVaultSource: NativeDexDefaultSource;
  nativeLpShareTokenAccountHex: `0x${string}` | null;
  nativeLpShareTokenSource: NativeDexDefaultSource;
  /** From `boing_getNetworkInfo.end_user.explorer_url` when set (https URL). */
  endUserExplorerUrl: string | null;
};

export type NativeDexIntegrationOverrides = {
  nativeCpPoolAccountHex?: string;
  nativeDexFactoryAccountHex?: string;
  nativeDexMultihopSwapRouterAccountHex?: string;
  nativeDexLedgerRouterV2AccountHex?: string;
  nativeDexLedgerRouterV3AccountHex?: string;
  nativeAmmLpVaultAccountHex?: string;
  nativeLpShareTokenAccountHex?: string;
};

function mergeOptionalAccountHex(
  override: string | undefined,
  rpcField: string | null | undefined,
  embeddedWhenOffline: `0x${string}` | null,
): { hex: `0x${string}` | null; source: NativeDexDefaultSource } {
  const o = override;
  if (o?.trim()) {
    try {
      return { hex: validateHex32(o) as `0x${string}`, source: 'override' };
    } catch {
      return { hex: null, source: 'none' };
    }
  }
  const rpc = parseOptionalHex32(rpcField ?? null);
  if (rpc) return { hex: rpc, source: 'rpc_end_user' };
  if (embeddedWhenOffline != null) {
    return { hex: embeddedWhenOffline, source: 'sdk_testnet_embedded' };
  }
  return { hex: null, source: 'none' };
}

function getProcessEnvRecord(): Record<string, string | undefined> | undefined {
  if (typeof globalThis === 'undefined') return undefined;
  const proc = (globalThis as unknown as { process?: { env?: Record<string, string | undefined> } }).process;
  return proc?.env;
}

function readFirstProcessEnv(keys: readonly string[]): string | undefined {
  const env = getProcessEnvRecord();
  if (env == null) return undefined;
  for (const k of keys) {
    const v = env[k];
    if (v != null && String(v).trim()) return String(v).trim();
  }
  return undefined;
}

/**
 * Build {@link NativeDexIntegrationOverrides} from **`process.env`** (Node / Vite / CRA).
 * First non-empty value wins per key group. Safe to call from browser bundles if env is injected at build time.
 */
export function buildNativeDexIntegrationOverridesFromProcessEnv(): NativeDexIntegrationOverrides {
  const o: NativeDexIntegrationOverrides = {};
  const pool = readFirstProcessEnv([
    'REACT_APP_BOING_NATIVE_AMM_POOL',
    'VITE_BOING_NATIVE_AMM_POOL',
    'BOING_NATIVE_AMM_POOL',
  ]);
  const fac = readFirstProcessEnv([
    'REACT_APP_BOING_NATIVE_VM_DEX_FACTORY',
    'VITE_BOING_NATIVE_VM_DEX_FACTORY',
    'BOING_NATIVE_VM_DEX_FACTORY',
    'BOING_DEX_FACTORY_HEX',
  ]);
  const hop = readFirstProcessEnv([
    'REACT_APP_BOING_NATIVE_VM_SWAP_ROUTER',
    'VITE_BOING_NATIVE_VM_SWAP_ROUTER',
    'BOING_NATIVE_VM_SWAP_ROUTER',
    'BOING_NATIVE_DEX_MULTIHOP_SWAP_ROUTER',
  ]);
  const l2 = readFirstProcessEnv([
    'REACT_APP_BOING_NATIVE_DEX_LEDGER_ROUTER_V2',
    'VITE_BOING_NATIVE_DEX_LEDGER_ROUTER_V2',
    'BOING_NATIVE_DEX_LEDGER_ROUTER_V2',
  ]);
  const l3 = readFirstProcessEnv([
    'REACT_APP_BOING_NATIVE_DEX_LEDGER_ROUTER_V3',
    'VITE_BOING_NATIVE_DEX_LEDGER_ROUTER_V3',
    'BOING_NATIVE_DEX_LEDGER_ROUTER_V3',
  ]);
  const vault = readFirstProcessEnv([
    'REACT_APP_BOING_NATIVE_AMM_LP_VAULT',
    'VITE_BOING_NATIVE_AMM_LP_VAULT',
    'BOING_NATIVE_AMM_LP_VAULT',
  ]);
  const share = readFirstProcessEnv([
    'REACT_APP_BOING_NATIVE_AMM_LP_SHARE_TOKEN',
    'VITE_BOING_NATIVE_AMM_LP_SHARE_TOKEN',
    'BOING_NATIVE_AMM_LP_SHARE_TOKEN',
  ]);
  if (pool) o.nativeCpPoolAccountHex = pool;
  if (fac) o.nativeDexFactoryAccountHex = fac;
  if (hop) o.nativeDexMultihopSwapRouterAccountHex = hop;
  if (l2) o.nativeDexLedgerRouterV2AccountHex = l2;
  if (l3) o.nativeDexLedgerRouterV3AccountHex = l3;
  if (vault) o.nativeAmmLpVaultAccountHex = vault;
  if (share) o.nativeLpShareTokenAccountHex = share;
  return o;
}

/**
 * Merge RPC **`end_user`** canonical addresses with optional app overrides.
 * Order per field: overrides → node hints. Embedded **6913** constants (see
 * [`canonicalTestnetDex.ts`](./canonicalTestnetDex.ts)) apply **only** when `info` is omitted
 * (offline / unit tests). A live `boing_getNetworkInfo` snapshot with null canonical fields
 * means the hosted chain has not published those contracts — do not substitute historical ids.
 */
export function mergeNativeDexIntegrationDefaults(
  info: NetworkInfo | null | undefined,
  overrides?: NativeDexIntegrationOverrides,
): NativeDexIntegrationDefaults {
  const eu = info?.end_user;
  const allowEmbedded = info == null;

  const poolEmb = allowEmbedded ? (CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX as `0x${string}`) : null;
  const facEmb = allowEmbedded ? (CANONICAL_BOING_TESTNET_NATIVE_DEX_FACTORY_HEX as `0x${string}`) : null;
  const hopEmb = allowEmbedded
    ? (CANONICAL_BOING_TESTNET_NATIVE_DEX_MULTIHOP_SWAP_ROUTER_HEX as `0x${string}`)
    : null;
  const l2Emb = allowEmbedded
    ? (CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V2_HEX as `0x${string}`)
    : null;
  const l3Emb = allowEmbedded
    ? (CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V3_HEX as `0x${string}`)
    : null;
  const vaultEmb = allowEmbedded
    ? (CANONICAL_BOING_TESTNET_NATIVE_AMM_LP_VAULT_HEX as `0x${string}`)
    : null;
  const shareEmb = allowEmbedded
    ? (CANONICAL_BOING_TESTNET_NATIVE_LP_SHARE_TOKEN_HEX as `0x${string}`)
    : null;

  const pool = mergeOptionalAccountHex(overrides?.nativeCpPoolAccountHex, eu?.canonical_native_cp_pool ?? null, poolEmb);
  const factory = mergeOptionalAccountHex(
    overrides?.nativeDexFactoryAccountHex,
    eu?.canonical_native_dex_factory ?? null,
    facEmb,
  );
  const multihop = mergeOptionalAccountHex(
    overrides?.nativeDexMultihopSwapRouterAccountHex,
    eu?.canonical_native_dex_multihop_swap_router ?? null,
    hopEmb,
  );
  const ledgerV2 = mergeOptionalAccountHex(
    overrides?.nativeDexLedgerRouterV2AccountHex,
    eu?.canonical_native_dex_ledger_router_v2 ?? null,
    l2Emb,
  );
  const ledgerV3 = mergeOptionalAccountHex(
    overrides?.nativeDexLedgerRouterV3AccountHex,
    eu?.canonical_native_dex_ledger_router_v3 ?? null,
    l3Emb,
  );
  const vault = mergeOptionalAccountHex(
    overrides?.nativeAmmLpVaultAccountHex,
    eu?.canonical_native_amm_lp_vault ?? null,
    vaultEmb,
  );
  const share = mergeOptionalAccountHex(
    overrides?.nativeLpShareTokenAccountHex,
    eu?.canonical_native_lp_share_token ?? null,
    shareEmb,
  );

  let endUserExplorerUrl: string | null = null;
  const ex = eu?.explorer_url;
  if (typeof ex === 'string') {
    const t = ex.trim();
    if (t && /^https?:\/\//i.test(t)) {
      endUserExplorerUrl = t.replace(/\/+$/, '');
    }
  }

  return {
    nativeCpPoolAccountHex: pool.hex,
    nativeDexFactoryAccountHex: factory.hex,
    poolSource: pool.source,
    factorySource: factory.source,
    nativeDexMultihopSwapRouterAccountHex: multihop.hex,
    nativeDexMultihopSwapRouterSource: multihop.source,
    nativeDexLedgerRouterV2AccountHex: ledgerV2.hex,
    nativeDexLedgerRouterV2Source: ledgerV2.source,
    nativeDexLedgerRouterV3AccountHex: ledgerV3.hex,
    nativeDexLedgerRouterV3Source: ledgerV3.source,
    nativeAmmLpVaultAccountHex: vault.hex,
    nativeAmmLpVaultSource: vault.source,
    nativeLpShareTokenAccountHex: share.hex,
    nativeLpShareTokenSource: share.source,
    endUserExplorerUrl,
  };
}

/** Fetch **`boing_getNetworkInfo`** and {@link mergeNativeDexIntegrationDefaults}. */
export async function fetchNativeDexIntegrationDefaults(
  client: BoingClient,
  overrides?: NativeDexIntegrationOverrides,
): Promise<NativeDexIntegrationDefaults> {
  const info = await client.getNetworkInfo();
  return mergeNativeDexIntegrationDefaults(info, overrides);
}

/**
 * Stream **`register_pair`** **`Log3`** rows for a factory (chunked **`boing_getLogs`**).
 * Requires a known factory **`AccountId`** (from {@link NativeDexIntegrationDefaults} or CREATE2 prediction).
 */
export async function fetchNativeDexFactoryRegisterLogs(
  client: BoingClient,
  opts: {
    factoryAccountHex: string;
    fromBlock: number;
    toBlock: number;
  },
): Promise<NativeDexFactoryRegisterRpcParsed[]> {
  const factoryAccountHex = validateHex32(opts.factoryAccountHex);
  const raw = await getLogsChunked(
    client,
    {
      fromBlock: opts.fromBlock,
      toBlock: opts.toBlock,
      address: factoryAccountHex,
      topics: [NATIVE_DEX_FACTORY_TOPIC_REGISTER_HEX, null, null],
    },
    {},
  );
  const out: NativeDexFactoryRegisterRpcParsed[] = [];
  for (const row of raw) {
    const p = tryParseNativeDexFactoryRegisterRpcLogEntry(row);
    if (p) out.push(p);
  }
  return out;
}
