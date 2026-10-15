/**
 * Embedded **public Boing testnet** (chain id **6913**) identifiers, used only when live network info is unavailable.
 *
 * **Live source of truth** is **`boing_getNetworkInfo.end_user.canonical_native_cp_pool`** on
 * `https://testnet-rpc.boing.network/`. Values below were verified against that RPC on 2026-10-15
 * (CREATE2 deploys at heights 2-8 by deployer `0x8079f30559c02c5fe087c9ad1bc3b751a29de35992f3f8431734243e825f696e`).
 */

import { validateHex32 } from './hex.js';

/**
 * Native CP pool `AccountId` on the current public testnet ledger (chain **6913**), matching
 * `boing_getNetworkInfo.end_user.canonical_native_cp_pool` as of 2026-10-15.
 * Rotations: [OPS-FRESH-TESTNET-BOOTSTRAP.md](../../docs/OPS-FRESH-TESTNET-BOOTSTRAP.md).
 */
export const CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX = validateHex32(
  '0x5b60d6f8514d1a8a40340fa354951d778b0821cd3ff7e534d0bea50f67d38976',
);
