/**
 * Chain identifiers for **native Boing** L1 — use in multi-network dApps (deploy wizards, network pickers).
 *
 * Prefer **`boing_getNetworkInfo`**.`chain_id` when connected to a live RPC; use these constants when
 * branching on **`eth_chainId`** / **`boing_chainId`** from the wallet.
 */
/** Public testnet chain id (decimal), per docs ([TESTNET.md](../../docs/TESTNET.md)). */
export declare const BOING_TESTNET_CHAIN_ID_DECIMAL = 6913;
/** Same id as EIP-155-style `0x`-prefixed hex (lowercase). */
export declare const BOING_TESTNET_CHAIN_ID_HEX: "0x1b01";
/**
 * Normalize wallet/RPC chain id to lowercase `0x` hex (e.g. **`0x1b01`**).
 * Accepts decimal string, `0x` hex, or bigint / number.
 */
export declare function normalizeBoingChainIdHex(chainId: string | number | bigint): `0x${string}`;
/** True when **`chainId`** is **Boing testnet** (6913 / `0x1b01`), after normalization. */
export declare function isBoingTestnetChainId(chainId: string | number | bigint): boolean;
//# sourceMappingURL=chainIds.d.ts.map