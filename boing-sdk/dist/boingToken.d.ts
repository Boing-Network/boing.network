/**
 * First-class reference fungible token helpers -- the TypeScript twin of the `boing token` CLI
 * (`crates/boing-cli/src/token.rs`). Transactions are byte-identical to the CLI for the same
 * signer, nonce and arguments (cross-checked in `tests/boingToken.test.ts` against CLI `--dry-run`
 * vectors): Boing VM template bytecode, bincode `SignedTransaction`, Ed25519, nonce-derived token
 * address, access list = Rust `Transaction::suggested_parallel_access_list()`.
 *
 * See `docs/BOING-REFERENCE-TOKEN.md`. The template allows exactly one `mint_first` (fixed supply);
 * the first caller becomes admin.
 */
import type { BoingClient } from './client.js';
import type { TransactionInput } from './bincode.js';
import { type Ed25519SecretKey32 } from './transactionBuilder.js';
/** QA purpose category for fungible deploys (same as CLI `TOKEN_PURPOSE`). */
export declare const BOING_TOKEN_PURPOSE = "token";
/** `TransactionPayload::MAX_ASSET_NAME_LEN` (UTF-8 bytes). */
export declare const BOING_TOKEN_MAX_NAME_BYTES = 256;
/** `TransactionPayload::MAX_ASSET_SYMBOL_LEN` (UTF-8 bytes). */
export declare const BOING_TOKEN_MAX_SYMBOL_BYTES = 32;
/** `REF_FUNGIBLE_BALANCE_XOR`: ASCII `BOING_REFFUNGIBLE_BAL1` zero-padded to 32 bytes. */
export declare const REFERENCE_FUNGIBLE_BALANCE_XOR: Uint8Array;
/** Template version: `2` (default, like the CLI: approve / transfer_from / allowance) or `1`. */
export type BoingTokenVersion = 1 | 2;
export declare const BOING_TOKEN_DEFAULT_VERSION: BoingTokenVersion;
/** `REF_FUNGIBLE_ALLOWANCE_XOR`: ASCII `BOING_REFFUNGIBLE_ALW2` zero-padded to 32 bytes. */
export declare const REFERENCE_FUNGIBLE_ALLOWANCE_XOR: Uint8Array;
/** v2 storage key of `allowance[owner][spender]`: `(owner[0..16] ‖ spender[0..16]) ^ ALLOWANCE_XOR`. */
export declare function boingTokenAllowanceStorageKeyHex(ownerHex: string, spenderHex: string): string;
/** Reference fungible template bytecode hex (what `boing token bytecode --version vN` prints). */
export declare function boingTokenBytecodeHex(version?: BoingTokenVersion): `0x${string}`;
/** `boing_getContractStorage` key for `holder`'s balance (`holder ^ REF_FUNGIBLE_BALANCE_XOR`). */
export declare function boingTokenBalanceStorageKeyHex(holderHex: string): string;
/** Decode a 32-byte big-endian storage word to a balance (throws if it exceeds u128). */
export declare function decodeBoingTokenBalanceWord(wordHex: string): bigint;
/** Same checks as CLI `validate_metadata`. */
export declare function validateBoingTokenMetadata(name?: string | null, symbol?: string | null): void;
export interface BuildBoingTokenDeployInput {
    nonce: bigint;
    senderHex: string;
    name?: string | null;
    symbol?: string | null;
    version?: BoingTokenVersion;
}
/** Unsigned `ContractDeployWithPurposeAndMetadata` (purpose `token`) for the reference template. */
export declare function buildBoingTokenDeployTransaction(input: BuildBoingTokenDeployInput): TransactionInput;
export interface BuildBoingTokenCallInput {
    nonce: bigint;
    senderHex: string;
    tokenHex: string;
    toHex: string;
    amount: bigint;
}
/** Unsigned one-time `mint_first(to, amount)` call. */
export declare function buildBoingTokenMintTransaction(input: BuildBoingTokenCallInput): TransactionInput;
/** Unsigned `transfer(to, amount)` call. */
export declare function buildBoingTokenTransferTransaction(input: BuildBoingTokenCallInput): TransactionInput;
/** v2: unsigned `approve(spender = toHex, amount)` call (overwrites the allowance). */
export declare function buildBoingTokenApproveTransaction(input: BuildBoingTokenCallInput): TransactionInput;
/** A signed token transaction (and whether it was submitted). */
export interface BoingTokenSignedTx {
    txId: string;
    signedHex: string;
    submitted: boolean;
}
export interface BoingTokenSendOptions {
    /** Override the sender nonce (default: `boing_getAccount`). */
    nonce?: bigint;
    /** Sign only; don't submit (CLI `--dry-run`). A `client` is still needed unless `nonce` is set. */
    dryRun?: boolean;
}
export interface DeployBoingTokenInput extends BoingTokenSendOptions {
    name?: string | null;
    symbol?: string | null;
    /** If > 0, a `mint_first` follows the deploy (nonce + 1). */
    initialSupply?: bigint;
    /** Mint recipient (default: deployer). */
    mintToHex?: string;
    /** Template version (default 2, like `boing token deploy`). */
    version?: BoingTokenVersion;
}
export interface DeployBoingTokenResult {
    deployerHex: string;
    /** Nonce-derived token address. */
    tokenHex: string;
    deployTx: BoingTokenSignedTx;
    mintTx: BoingTokenSignedTx | null;
}
/** `boing token deploy`: deploy the reference template, optionally minting the fixed supply. */
export declare function deployBoingToken(client: BoingClient | null, secretKey32: Ed25519SecretKey32, input?: DeployBoingTokenInput): Promise<DeployBoingTokenResult>;
/** `boing token mint`: one-time `mint_first` (admin only). `toHex` defaults to the signer. */
export declare function mintBoingToken(client: BoingClient | null, secretKey32: Ed25519SecretKey32, input: {
    tokenHex: string;
    toHex?: string;
    amount: bigint;
} & BoingTokenSendOptions): Promise<BoingTokenSignedTx>;
/** `boing token transfer`. */
export declare function transferBoingToken(client: BoingClient | null, secretKey32: Ed25519SecretKey32, input: {
    tokenHex: string;
    toHex: string;
    amount: bigint;
} & BoingTokenSendOptions): Promise<BoingTokenSignedTx>;
/** `boing token balance`: read a holder's balance via `boing_getContractStorage`. */
export declare function getBoingTokenBalance(client: BoingClient, tokenHex: string, holderHex: string): Promise<bigint>;
/** `boing token approve` (v2): let `spenderHex` pull up to `amount` (overwrites). */
export declare function approveBoingToken(client: BoingClient | null, secretKey32: Ed25519SecretKey32, input: {
    tokenHex: string;
    spenderHex: string;
    amount: bigint;
} & BoingTokenSendOptions): Promise<BoingTokenSignedTx>;
/** `boing token allowance` (v2): read `allowance[owner][spender]` via `boing_getContractStorage`. */
export declare function getBoingTokenAllowance(client: BoingClient, tokenHex: string, ownerHex: string, spenderHex: string): Promise<bigint>;
//# sourceMappingURL=boingToken.d.ts.map