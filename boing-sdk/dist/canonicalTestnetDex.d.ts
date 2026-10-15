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
//# sourceMappingURL=canonicalTestnetDex.d.ts.map