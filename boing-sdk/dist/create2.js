/**
 * Predict CREATE2 contract `AccountId` (matches `boing_primitives::create2_contract_address`).
 */
import { blake3 } from '@noble/hashes/blake3';
import { concatBytes } from './bincode.js';
import { bytesToHex, hexToBytes, validateHex32 } from './hex.js';
/** Domain separator: `b"boing.create2.v1\\0"` in Rust. */
const CREATE2_DOMAIN = concatBytes(new TextEncoder().encode('boing.create2.v1'), new Uint8Array([0]));
/** Same bytes as `boing_execution::native_amm::NATIVE_CP_POOL_CREATE2_SALT_V1`. */
export const NATIVE_CP_POOL_CREATE2_SALT_V1 = (() => {
    const label = new TextEncoder().encode('BOING_NATIVECP_C2V1');
    const out = new Uint8Array(32);
    out.set(label);
    return out;
})();
/** Same bytes as `boing_execution::native_amm::NATIVE_CP_POOL_CREATE2_SALT_V2` (token-hook pool bytecode). */
export const NATIVE_CP_POOL_CREATE2_SALT_V2 = (() => {
    const label = new TextEncoder().encode('BOING_NATIVECP_C2V2');
    const out = new Uint8Array(32);
    out.set(label);
    return out;
})();
/** Same bytes as `boing_execution::native_amm::NATIVE_CP_POOL_CREATE2_SALT_V3` (ledger-only + configurable swap fee). */
export const NATIVE_CP_POOL_CREATE2_SALT_V3 = (() => {
    const label = new TextEncoder().encode('BOING_NATIVECP_C2V3');
    const out = new Uint8Array(32);
    out.set(label);
    return out;
})();
/** Same bytes as `boing_execution::native_amm::NATIVE_CP_POOL_CREATE2_SALT_V4` (v2 hooks + configurable fee). */
export const NATIVE_CP_POOL_CREATE2_SALT_V4 = (() => {
    const label = new TextEncoder().encode('BOING_NATIVECP_C2V4');
    const out = new Uint8Array(32);
    out.set(label);
    return out;
})();
/** Same bytes as `boing_execution::native_amm::NATIVE_CP_POOL_CREATE2_SALT_V5` (v4 + `swap_to` recipient). */
export const NATIVE_CP_POOL_CREATE2_SALT_V5 = (() => {
    const label = new TextEncoder().encode('BOING_NATIVECP_C2V5');
    const out = new Uint8Array(32);
    out.set(label);
    return out;
})();
/** Same bytes as `native_dex_factory::NATIVE_DEX_FACTORY_CREATE2_SALT_V1`. */
export const NATIVE_DEX_FACTORY_CREATE2_SALT_V1 = (() => {
    const label = new TextEncoder().encode('BOING_NATIVEDEX_FACTORY_V1');
    const out = new Uint8Array(32);
    out.set(label);
    return out;
})();
/** Same bytes as `native_dex_ledger_router::NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V1`. */
export const NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V1 = (() => {
    const label = new TextEncoder().encode('BOING_NATIVEDEX_LROUTER_V1');
    const out = new Uint8Array(32);
    out.set(label);
    return out;
})();
/** Same bytes as `native_dex_ledger_router::NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V2`. */
export const NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V2 = (() => {
    const label = new TextEncoder().encode('BOING_NATIVEDEX_LROUTER_V2');
    const out = new Uint8Array(32);
    out.set(label);
    return out;
})();
/** Same bytes as `native_dex_ledger_router::NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V3`. */
export const NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V3 = (() => {
    const label = new TextEncoder().encode('BOING_NATIVEDEX_LROUTER_V3');
    const out = new Uint8Array(32);
    out.set(label);
    return out;
})();
/** Same bytes as `native_dex_multihop_swap_router::NATIVE_DEX_MULTIHOP_SWAP_ROUTER_CREATE2_SALT_V1`. */
export const NATIVE_DEX_MULTIHOP_SWAP_ROUTER_CREATE2_SALT_V1 = (() => {
    const label = new TextEncoder().encode('BOING_NATIVEDEX_MHOP_V1');
    const out = new Uint8Array(32);
    out.set(label);
    return out;
})();
/**
 * @deprecated Same bytes as {@link NATIVE_DEX_MULTIHOP_SWAP_ROUTER_CREATE2_SALT_V1} (Rust
 * `NATIVE_DEX_SWAP2_ROUTER_CREATE2_SALT_V1` alias).
 */
