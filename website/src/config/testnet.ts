/**
 * Testnet configuration — single source for RPC URL, bootnodes, and client versions.
 * Override at build time with PUBLIC_TESTNET_RPC_URL and PUBLIC_BOOTNODES when ops changes endpoints.
 * Live chain source of truth remains `boing_getNetworkInfo` on the public RPC.
 */

const fromEnv = typeof import.meta !== 'undefined' && import.meta.env;
const env = fromEnv as Record<string, string | undefined> | undefined;

/** Public testnet chain id (decimal). Mainnet id 6914 is not live. */
export const TESTNET_CHAIN_ID = 6913 as const;

/** Public testnet RPC URL. Set PUBLIC_TESTNET_RPC_URL at build time to override. */
export const TESTNET_RPC_URL =
  (env && env.PUBLIC_TESTNET_RPC_URL) || 'https://testnet-rpc.boing.network/';

/**
 * Official testnet bootnode multiaddrs. Set PUBLIC_BOOTNODES (comma-separated)
 * at build time to override; otherwise this fallback is used.
 * Hosted: Fly `boing-testnet-1` only (`boing-testnet-2` / 109.105.220.118 retired 2026-10-24).
 */
export const BOOTNODES: string[] =
  env && env.PUBLIC_BOOTNODES
    ? env.PUBLIC_BOOTNODES.split(',').map((s) => s.trim()).filter(Boolean)
    : [
        '/ip4/169.155.48.188/tcp/4001', // Fly validator (boing-testnet-1)
      ];

/** Comma-joined bootnodes for `--bootnodes` copy-paste. */
export const BOOTNODES_CSV = BOOTNODES.join(',');

/** Whether the testnet is "live" (we have at least one bootnode and a non-local RPC). */
export const isTestnetLive =
  BOOTNODES.length > 0 &&
  !TESTNET_RPC_URL.includes('127.0.0.1') &&
  !TESTNET_RPC_URL.includes('localhost');

/**
 * Current public testnet node release tag (Windows / Linux / macOS zips on GitHub Releases).
 * Keep in sync with `/api/networks` meta.boing_testnet_download_tag and TESTNET-VALIDATOR-GUIDE.md.
 */
export const TESTNET_NODE_RELEASE = 'testnet-v0.1.24' as const;

/** Recommended Boing Express (wallet) version from live `client_hints.recommended_wallet_version`. */
export const RECOMMENDED_WALLET_VERSION = '0.1.24' as const;

/** Recommended `@boing-network/boing-sdk` from live `client_hints.recommended_sdk`. */
export const RECOMMENDED_SDK_VERSION = '0.5.38' as const;

/** Explorer / wallet / DEX product URLs for ecosystem cards. */
export const ECOSYSTEM = {
  express: 'https://boing.express',
  finance: 'https://boing.finance',
  observer: 'https://boing.observer',
  faucet: 'https://boing.network/faucet',
  discord: 'https://discord.gg/K3w9nbUw4N',
} as const;

/**
 * Whether hosted public testnet advertises native DEX ids on `boing_getNetworkInfo.end_user`
 * (chain **6913**). Verified against `https://testnet-rpc.boing.network/`.
 * Flip to `false` after a chain reset until ops re-publishes `BOING_CANONICAL_NATIVE_*`.
 * Live source of truth remains the RPC snapshot — embedded hexes below are fallbacks only.
 */
export const HOSTED_TESTNET_NATIVE_DEX_PUBLISHED = true;

/**
 * Canonical **native constant-product AMM** pool `AccountId` (public testnet, chain 6913).
 * Live source of truth: `boing_getNetworkInfo.end_user.canonical_native_cp_pool`.
 */
export const CANONICAL_NATIVE_CP_POOL_ACCOUNT_ID_HEX =
  '0x5b60d6f8514d1a8a40340fa354951d778b0821cd3ff7e534d0bea50f67d38976' as const;

/**
 * Native AMM **LP vault** + **LP share token** (full-stack deploy; also on live `end_user`).
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
