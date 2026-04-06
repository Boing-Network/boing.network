/**
 * Testnet configuration — single source for RPC URL and bootnodes.
 * Update before incentivized testnet launch. For production deploy, set
 * PUBLIC_TESTNET_RPC_URL (and optionally PUBLIC_BOOTNODES as comma-separated multiaddrs)
 * in your build environment so the faucet and testnet pages show the live URLs.
 */

const fromEnv = typeof import.meta !== 'undefined' && import.meta.env;
const env = fromEnv as Record<string, string | undefined> | undefined;

/** Public testnet RPC URL. Set PUBLIC_TESTNET_RPC_URL at build time to override. */
export const TESTNET_RPC_URL =
  (env && env.PUBLIC_TESTNET_RPC_URL) || 'https://testnet-rpc.boing.network/';

/**
 * Official testnet bootnode multiaddrs. Set PUBLIC_BOOTNODES (comma-separated)
 * at build time to override; otherwise this fallback is used.
 * Production: set PUBLIC_BOOTNODES="/ip4/PRIMARY_IP/tcp/4001,/ip4/SECONDARY_IP/tcp/4001"
 */
export const BOOTNODES: string[] =
  env && env.PUBLIC_BOOTNODES
    ? env.PUBLIC_BOOTNODES.split(',').map((s) => s.trim()).filter(Boolean)
    : [
        '/ip4/73.84.106.121/tcp/4001', // Primary (faucet + RPC via testnet-rpc.boing.network)
        '/ip4/73.84.106.121/tcp/4001', // Secondary bootnode
      ];

/** Whether the testnet is "live" (we have at least one bootnode and a non-local RPC). */
export const isTestnetLive =
  BOOTNODES.length > 0 &&
  !TESTNET_RPC_URL.includes('127.0.0.1') &&
  !TESTNET_RPC_URL.includes('localhost');

/**
 * Canonical **native constant-product AMM** pool `AccountId` on public testnet (chain **6913**).
 * Keep in sync with [docs/RPC-API-SPEC.md](../../../docs/RPC-API-SPEC.md) § Native constant-product AMM
 * and `boing-sdk` **`CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX`**. **boing.finance** (separate app) should use the same hex in its env / `contracts.js` — not generated from this file.
 */
export const CANONICAL_NATIVE_CP_POOL_ACCOUNT_ID_HEX =
  '0xffaa1290614441902ba813bf3bd8bf057624e0bd4f16160a9d32cd65d3f4d0c2' as const;