export const NATIVE_DEX_SWAP2_ROUTER_CREATE2_SALT_V1 = NATIVE_DEX_MULTIHOP_SWAP_ROUTER_CREATE2_SALT_V1;
/** Same bytes as `native_lp_share_token::NATIVE_LP_SHARE_TOKEN_CREATE2_SALT_V1`. */
export const NATIVE_LP_SHARE_TOKEN_CREATE2_SALT_V1 = (() => {
    const label = new TextEncoder().encode('BOING_LP_SHARE_TOKEN_V1');
    const out = new Uint8Array(32);
    out.set(label);
    return out;
})();
/** Same bytes as `native_amm_lp_vault::NATIVE_AMM_LP_VAULT_CREATE2_SALT_V1`. */
export const NATIVE_AMM_LP_VAULT_CREATE2_SALT_V1 = (() => {
    const label = new TextEncoder().encode('BOING_AMM_LP_VAULT_V1');
    const out = new Uint8Array(32);
    out.set(label);
    return out;
})();
/** `0x` + 64 hex for {@link NATIVE_CP_POOL_CREATE2_SALT_V1}. */
export function nativeCpPoolCreate2SaltV1Hex() {
    return validateHex32(bytesToHex(NATIVE_CP_POOL_CREATE2_SALT_V1));
}
/** `0x` + 64 hex for {@link NATIVE_CP_POOL_CREATE2_SALT_V2}. */
export function nativeCpPoolCreate2SaltV2Hex() {
    return validateHex32(bytesToHex(NATIVE_CP_POOL_CREATE2_SALT_V2));
}
/** `0x` + 64 hex for {@link NATIVE_CP_POOL_CREATE2_SALT_V3}. */
export function nativeCpPoolCreate2SaltV3Hex() {
    return validateHex32(bytesToHex(NATIVE_CP_POOL_CREATE2_SALT_V3));
}
/** `0x` + 64 hex for {@link NATIVE_CP_POOL_CREATE2_SALT_V4}. */
export function nativeCpPoolCreate2SaltV4Hex() {
    return validateHex32(bytesToHex(NATIVE_CP_POOL_CREATE2_SALT_V4));
}
/** `0x` + 64 hex for {@link NATIVE_CP_POOL_CREATE2_SALT_V5}. */
export function nativeCpPoolCreate2SaltV5Hex() {
    return validateHex32(bytesToHex(NATIVE_CP_POOL_CREATE2_SALT_V5));
}
/**
 * `BLAKE3(domain || deployer_32 || salt_32 || BLAKE3(bytecode))`.
 */
