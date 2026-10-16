/**
 * Native DEX aux ids on the current public testnet ledger (chain **6913**), verified against
 * `boing_getNetworkInfo.end_user.canonical_native_*` and the CREATE2 deploy txs (heights 2-8) on 2026-10-15.
 * Live RPC values always win; these apply only when network info is unavailable.
 *
 * **V1 ledger router** is optional (`BOING_AUX_INCLUDE_LEDGER_V1`).
 * `CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V1_HEX` is a legacy CREATE2 prediction id and is **not** deployed on the current ledger.
 */

import { validateHex32 } from './hex.js';

export const CANONICAL_BOING_TESTNET_NATIVE_DEX_DEPLOYER_HEX = validateHex32(
  '0x8079f30559c02c5fe087c9ad1bc3b751a29de35992f3f8431734243e825f696e',
);

export const CANONICAL_BOING_TESTNET_NATIVE_DEX_FACTORY_HEX = validateHex32(
  '0x7322ccec1ffdbf25d3f60384db0f9a38f8d6121233daccd48d4477d6036aff2a',
);

/** Legacy predicted v1 — not deployed on the current testnet-rpc full-stack bundle unless aux includes v1. */
export const CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V1_HEX = validateHex32(
  '0x371b4cd7e3b88e06e6b89bdc86214918a7e7ec73b62deb7f9975e4166736d54d',
);

export const CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V2_HEX = validateHex32(
  '0xd39d7bb18e986a6e39a69521d9b6f1fe403ff757ee8d409b89f2bcccc4df7980',
);

export const CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V3_HEX = validateHex32(
  '0x082a13685fbb69999f2d1378b2f704f3103ae6929b18a87510b513cb790c53c9',
);

export const CANONICAL_BOING_TESTNET_NATIVE_DEX_MULTIHOP_SWAP_ROUTER_HEX = validateHex32(
  '0xd8504729900a2d6dc8b8afc7bf24a18dc08732d24a4d2e3555ea5b7f28f3672c',
);

export const CANONICAL_BOING_TESTNET_NATIVE_AMM_LP_VAULT_HEX = validateHex32(
  '0xac7a635931b192f5cff054e9994242f54fbda579a46eba21d9787600971ec90f',
);

export const CANONICAL_BOING_TESTNET_NATIVE_LP_SHARE_TOKEN_HEX = validateHex32(
  '0x2f460046eb700ff8c7eebe6f4c409465a9111f32210edcc478a2a0de9d0ec2db',
);

/**
 * Public-testnet USDC / BOING-token pair (created 2026-10-15, block 94, by the deployer above).
 * Both tokens are reference fungible **v2** templates with a fixed 1e9 supply (whole units, no decimals);
 * the pool is a **v9** native CP pool that holds real token balances (`transfer_from` custody), seeded 100k : 100k
 * and listed in the canonical factory, so it is what `boing_listDexPools` returns.
 * The v1 ledger-only `canonical_native_cp_pool` stays the pool the ledger routers / LP vault are wired to.
 */
export const CANONICAL_BOING_TESTNET_USDC_TOKEN_HEX = validateHex32(
  '0x6676f971ade3982406efa3e2cdb0477923f8f2a15624c4b6ad3a432817a75e93',
);

export const CANONICAL_BOING_TESTNET_BOING_TOKEN_HEX = validateHex32(
  '0x6fce9d3e3f13df9fc17e3fd947148a4e6ce6966538b7dd9bf2a891d3b9699c6b',
);

/**
 * Display label for {@link CANONICAL_BOING_TESTNET_BOING_TOKEN_HEX}. It is a reference *contract* token, not the
 * native BOING coin, so UIs must never show it as plain "BOING".
 */
export const CANONICAL_BOING_TESTNET_BOING_TOKEN_SYMBOL = 'tBOING';
export const CANONICAL_BOING_TESTNET_BOING_TOKEN_NAME = 'Boing Token (test)';

export const CANONICAL_BOING_TESTNET_USDC_BOING_POOL_HEX = validateHex32(
  '0x2ad75c2e11b1e93761f4f8df39502deecbe7148324e5a80a951798142b2c5b61',
);

/**
 * Custody router v1 (`boing pool router-deploy`, block 97) for v8/v9 pools such as the USDC/BOING pool:
 * pulls the input with `transfer_from` and chains 1..=4 `swap_to` hops (see `routeBoingSwap`).
 * Stateless: the pool is passed per call. The ledger routers v2/v3 and multihop router above only drive
 * ledger-only (v1..v7) pools and cannot route the v9 pool.
 */
export const CANONICAL_BOING_TESTNET_CUSTODY_ROUTER_HEX = validateHex32(
  '0x806b2462b28f2f5772bbf5ae2628ad3a333bd2c5fa341144e0400ef5c2f1e4c6',
);

/**
 * LP vault v2 for the USDC/BOING v9 pool (`scripts/lp-vault-v2-testnet.mjs deploy`, blocks 101-102): deposit both
 * tokens -> add_liquidity -> vault shares (1:1 with LP); withdraw burns shares and the pool pays the user.
 * Configured once to (USDC/BOING pool, USDC, BOING). See `docs/NATIVE-LP-VAULT-V2.md` and `boingLpVaultV2.ts`.
 */
export const CANONICAL_BOING_TESTNET_LP_VAULT_V2_HEX = validateHex32(
  '0x236a851dd50faaa268e2e1b7ba95cfb9993773abf63a4c0d949183d8302e65c7',
);
