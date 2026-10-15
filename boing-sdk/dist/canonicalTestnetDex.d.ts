/**
 * Native DEX aux ids on the current public testnet ledger (chain **6913**), verified against
 * `boing_getNetworkInfo.end_user.canonical_native_*` and the CREATE2 deploy txs (heights 2-8) on 2026-10-15.
 * Live RPC values always win; these apply only when network info is unavailable.
 *
 * **V1 ledger router** is optional (`BOING_AUX_INCLUDE_LEDGER_V1`).
 * `CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V1_HEX` is a legacy CREATE2 prediction id and is **not** deployed on the current ledger.
 */
export declare const CANONICAL_BOING_TESTNET_NATIVE_DEX_DEPLOYER_HEX: string;
export declare const CANONICAL_BOING_TESTNET_NATIVE_DEX_FACTORY_HEX: string;
/** Legacy predicted v1 — not deployed on the current testnet-rpc full-stack bundle unless aux includes v1. */
export declare const CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V1_HEX: string;
export declare const CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V2_HEX: string;
export declare const CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V3_HEX: string;
export declare const CANONICAL_BOING_TESTNET_NATIVE_DEX_MULTIHOP_SWAP_ROUTER_HEX: string;
export declare const CANONICAL_BOING_TESTNET_NATIVE_AMM_LP_VAULT_HEX: string;
export declare const CANONICAL_BOING_TESTNET_NATIVE_LP_SHARE_TOKEN_HEX: string;
/**
 * Public-testnet USDC / BOING-token pair (created 2026-10-15, block 94, by the deployer above).
 * Both tokens are reference fungible **v2** templates with a fixed 1e9 supply (whole units, no decimals);
 * the pool is a **v9** native CP pool that holds real token balances (`transfer_from` custody), seeded 100k : 100k
 * and listed in the canonical factory, so it is what `boing_listDexPools` returns.
 * The v1 ledger-only `canonical_native_cp_pool` stays the pool the ledger routers / LP vault are wired to.
 */
export declare const CANONICAL_BOING_TESTNET_USDC_TOKEN_HEX: string;
export declare const CANONICAL_BOING_TESTNET_BOING_TOKEN_HEX: string;
export declare const CANONICAL_BOING_TESTNET_USDC_BOING_POOL_HEX: string;
//# sourceMappingURL=canonicalTestnetDex.d.ts.map