/**
 * Predict CREATE2 contract `AccountId` (matches `boing_primitives::create2_contract_address`).
 */
/** Same bytes as `boing_execution::native_amm::NATIVE_CP_POOL_CREATE2_SALT_V1`. */
export declare const NATIVE_CP_POOL_CREATE2_SALT_V1: Uint8Array<ArrayBuffer>;
/** Same bytes as `boing_execution::native_amm::NATIVE_CP_POOL_CREATE2_SALT_V2` (token-hook pool bytecode). */
export declare const NATIVE_CP_POOL_CREATE2_SALT_V2: Uint8Array<ArrayBuffer>;
/** Same bytes as `boing_execution::native_amm::NATIVE_CP_POOL_CREATE2_SALT_V3` (ledger-only + configurable swap fee). */
export declare const NATIVE_CP_POOL_CREATE2_SALT_V3: Uint8Array<ArrayBuffer>;
/** Same bytes as `boing_execution::native_amm::NATIVE_CP_POOL_CREATE2_SALT_V4` (v2 hooks + configurable fee). */
export declare const NATIVE_CP_POOL_CREATE2_SALT_V4: Uint8Array<ArrayBuffer>;
/** Same bytes as `boing_execution::native_amm::NATIVE_CP_POOL_CREATE2_SALT_V5` (v4 + `swap_to` recipient). */
export declare const NATIVE_CP_POOL_CREATE2_SALT_V5: Uint8Array<ArrayBuffer>;
/** Same bytes as `native_dex_factory::NATIVE_DEX_FACTORY_CREATE2_SALT_V1`. */
export declare const NATIVE_DEX_FACTORY_CREATE2_SALT_V1: Uint8Array<ArrayBuffer>;
/** Same bytes as `native_dex_ledger_router::NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V1`. */
export declare const NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V1: Uint8Array<ArrayBuffer>;
/** Same bytes as `native_dex_ledger_router::NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V2`. */
export declare const NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V2: Uint8Array<ArrayBuffer>;
/** Same bytes as `native_dex_ledger_router::NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V3`. */
export declare const NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V3: Uint8Array<ArrayBuffer>;
/** Same bytes as `native_dex_multihop_swap_router::NATIVE_DEX_MULTIHOP_SWAP_ROUTER_CREATE2_SALT_V1`. */
export declare const NATIVE_DEX_MULTIHOP_SWAP_ROUTER_CREATE2_SALT_V1: Uint8Array<ArrayBuffer>;
/**
 * @deprecated Same bytes as {@link NATIVE_DEX_MULTIHOP_SWAP_ROUTER_CREATE2_SALT_V1} (Rust
 * `NATIVE_DEX_SWAP2_ROUTER_CREATE2_SALT_V1` alias).
 */
export declare const NATIVE_DEX_SWAP2_ROUTER_CREATE2_SALT_V1: Uint8Array<ArrayBuffer>;
/** Same bytes as `native_lp_share_token::NATIVE_LP_SHARE_TOKEN_CREATE2_SALT_V1`. */
export declare const NATIVE_LP_SHARE_TOKEN_CREATE2_SALT_V1: Uint8Array<ArrayBuffer>;
/** Same bytes as `native_amm_lp_vault::NATIVE_AMM_LP_VAULT_CREATE2_SALT_V1`. */
export declare const NATIVE_AMM_LP_VAULT_CREATE2_SALT_V1: Uint8Array<ArrayBuffer>;
/** `0x` + 64 hex for {@link NATIVE_CP_POOL_CREATE2_SALT_V1}. */
export declare function nativeCpPoolCreate2SaltV1Hex(): string;
/** `0x` + 64 hex for {@link NATIVE_CP_POOL_CREATE2_SALT_V2}. */
export declare function nativeCpPoolCreate2SaltV2Hex(): string;
/** `0x` + 64 hex for {@link NATIVE_CP_POOL_CREATE2_SALT_V3}. */
export declare function nativeCpPoolCreate2SaltV3Hex(): string;
/** `0x` + 64 hex for {@link NATIVE_CP_POOL_CREATE2_SALT_V4}. */
export declare function nativeCpPoolCreate2SaltV4Hex(): string;
/** `0x` + 64 hex for {@link NATIVE_CP_POOL_CREATE2_SALT_V5}. */
export declare function nativeCpPoolCreate2SaltV5Hex(): string;
/**
 * `BLAKE3(domain || deployer_32 || salt_32 || BLAKE3(bytecode))`.
 */
