/**
 * Pair directory contract — access lists, `contract_call` payloads, storage reads.
 * Matches `boing_execution::native_dex_factory` + `native_dex_factory_count_key`.
 */
import type { BoingClient } from './client.js';
import type { SimulateResult } from './types.js';
import { type Ed25519SecretKey32 } from './transactionBuilder.js';
/**
 * `boing_getContractStorage` key for pair count (`native_dex_factory::native_dex_factory_count_key`).
 * 16 zero bytes, **`BOINGDEX`** (8 bytes), 4 zero bytes, **`0xffffffff`** (big-endian).
 */
export declare const NATIVE_DEX_FACTORY_COUNT_KEY_HEX: `0x${string}424f494e4744455800000000ffffffff`;
/** `read` / `write`: signer + directory contract (parallel scheduling minimum). */
export declare function buildNativeDexFactoryAccessList(senderHex32: string, factoryHex32: string): {
    read: string[];
    write: string[];
};
export declare function buildNativeDexFactoryContractCallTx(senderHex32: string, factoryHex32: string, calldataHex: string): {
    type: 'contract_call';
    contract: string;
    calldata: string;
    access_list: {
        read: string[];
        write: string[];
    };
};
export declare function mergeNativeDexFactoryAccessListWithSimulation(senderHex32: string, factoryHex32: string, sim: SimulateResult): {
    read: string[];
    write: string[];
};
/** Decode `pairs_count` return data (32-byte word); count is in the low **8** bytes (matches on-chain layout). */
export declare function decodeNativeDexFactoryPairsCountReturnData(returnDataHex: string): bigint;
/**
 * Decode `get_pair_at` return data (**96** bytes = three account words).
 */
export declare function decodeNativeDexFactoryGetPairAtReturnData(returnDataHex: string): {
    tokenAHex: string;
    tokenBHex: string;
    poolHex: string;
};
/** Read registered pair count from contract storage (no simulation). */
export declare function fetchNativeDexFactoryPairsCount(client: BoingClient, factoryHex32: string): Promise<bigint>;
export type FindNativeDexFactoryPoolOptions = {
    secretKey32: Ed25519SecretKey32;
    senderHex: string;
    /**
     * Max directory indices to simulate (each index = one `boing_simulateTransaction`).
     * Defaults to **`4096`** (on-chain max). Lower this on slow RPCs.
     */
    maxPairsToScan?: number;
};
/**
 * Find a registered pool address for **`(tokenA, tokenB)`** in either order.
 *
 * There is no on-chain reverse map; this helper reads **`pairs_count`** from storage, then simulates
 * **`get_pair_at(i)`** for each index until a matching triplet is found.
 *
 * Requires a signer because **`boing_simulateTransaction`** expects a signed tx envelope (same as submit).
 */
export declare function findNativeDexFactoryPoolByTokens(client: BoingClient, factoryHex32: string, tokenAHex32: string, tokenBHex32: string, options: FindNativeDexFactoryPoolOptions): Promise<string | null>;
//# sourceMappingURL=nativeDexFactoryPool.d.ts.map