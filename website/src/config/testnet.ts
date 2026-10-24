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
 * Production: set PUBLIC_BOOTNODES="/ip4/PRIMARY_IP/tcp/4001" (comma-separated if multiple).
 */
export const BOOTNODES: string[] =
  env && env.PUBLIC_BOOTNODES
    ? env.PUBLIC_BOOTNODES.split(',').map((s) => s.trim()).filter(Boolean)
    : [
        '/ip4/169.155.48.188/tcp/4001', // Fly validator (boing-testnet-1)
      ];

/** Whether the testnet is "live" (we have at least one bootnode and a non-local RPC). */
export const isTestnetLive =
  BOOTNODES.length > 0 &&
  !TESTNET_RPC_URL.includes('127.0.0.1') &&
  !TESTNET_RPC_URL.includes('localhost');

/**
 * Last-published native DEX `AccountId`s from the **previous** tunnel ledger (chain **6913**).
 * The hosted Fly testnet behind `https://testnet-rpc.boing.network/` currently returns
 * **null** `boing_getNetworkInfo.end_user.canonical_native_*` — do not treat these hexes as live.
 * Re-publish after `npm run deploy-native-dex-full-stack` + `BOING_CANONICAL_NATIVE_*` on the nodes.
 */
export const HOSTED_TESTNET_NATIVE_DEX_PUBLISHED = false;

/**
 * Historical canonical **native constant-product AMM** pool `AccountId` (previous public testnet).
 * Live source of truth: `boing_getNetworkInfo.end_user.canonical_native_cp_pool`.
 */
export const CANONICAL_NATIVE_CP_POOL_ACCOUNT_ID_HEX =
  '0x5b60d6f8514d1a8a40340fa354951d778b0821cd3ff7e534d0bea50f67d38976' as const;

/**
 * Historical native AMM **LP vault** + **LP share token** (previous full-stack deploy).
 * See docs/NATIVE-DEX-OPERATOR-DEPLOYMENT-RECORD.md Appendix B.
 */
export const CANONICAL_NATIVE_AMM_LP_VAULT_ACCOUNT_ID_HEX =
  '0xac7a635931b192f5cff054e9994242f54fbda579a46eba21d9787600971ec90f' as const;

export const CANONICAL_NATIVE_LP_SHARE_TOKEN_ACCOUNT_ID_HEX =
  '0x2f460046eb700ff8c7eebe6f4c409465a9111f32210edcc478a2a0de9d0ec2db' as const;

/**
 * Live **linked NFT ↔ fungible token** registry on public testnet (PR #42 / `924c0ba`).
 * CREATE2 salt `BOING_NFT_TOKEN_LINK_REG_V1`, selectors `0xE0`–`0xE6`.
 * See docs/BOING-LINKED-NFT-TOKEN.md.
 */
export const CANONICAL_LINKED_NFT_TOKEN_REGISTRY_ACCOUNT_ID_HEX =
  '0xebf9f0190f415852f90d0e60343126201248ab96273fdbf8acc5fe5fa03c3dd8' as const;

/**
 * Live **builder attestation** registry on public testnet (PR #49 / `61fa426`).
 * CREATE2 salt `BOING_BUILDER_ATTEST_REG_V1`, selectors `0xE0`–`0xE6`.
 * Builder signal only — companions remain project authority.
 * See docs/BOING-BUILDER-ATTESTATION.md.
 */
export const CANONICAL_BUILDER_ATTESTATION_REGISTRY_ACCOUNT_ID_HEX =
  '0x734077a9f926077d39e84ec278b77b7b600c840f7101809646d8797a6a9e1816' as const;
