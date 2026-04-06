/**
 * Well-known **public Boing testnet** (chain id **6913**) identifiers.
 *
 * **Normative source of truth** for the canonical native CP pool address is
 * [RPC-API-SPEC.md](https://github.com/chiku524/boing.network/blob/main/docs/RPC-API-SPEC.md) § Native constant-product AMM
 * and [TESTNET.md](https://github.com/chiku524/boing.network/blob/main/docs/TESTNET.md) §5.3.
 * This constant is a **convenience mirror** for TypeScript apps and tutorials; it may lag a doc-only update — verify on docs if unsure.
 */
import { validateHex32 } from './hex.js';
/**
 * Canonical **v1** native constant-product pool `AccountId` on public Boing testnet (**6913**).
 * Deploy: CREATE2 + `NATIVE_CP_POOL_CREATE2_SALT_V1`, purpose `dapp` ([OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md](https://github.com/chiku524/boing.network/blob/main/docs/OPS-CANONICAL-TESTNET-NATIVE-AMM-POOL.md) § Published).
 */
export const CANONICAL_BOING_TESTNET_NATIVE_CP_POOL_HEX = validateHex32('0xffaa1290614441902ba813bf3bd8bf057624e0bd4f16160a9d32cd65d3f4d0c2');
