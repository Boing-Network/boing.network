/**
 * LP vault v2 for custody (v8/v9) pools: `deposit` pulls both tokens, adds liquidity and mints vault shares
 * (1:1 with the LP the pool returns); `withdraw` burns shares and the pool pays the user directly;
 * `transfer_shares` (self-transfer is a no-op). Mirrors `crates/boing-execution/src/native_lp_vault_v2.rs`
 * (see `tests/boingLpVaultV2.test.ts`, vectors from the CLI).
 */
import type { BoingClient } from './client.js';
import type { TransactionInput } from './bincode.js';
import { type BoingPoolSignedTx, type BoingPoolSendOptions } from './boingPool.js';
import { type Ed25519SecretKey32 } from './transactionBuilder.js';
export declare const BOING_LP_VAULT_V2_BYTECODE_HEX: `0x${string}`;
export declare const BOING_LP_VAULT_V2_SELECTOR_CONFIGURE = 192;
export declare const BOING_LP_VAULT_V2_SELECTOR_DEPOSIT = 193;
export declare const BOING_LP_VAULT_V2_SELECTOR_WITHDRAW = 194;
export declare const BOING_LP_VAULT_V2_SELECTOR_TRANSFER_SHARES = 195;
/** Share slot = `holder ^ "BOING_LP_VAULT_V2_SHARES"` (zero-padded). */
export declare const BOING_LP_VAULT_V2_SHARE_XOR: Uint8Array<ArrayBufferLike>;
export declare const BOING_LP_VAULT_V2_KEY_CONFIGURED_HEX: string;
export declare const BOING_LP_VAULT_V2_KEY_POOL_HEX: string;
export declare const BOING_LP_VAULT_V2_KEY_TOKEN_A_HEX: string;
export declare const BOING_LP_VAULT_V2_KEY_TOKEN_B_HEX: string;
export declare const BOING_LP_VAULT_V2_KEY_TOTAL_SHARES_HEX: string;
export declare function boingLpVaultV2ShareKeyHex(holderHex: string): string;
export declare const encodeBoingLpVaultV2ConfigureCalldata: (poolHex: string, tokenAHex: string, tokenBHex: string) => Uint8Array<ArrayBufferLike>;
export declare const encodeBoingLpVaultV2DepositCalldata: (amountA: bigint, amountB: bigint, minShares: bigint, deadline?: bigint) => Uint8Array<ArrayBufferLike>;
export declare const encodeBoingLpVaultV2WithdrawCalldata: (shares: bigint, minA: bigint, minB: bigint, deadline?: bigint) => Uint8Array<ArrayBufferLike>;
export declare const encodeBoingLpVaultV2TransferSharesCalldata: (toHex: string, amount: bigint) => Uint8Array<ArrayBufferLike>;
export declare function buildBoingLpVaultV2DeployTransaction(input: {
    nonce: bigint;
    senderHex: string;
}): TransactionInput;
export interface BoingLpVaultV2Config {
    vaultHex: string;
    poolHex: string;
    tokenAHex: string;
    tokenBHex: string;
}
export declare const buildBoingLpVaultV2ConfigureTransaction: (nonce: bigint, senderHex: string, c: BoingLpVaultV2Config) => TransactionInput;
export declare const buildBoingLpVaultV2DepositTransaction: (nonce: bigint, senderHex: string, c: BoingLpVaultV2Config, a: bigint, b: bigint, minShares: bigint, deadline?: bigint) => TransactionInput;
export declare const buildBoingLpVaultV2WithdrawTransaction: (nonce: bigint, senderHex: string, c: BoingLpVaultV2Config, shares: bigint, minA: bigint, minB: bigint, deadline?: bigint) => TransactionInput;
export declare const buildBoingLpVaultV2TransferSharesTransaction: (nonce: bigint, senderHex: string, c: BoingLpVaultV2Config, toHex: string, amount: bigint) => TransactionInput;
/** Exact-amount approvals (vault as spender) + deposit, in nonce order. */
export declare function buildBoingLpVaultV2DepositFlow(nonce: bigint, senderHex: string, c: BoingLpVaultV2Config, a: bigint, b: bigint, minShares: bigint, deadline?: bigint): TransactionInput[];
/**
 * Balanced deposit for `amountA` at current reserves: `amountB = ceil(amountA * rB / rA)`, expected shares
 * (= pool LP minted) and `minShares` after `slippageBps`. Off-ratio excess is donated to the pool, so use this.
 */
export declare function quoteBoingLpVaultV2Deposit(s: {
    reserveA: bigint;
    reserveB: bigint;
    totalLp: bigint;
}, amountA: bigint, slippageBps?: bigint): {
    amountA: bigint;
    amountB: bigint;
    shares: bigint;
    minShares: bigint;
};
/** Withdraw outputs for `shares` (pool `burn * reserve / totalLp`) and slippage-bounded minimums. */
export declare function quoteBoingLpVaultV2Withdraw(s: {
    reserveA: bigint;
    reserveB: bigint;
    totalLp: bigint;
}, shares: bigint, slippageBps?: bigint): {
    outA: bigint;
    outB: bigint;
    minA: bigint;
    minB: bigint;
};
export declare function getBoingLpVaultV2Shares(client: BoingClient, vaultHex: string, holderHex: string): Promise<bigint>;
export declare function getBoingLpVaultV2State(client: BoingClient, vaultHex: string): Promise<{
    configured: boolean;
    poolHex: string;
    tokenAHex: string;
    tokenBHex: string;
    totalShares: bigint;
}>;
/** approve A, approve B (exact), deposit. Quotes a balanced `amountB` and `minShares` unless given. */
export declare function depositBoingLpVaultV2(client: BoingClient | null, key: Ed25519SecretKey32, input: BoingLpVaultV2Config & {
    amountA: bigint;
    amountB?: bigint;
    minShares?: bigint;
    slippageBps?: bigint;
    deadlineHeight?: bigint;
} & BoingPoolSendOptions): Promise<{
    txs: BoingPoolSignedTx[];
    amountB: bigint;
    minShares: bigint;
}>;
/** Burn `shares`; the pool pays both tokens to the signer. Quotes minimums unless given. */
export declare function withdrawBoingLpVaultV2(client: BoingClient | null, key: Ed25519SecretKey32, input: BoingLpVaultV2Config & {
    shares: bigint;
    minA?: bigint;
    minB?: bigint;
    slippageBps?: bigint;
    deadlineHeight?: bigint;
} & BoingPoolSendOptions): Promise<{
    tx: BoingPoolSignedTx;
    minA: bigint;
    minB: bigint;
}>;
//# sourceMappingURL=boingLpVaultV2.d.ts.map