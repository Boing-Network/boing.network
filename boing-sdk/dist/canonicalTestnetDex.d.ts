/**
 * Last published native DEX aux ids from the **previous** public testnet ledger (chain **6913**).
 * Hosted Fly RPC currently returns null `end_user.canonical_native_*` until ops re-runs
 * `npm run deploy-native-dex-full-stack` and sets `BOING_CANONICAL_NATIVE_*` on the nodes.
 *
 * **V1 ledger router** is optional (`BOING_AUX_INCLUDE_LEDGER_V1`).
 * `CANONICAL_BOING_TESTNET_NATIVE_DEX_LEDGER_ROUTER_V1_HEX` remains a legacy CREATE2 prediction id — do not assume it is deployed.
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