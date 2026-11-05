/**
 * Native DEX **create-and-seed** factory calldata + one-approval batch helpers.
 * Matches `boing_execution::native_dex_create_and_seed` — see `docs/NATIVE-DEX-CREATE-AND-SEED.md`.
 *
 * On-chain `create_and_seed` CREATE2s the pool, `set_tokens`, and optionally `register_pair`.
 * Liquidity seed stays a separate Batch step with `CALLER` = tx sender.
 */
import { NATIVE_DEX_CREATE_AND_SEED_BYTECODE_HEX } from './defaultNativeDexCreateAndSeedBytecodeHex.js';
import type { TransactionInput } from './bincode.js';
import { type BuildBatchCall } from './transactionBuilder.js';
/** `create_and_seed` selector (`0xC0`). */
export declare const SELECTOR_NATIVE_DEX_CREATE_AND_SEED = 192;
/** Max pool runtime bytes accepted by the factory. */
export declare const NATIVE_DEX_CREATE_AND_SEED_MAX_POOL_BYTES: number;
/** CREATE2 salt for deploying the factory itself. */
export declare const NATIVE_DEX_CREATE_AND_SEED_CREATE2_SALT_V1_HEX: string;
/** `Log3` topic0 after a successful create. */
export declare const NATIVE_DEX_CREATE_AND_SEED_TOPIC_HEX: string;
export { NATIVE_DEX_CREATE_AND_SEED_BYTECODE_HEX };
/** Random 32-byte CREATE2 salt as `0x` hex (pool child salt, not the factory deploy salt). */
export declare function randomCreateAndSeedPoolSaltHex(): `0x${string}`;
/**
 * Encode `create_and_seed` calldata (header + pool runtime).
 * Pass `directoryHex32` null/empty to skip `register_pair`.
 */
export declare function encodeNativeDexCreateAndSeedCalldata(input: {
    saltHex32: string;
    tokenAHex32: string;
    tokenBHex32: string;
    directoryHex32?: string | null;
    poolBytecode: Uint8Array | string;
}): Uint8Array;
export declare function encodeNativeDexCreateAndSeedCalldataHex(input: Parameters<typeof encodeNativeDexCreateAndSeedCalldata>[0]): `0x${string}`;
/** Predict the pool AccountId for a create-and-seed call. */
export declare function predictCreateAndSeedPoolAddress(factoryHex32: string, saltHex32: string, poolBytecode: Uint8Array | string): string;
/** Predict the factory AccountId when deployed with the canonical CREATE2 salt. */
export declare function predictNativeDexCreateAndSeedFactoryCreate2Address(deployerHex: string): string;
export type CreateAndSeedBatchInput = {
    senderHex: string;
    factoryHex: string;
    saltHex32: string;
    tokenAHex: string;
    tokenBHex: string;
    amountA: bigint;
    amountB: bigint;
    minLp?: bigint;
    directoryHex?: string | null;
    poolBytecode: Uint8Array | string;
    /** Optional wrap step: contract + calldata + native value (e.g. wBOING.deposit). */
    wrap?: {
        contractHex: string;
        calldata: Uint8Array;
        value: bigint;
    } | null;
    nonce?: bigint;
};
/**
 * One-approval Batch: optional wrap → approve A/B to predicted pool → create_and_seed → add_liquidity.
 */
export declare function buildCreateAndSeedBatchCalls(input: CreateAndSeedBatchInput): {
    poolHex: string;
    calls: BuildBatchCall[];
};
/** Full Batch transaction input for Express / signSimulateSubmit. */
export declare function buildCreateAndSeedBatchTransaction(input: CreateAndSeedBatchInput): {
    poolHex: string;
    tx: TransactionInput;
};
//# sourceMappingURL=nativeDexCreateAndSeed.d.ts.map