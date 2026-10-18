/** Native constant-product pool template bytecode hex by version (v1..v10; v10 = u128 amounts for 18-decimal tokens). */
export declare const NATIVE_CP_POOL_BYTECODE_HEX: Readonly<Record<1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10, `0x${string}`>>;
/** Custody router v1 (`boing pool router-deploy`), pulls via transfer_from, 1..=4 hops. */
export declare const NATIVE_DEX_CUSTODY_ROUTER_BYTECODE_HEX: `0x${string}`;
/** LP vault v2 for custody (v8/v9) pools: deposit -> add_liquidity -> shares; withdraw -> burn -> remove_liquidity_to. */
export declare const NATIVE_LP_VAULT_V2_BYTECODE_HEX: `0x${string}`;
//# sourceMappingURL=defaultNativeCpPoolBytecodeHex.d.ts.map