export declare function predictCreate2ContractAddress(deployerHex: string, salt32: Uint8Array, bytecode: Uint8Array): string;
/** Pool address for native CP bytecode + documented v1 salt (deployer = future deployer account). */
export declare function predictNativeCpPoolCreate2Address(deployerHex: string, poolBytecode: Uint8Array): string;
/** **v2** pool (token `CALL` hooks) + documented v2 salt. */
export declare function predictNativeCpPoolV2Create2Address(deployerHex: string, poolBytecodeV2: Uint8Array): string;
/** **v3** pool (ledger-only + on-chain swap fee bps) + documented v3 salt. */
export declare function predictNativeCpPoolV3Create2Address(deployerHex: string, poolBytecodeV3: Uint8Array): string;
/** **v4** pool (v2 hooks + configurable fee) + documented v4 salt. */
export declare function predictNativeCpPoolV4Create2Address(deployerHex: string, poolBytecodeV4: Uint8Array): string;
/** **v5** pool (v4 + explicit swap output recipient) + documented v5 salt. */
export declare function predictNativeCpPoolV5Create2Address(deployerHex: string, poolBytecodeV5: Uint8Array): string;
/** Pair-directory contract (`native_dex_factory_bytecode`) + documented salt. */
export declare function predictNativeDexFactoryCreate2Address(deployerHex: string, factoryBytecode: Uint8Array): string;
/** Ledger router (`native_dex_ledger_router_bytecode`) + documented salt. */
export declare function predictNativeDexLedgerRouterCreate2Address(deployerHex: string, routerBytecode: Uint8Array): string;
/** Ledger router v2 (`native_dex_ledger_router_bytecode_v2`) + documented salt. */
export declare function predictNativeDexLedgerRouterV2Create2Address(deployerHex: string, routerBytecodeV2: Uint8Array): string;
/** Ledger router v3 (`native_dex_ledger_router_bytecode_v3`) + documented salt. */
export declare function predictNativeDexLedgerRouterV3Create2Address(deployerHex: string, routerBytecodeV3: Uint8Array): string;
/** Multihop swap router (`native_dex_multihop_swap_router_bytecode`) + documented salt. */
export declare function predictNativeDexMultihopSwapRouterCreate2Address(deployerHex: string, routerBytecode: Uint8Array): string;
/** @deprecated Use {@link predictNativeDexMultihopSwapRouterCreate2Address}. */
export declare function predictNativeDexSwap2RouterCreate2Address(deployerHex: string, swap2RouterBytecode: Uint8Array): string;
/** LP share token (`lp_share_token_bytecode`) + documented salt. */
export declare function predictNativeLpShareTokenCreate2Address(deployerHex: string, bytecode: Uint8Array): string;
/** Native AMM LP vault (`native_amm_lp_vault_bytecode`) + documented salt. */
export declare function predictNativeAmmLpVaultCreate2Address(deployerHex: string, bytecode: Uint8Array): string;
/** `0x` + 64 hex for {@link NATIVE_DEX_FACTORY_CREATE2_SALT_V1}. */
export declare function nativeDexFactoryCreate2SaltV1Hex(): string;
/** `0x` + 64 hex for {@link NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V1}. */
export declare function nativeDexLedgerRouterCreate2SaltV1Hex(): string;
/** `0x` + 64 hex for {@link NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V2}. */
export declare function nativeDexLedgerRouterCreate2SaltV2Hex(): string;
/** `0x` + 64 hex for {@link NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V3}. */
export declare function nativeDexLedgerRouterCreate2SaltV3Hex(): string;
/** `0x` + 64 hex for {@link NATIVE_DEX_MULTIHOP_SWAP_ROUTER_CREATE2_SALT_V1}. */
export declare function nativeDexMultihopSwapRouterCreate2SaltV1Hex(): string;
/** @deprecated Alias for {@link nativeDexMultihopSwapRouterCreate2SaltV1Hex}. */
export declare function nativeDexSwap2RouterCreate2SaltV1Hex(): string;
/** `0x` + 64 hex for {@link NATIVE_LP_SHARE_TOKEN_CREATE2_SALT_V1}. */
export declare function nativeLpShareTokenCreate2SaltV1Hex(): string;
/** `0x` + 64 hex for {@link NATIVE_AMM_LP_VAULT_CREATE2_SALT_V1}. */
export declare function nativeAmmLpVaultCreate2SaltV1Hex(): string;
//# sourceMappingURL=create2.d.ts.map