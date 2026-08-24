/**
 * Historical **public Boing testnet** (chain id **6913**) identifiers from the previous tunnel ledger.
 *
 * **Live source of truth** is **`boing_getNetworkInfo.end_user.canonical_native_cp_pool`** on
 * `https://testnet-rpc.boing.network/` (currently unset on the hosted Fly cluster).
 */
import { validateHex32 } from './hex.js';
/**
 * Last published native CP pool `AccountId` on the **previous** public testnet ledger (chain **6913**).
 * The hosted Fly cluster behind `https://testnet-rpc.boing.network` does **not** currently set
 * `boing_getNetworkInfo.end_user.canonical_native_cp_pool` — treat live RPC as source of truth.
 * Rotations: [OPS-FRESH-TESTNET-BOOTSTRAP.md](../../docs/OPS-FRESH-TESTNET-BOOTSTRAP.md).
 */
export const CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX = validateHex32('0x7247ddc3180fdc4d3fd1e716229bfa16bad334a07d28aa9fda9ad1bfa7bdacc3');
