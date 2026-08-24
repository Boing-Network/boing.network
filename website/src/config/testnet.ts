/**
 * Testnet configuration — single source for RPC URL and bootnodes.
 * Override at build time with PUBLIC_TESTNET_RPC_URL and PUBLIC_BOOTNODES when ops changes endpoints.
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
        '/ip4/169.155.48.188/tcp/4001', // Fly validator (boing-testnet-1)
        '/ip4/109.105.220.118/tcp/4001', // Fly full node (boing-testnet-2)
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
  '0x7247ddc3180fdc4d3fd1e716229bfa16bad334a07d28aa9fda9ad1bfa7bdacc3' as const;

/**
 * Native AMM **LP vault** + **LP share token** (live public testnet full-stack deploy). Matches
 * `boing_getNetworkInfo.end_user` and `boing-sdk` `CANONICAL_BOING_TESTNET_NATIVE_*`.
 * See docs/NATIVE-DEX-OPERATOR-DEPLOYMENT-RECORD.md Appendix B.
 */
export const CANONICAL_NATIVE_AMM_LP_VAULT_ACCOUNT_ID_HEX =
  '0x937d09ee8e4dcc521c812566ad4930792e74ad004ecb3ae2cc73dc015813aa8d' as const;

export const CANONICAL_NATIVE_LP_SHARE_TOKEN_ACCOUNT_ID_HEX =
  '0x101201403f573e5b1d6d5c6b93d52d12c68957f4a228d5dad76e78c747044421' as const;
