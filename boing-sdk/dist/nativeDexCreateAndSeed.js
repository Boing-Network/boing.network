/**
 * Native DEX **create-and-seed** factory calldata + one-approval batch helpers.
 * Matches `boing_execution::native_dex_create_and_seed` — see `docs/NATIVE-DEX-CREATE-AND-SEED.md`.
 *
 * On-chain `create_and_seed` CREATE2s the pool, `set_tokens`, and optionally `register_pair`.
 * Liquidity seed stays a separate Batch step with `CALLER` = tx sender.
 */
import { predictCreate2ContractAddress } from './create2.js';
import { NATIVE_DEX_CREATE_AND_SEED_BYTECODE_HEX } from './defaultNativeDexCreateAndSeedBytecodeHex.js';
import { bytesToHex, hexToBytes, validateHex32 } from './hex.js';
import { encodeBoingPoolV8AddLiquidityCalldata } from './boingPool.js';
import { encodeReferenceApproveCalldata } from './referenceToken.js';
import { buildBatchTransaction } from './transactionBuilder.js';
/** `create_and_seed` selector (`0xC0`). */
export const SELECTOR_NATIVE_DEX_CREATE_AND_SEED = 0xc0;
/** Max pool runtime bytes accepted by the factory. */
export const NATIVE_DEX_CREATE_AND_SEED_MAX_POOL_BYTES = 32 * 1024;
/** CREATE2 salt for deploying the factory itself. */
export const NATIVE_DEX_CREATE_AND_SEED_CREATE2_SALT_V1_HEX = (() => {
    const u8 = new Uint8Array(32);
    u8.set(new TextEncoder().encode('BOING_NATIVEDEX_CREATESEED_V1'));
    return bytesToHex(u8);
})();
/** `Log3` topic0 after a successful create. */
export const NATIVE_DEX_CREATE_AND_SEED_TOPIC_HEX = (() => {
    const u8 = new Uint8Array(32);
    u8.set(new TextEncoder().encode('BOING_NATIVE_DEX_CREATE_SEED1'));
    return bytesToHex(u8);
})();
export { NATIVE_DEX_CREATE_AND_SEED_BYTECODE_HEX };
function selectorWord(selector) {
    const w = new Uint8Array(32);
    w[31] = selector & 0xff;
    return w;
}
function u128Word(n) {
    if (n < 0n || n >= 1n << 128n)
        throw new RangeError('u128 out of range');
    const w = new Uint8Array(32);
    for (let i = 0; i < 16; i++) {
        w[31 - i] = Number((n >> BigInt(8 * i)) & 0xffn);
    }
    return w;
}
function concatBytes(parts) {
    const n = parts.reduce((a, p) => a + p.length, 0);
    const out = new Uint8Array(n);
    let o = 0;
    for (const p of parts) {
        out.set(p, o);
        o += p.length;
    }
    return out;
}
/** Random 32-byte CREATE2 salt as `0x` hex (pool child salt, not the factory deploy salt). */
export function randomCreateAndSeedPoolSaltHex() {
    const u8 = new Uint8Array(32);
    if (typeof globalThis.crypto?.getRandomValues === 'function') {
        globalThis.crypto.getRandomValues(u8);
    }
    else {
        for (let i = 0; i < 32; i++)
            u8[i] = (Math.random() * 256) | 0;
    }
    return bytesToHex(u8);
}
/**
 * Encode `create_and_seed` calldata (header + pool runtime).
 * Pass `directoryHex32` null/empty to skip `register_pair`.
 */