export function predictCreate2ContractAddress(deployerHex, salt32, bytecode) {
    if (salt32.length !== 32) {
        throw new Error('create2 salt must be exactly 32 bytes');
    }
    const deployer = hexToBytes(validateHex32(deployerHex));
    const codeHash = blake3(bytecode);
    const preimage = concatBytes(CREATE2_DOMAIN, deployer, salt32, codeHash);
    return validateHex32(bytesToHex(blake3(preimage)));
}
/** Pool address for native CP bytecode + documented v1 salt (deployer = future deployer account). */
export function predictNativeCpPoolCreate2Address(deployerHex, poolBytecode) {
    return predictCreate2ContractAddress(deployerHex, NATIVE_CP_POOL_CREATE2_SALT_V1, poolBytecode);
}
/** **v2** pool (token `CALL` hooks) + documented v2 salt. */
export function predictNativeCpPoolV2Create2Address(deployerHex, poolBytecodeV2) {
    return predictCreate2ContractAddress(deployerHex, NATIVE_CP_POOL_CREATE2_SALT_V2, poolBytecodeV2);
}
/** **v3** pool (ledger-only + on-chain swap fee bps) + documented v3 salt. */
export function predictNativeCpPoolV3Create2Address(deployerHex, poolBytecodeV3) {
    return predictCreate2ContractAddress(deployerHex, NATIVE_CP_POOL_CREATE2_SALT_V3, poolBytecodeV3);
}
/** **v4** pool (v2 hooks + configurable fee) + documented v4 salt. */
export function predictNativeCpPoolV4Create2Address(deployerHex, poolBytecodeV4) {
    return predictCreate2ContractAddress(deployerHex, NATIVE_CP_POOL_CREATE2_SALT_V4, poolBytecodeV4);
}
/** **v5** pool (v4 + explicit swap output recipient) + documented v5 salt. */
export function predictNativeCpPoolV5Create2Address(deployerHex, poolBytecodeV5) {
    return predictCreate2ContractAddress(deployerHex, NATIVE_CP_POOL_CREATE2_SALT_V5, poolBytecodeV5);
}
/** Pair-directory contract (`native_dex_factory_bytecode`) + documented salt. */
export function predictNativeDexFactoryCreate2Address(deployerHex, factoryBytecode) {
    return predictCreate2ContractAddress(deployerHex, NATIVE_DEX_FACTORY_CREATE2_SALT_V1, factoryBytecode);
}
/** Ledger router (`native_dex_ledger_router_bytecode`) + documented salt. */
export function predictNativeDexLedgerRouterCreate2Address(deployerHex, routerBytecode) {
    return predictCreate2ContractAddress(deployerHex, NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V1, routerBytecode);
}
/** Ledger router v2 (`native_dex_ledger_router_bytecode_v2`) + documented salt. */
export function predictNativeDexLedgerRouterV2Create2Address(deployerHex, routerBytecodeV2) {
    return predictCreate2ContractAddress(deployerHex, NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V2, routerBytecodeV2);
}
/** Ledger router v3 (`native_dex_ledger_router_bytecode_v3`) + documented salt. */
export function predictNativeDexLedgerRouterV3Create2Address(deployerHex, routerBytecodeV3) {
    return predictCreate2ContractAddress(deployerHex, NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V3, routerBytecodeV3);
}
/** Multihop swap router (`native_dex_multihop_swap_router_bytecode`) + documented salt. */
export function predictNativeDexMultihopSwapRouterCreate2Address(deployerHex, routerBytecode) {
    return predictCreate2ContractAddress(deployerHex, NATIVE_DEX_MULTIHOP_SWAP_ROUTER_CREATE2_SALT_V1, routerBytecode);
}
/** @deprecated Use {@link predictNativeDexMultihopSwapRouterCreate2Address}. */
export function predictNativeDexSwap2RouterCreate2Address(deployerHex, swap2RouterBytecode) {
    return predictNativeDexMultihopSwapRouterCreate2Address(deployerHex, swap2RouterBytecode);
}
/** LP share token (`lp_share_token_bytecode`) + documented salt. */
export function predictNativeLpShareTokenCreate2Address(deployerHex, bytecode) {
    return predictCreate2ContractAddress(deployerHex, NATIVE_LP_SHARE_TOKEN_CREATE2_SALT_V1, bytecode);
}
/** Native AMM LP vault (`native_amm_lp_vault_bytecode`) + documented salt. */
export function predictNativeAmmLpVaultCreate2Address(deployerHex, bytecode) {
    return predictCreate2ContractAddress(deployerHex, NATIVE_AMM_LP_VAULT_CREATE2_SALT_V1, bytecode);
}
/** `0x` + 64 hex for {@link NATIVE_DEX_FACTORY_CREATE2_SALT_V1}. */
export function nativeDexFactoryCreate2SaltV1Hex() {
    return validateHex32(bytesToHex(NATIVE_DEX_FACTORY_CREATE2_SALT_V1));
}
/** `0x` + 64 hex for {@link NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V1}. */
export function nativeDexLedgerRouterCreate2SaltV1Hex() {
    return validateHex32(bytesToHex(NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V1));
}
/** `0x` + 64 hex for {@link NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V2}. */
export function nativeDexLedgerRouterCreate2SaltV2Hex() {
    return validateHex32(bytesToHex(NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V2));
}
/** `0x` + 64 hex for {@link NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V3}. */
export function nativeDexLedgerRouterCreate2SaltV3Hex() {
    return validateHex32(bytesToHex(NATIVE_DEX_LEDGER_ROUTER_CREATE2_SALT_V3));
}
/** `0x` + 64 hex for {@link NATIVE_DEX_MULTIHOP_SWAP_ROUTER_CREATE2_SALT_V1}. */
export function nativeDexMultihopSwapRouterCreate2SaltV1Hex() {
    return validateHex32(bytesToHex(NATIVE_DEX_MULTIHOP_SWAP_ROUTER_CREATE2_SALT_V1));
}
/** @deprecated Alias for {@link nativeDexMultihopSwapRouterCreate2SaltV1Hex}. */
export function nativeDexSwap2RouterCreate2SaltV1Hex() {
    return nativeDexMultihopSwapRouterCreate2SaltV1Hex();
}
/** `0x` + 64 hex for {@link NATIVE_LP_SHARE_TOKEN_CREATE2_SALT_V1}. */
export function nativeLpShareTokenCreate2SaltV1Hex() {
    return validateHex32(bytesToHex(NATIVE_LP_SHARE_TOKEN_CREATE2_SALT_V1));
}
/** `0x` + 64 hex for {@link NATIVE_AMM_LP_VAULT_CREATE2_SALT_V1}. */
export function nativeAmmLpVaultCreate2SaltV1Hex() {
    return validateHex32(bytesToHex(NATIVE_AMM_LP_VAULT_CREATE2_SALT_V1));
}
