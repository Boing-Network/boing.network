/**
 * Native constant-product pool — `contract_call` + access list for Boing Express / JSON-RPC.
 * Matches `Transaction::suggested_parallel_access_list` for `ContractCall` when only sender + pool touch state.
 *
 * Storage layout matches `boing_execution::native_amm`:
 * reserves (`reserve_a_key` / `reserve_b_key`), total LP (`total_lp_supply_key`), per-signer LP (`lp_balance_storage_key`);
 * **v3/v4:** `swap_fee_bps_key` (`k[31] == 0x07`).
 * Amounts are u128 BE in the **low 16 bytes** of each 32-byte word.
 */
import type { BoingClient } from './client.js';
import type { SimulateResult } from './types.js';
/** Optional accounts merged into native CP `contract_call` access lists (forward-compatible). */
export type NativePoolAccessListOptions = {
    /**
     * Extra 32-byte account ids (e.g. reference-token contracts). **Required** for **v2** pools on `swap` / `remove_liquidity` when token slots are set (pool `CALL`s those contracts).
     * Duplicates and ids equal to sender/pool are ignored; extras are appended in sorted hex order after signer + pool.
     */
    additionalAccountsHex32?: string[];
};
/** `read` and `write` both include signer + pool (parallel-scheduling minimum for pool-only bytecode). */
export declare function buildNativeConstantProductPoolAccessList(senderHex32: string, poolHex32: string, options?: NativePoolAccessListOptions): {
    read: string[];
    write: string[];
};
/** Params for `boing_sendTransaction` / Express `contract_call` with explicit access list. */
export declare function buildNativeConstantProductContractCallTx(senderHex32: string, poolHex32: string, calldataHex: string, options?: NativePoolAccessListOptions): {
    type: 'contract_call';
    contract: string;
    calldata: string;
    access_list: {
        read: string[];
        write: string[];
    };
};
/**
 * Widen `read`/`write` with `sim.suggested_access_list` (e.g. after `boing_simulateTransaction`).
 */
export declare function mergeNativePoolAccessListWithSimulation(senderHex32: string, poolHex32: string, sim: SimulateResult, options?: NativePoolAccessListOptions): {
    read: string[];
    write: string[];
};
/** `boing_getContractStorage` key for reserve A (`native_amm::reserve_a_key`). */
export declare const NATIVE_CONSTANT_PRODUCT_RESERVE_A_KEY_HEX: `0x${string}01`;
/** `boing_getContractStorage` key for reserve B (`native_amm::reserve_b_key`). */
export declare const NATIVE_CONSTANT_PRODUCT_RESERVE_B_KEY_HEX: `0x${string}02`;
/** Total LP supply key (`native_amm::total_lp_supply_key`, `k[31] == 0x03`). */
export declare const NATIVE_CONSTANT_PRODUCT_TOTAL_LP_KEY_HEX: `0x${string}03`;
/** **v2:** On-chain reference-token id for reserve side A (`token_a_key`, `k[31] == 0x04`). */
export declare const NATIVE_CONSTANT_PRODUCT_TOKEN_A_KEY_HEX: `0x${string}04`;
/** **v2:** Reference-token id for side B (`k[31] == 0x05`). */
export declare const NATIVE_CONSTANT_PRODUCT_TOKEN_B_KEY_HEX: `0x${string}05`;
/** **v2:** Non-zero after successful `set_tokens` (`k[31] == 0x06`). */
export declare const NATIVE_CONSTANT_PRODUCT_TOKENS_CONFIGURED_KEY_HEX: `0x${string}06`;
/** **v3/v4:** Swap fee bps on output (`swap_fee_bps_key`, `k[31] == 0x07`). **`0`** = unset until first `add_liquidity` (then defaults to **`NATIVE_CP_SWAP_FEE_BPS`** on-chain). */
export declare const NATIVE_CONSTANT_PRODUCT_SWAP_FEE_BPS_KEY_HEX: `0x${string}07`;
/**
 * `boing_getContractStorage` key for the caller's LP balance (`native_amm::lp_balance_storage_key`).
 */
export declare function nativeAmmLpBalanceStorageKeyHex(senderHex32: string): string;
/**
 * Decode **u128** from a 32-byte Boing contract storage word (`value` from `boing_getContractStorage`):
 * big-endian integer in the **low 16 bytes** (high 16 bytes ignored), matching reference-token amount words.
 */
export declare function decodeBoingStorageWordU128(valueHex: string): bigint;
/**
 * Decode successful native pool **`add_liquidity`** contract return data: **exactly 32 bytes** (64 hex
 * chars), **u128** LP minted in the **low 16 bytes**; high 16 bytes must be zero (canonical amount word).
 */
export declare function decodeNativeAmmAddLiquidityReturnLpMinted(returnDataHex: string): bigint;
/**
 * Decode native AMM `Log2` **data** (96 bytes): three u128 words (low 16 bytes of each 32-byte word), per `NATIVE-AMM-CALLDATA.md` § Logs.
 */
export declare function decodeNativeAmmLogDataU128Triple(dataHex: string): readonly [bigint, bigint, bigint];
/**
 * Read both in-ledger reserves for the MVP constant-product pool (`Promise.all` of two storage reads).
 */
export declare function fetchNativeConstantProductReserves(client: BoingClient, poolHex32: string): Promise<{
    reserveA: bigint;
    reserveB: bigint;
}>;
/** Single `boing_getContractStorage` read for **`total_lp_supply_key`**. */
export declare function fetchNativeConstantProductTotalLpSupply(client: BoingClient, poolHex32: string): Promise<bigint>;
/** **v3/v4:** Raw u128 at **`swap_fee_bps_key`**. Use **`0n`** → default fee **`NATIVE_CP_SWAP_FEE_BPS`** when quoting swaps. */
export declare function fetchNativeConstantProductSwapFeeBps(client: BoingClient, poolHex32: string): Promise<bigint>;
/** LP balance for **`signerHex32`** in **`poolHex32`** (XOR-derived storage key). */
export declare function fetchNativeAmmSignerLpBalance(client: BoingClient, poolHex32: string, signerHex32: string): Promise<bigint>;
export interface NativeConstantProductPoolSnapshot {
    reserveA: bigint;
    reserveB: bigint;
    totalLpSupply: bigint;
    /** Present only when **`options.signerHex32`** was set. */
    signerLpBalance?: bigint;
}
/**
 * One round-trip batch: reserves + total LP, and optionally the given signer's LP balance (**3** or **4** parallel storage reads).
 */
export declare function fetchNativeConstantProductPoolSnapshot(client: BoingClient, poolHex32: string, options?: {
    signerHex32?: string;
}): Promise<NativeConstantProductPoolSnapshot>;
//# sourceMappingURL=nativeAmmPool.d.ts.map