export function encodeNativeDexCreateAndSeedCalldata(input) {
    const salt = hexToBytes(validateHex32(input.saltHex32));
    const tokenA = hexToBytes(validateHex32(input.tokenAHex32));
    const tokenB = hexToBytes(validateHex32(input.tokenBHex32));
    const directory = input.directoryHex32
        ? hexToBytes(validateHex32(input.directoryHex32))
        : new Uint8Array(32);
    const pool = typeof input.poolBytecode === 'string'
        ? hexToBytes(input.poolBytecode.startsWith('0x') ? input.poolBytecode : `0x${input.poolBytecode}`)
        : input.poolBytecode;
    if (pool.length === 0 || pool.length > NATIVE_DEX_CREATE_AND_SEED_MAX_POOL_BYTES) {
        throw new RangeError(`pool bytecode length ${pool.length} out of 1..=${NATIVE_DEX_CREATE_AND_SEED_MAX_POOL_BYTES}`);
    }
    return concatBytes([
        selectorWord(SELECTOR_NATIVE_DEX_CREATE_AND_SEED),
        salt,
        tokenA,
        tokenB,
        directory,
        u128Word(BigInt(pool.length)),
        pool,
    ]);
}
export function encodeNativeDexCreateAndSeedCalldataHex(input) {
    return bytesToHex(encodeNativeDexCreateAndSeedCalldata(input));
}
/** Predict the pool AccountId for a create-and-seed call. */
export function predictCreateAndSeedPoolAddress(factoryHex32, saltHex32, poolBytecode) {
    const pool = typeof poolBytecode === 'string'
        ? hexToBytes(poolBytecode.startsWith('0x') ? poolBytecode : `0x${poolBytecode}`)
        : poolBytecode;
    return predictCreate2ContractAddress(factoryHex32, hexToBytes(validateHex32(saltHex32)), pool);
}
/** Predict the factory AccountId when deployed with the canonical CREATE2 salt. */
export function predictNativeDexCreateAndSeedFactoryCreate2Address(deployerHex) {
    return predictCreate2ContractAddress(deployerHex, hexToBytes(NATIVE_DEX_CREATE_AND_SEED_CREATE2_SALT_V1_HEX), hexToBytes(NATIVE_DEX_CREATE_AND_SEED_BYTECODE_HEX));
}
/**
 * One-approval Batch: optional wrap → approve A/B to predicted pool → create_and_seed → add_liquidity.
 */
export function buildCreateAndSeedBatchCalls(input) {
    const factory = validateHex32(input.factoryHex).toLowerCase();
    const tokenA = validateHex32(input.tokenAHex).toLowerCase();
    const tokenB = validateHex32(input.tokenBHex).toLowerCase();
    const salt = validateHex32(input.saltHex32).toLowerCase();
    const directory = input.directoryHex ? validateHex32(input.directoryHex).toLowerCase() : null;
    const poolHex = predictCreateAndSeedPoolAddress(factory, salt, input.poolBytecode).toLowerCase();
    const createCd = encodeNativeDexCreateAndSeedCalldata({
        saltHex32: salt,
        tokenAHex32: tokenA,
        tokenBHex32: tokenB,
        directoryHex32: directory,
        poolBytecode: input.poolBytecode,
    });
    const calls = [];
    if (input.wrap) {
        calls.push({
            kind: 'contractCallWithValue',
            contractHex: validateHex32(input.wrap.contractHex).toLowerCase(),
            calldata: input.wrap.calldata,
            value: input.wrap.value,
        });
    }
    calls.push({
        kind: 'contractCall',
        contractHex: tokenA,
        calldata: encodeReferenceApproveCalldata(poolHex, input.amountA),
    }, {
        kind: 'contractCall',
        contractHex: tokenB,
        calldata: encodeReferenceApproveCalldata(poolHex, input.amountB),
    }, { kind: 'contractCall', contractHex: factory, calldata: createCd }, {
        kind: 'contractCall',
        contractHex: poolHex,
        calldata: encodeBoingPoolV8AddLiquidityCalldata(input.amountA, input.amountB, input.minLp ?? 1n, 0n),
    });
    return { poolHex, calls };
}
/** Full Batch transaction input for Express / signSimulateSubmit. */
export function buildCreateAndSeedBatchTransaction(input) {
    const { poolHex, calls } = buildCreateAndSeedBatchCalls(input);
    const ids = [
        input.senderHex,
        input.factoryHex,
        input.tokenAHex,
        input.tokenBHex,
        poolHex,
        input.directoryHex || null,
        input.wrap?.contractHex || null,
    ]
        .filter(Boolean)
        .map((h) => validateHex32(h).toLowerCase())
        .filter((v, i, a) => a.indexOf(v) === i);
    const tx = buildBatchTransaction({
        nonce: input.nonce ?? 0n,
        senderHex: input.senderHex,
        calls,
        accessList: { read: ids, write: ids },
    });
    return { poolHex, tx };
}
