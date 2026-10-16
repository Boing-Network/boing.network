/**
 * Boing name registry helpers (`label.boing`) — mirrors `boing_primitives::names` and the `Name*`
 * transaction payloads (variants 13-19).
 *
 * Flow to claim a name: `NameCommit` (hides the label) → wait until it is included → `NameRegister`
 * revealing the label + salt (at least `commit_min_age` blocks later) → optionally `NameSetPrimary`.
 * Rent is burned: 3 chars 10, 4 chars 5, 5 chars 2, 6+ chars 1 BOING per year (testnet).
 */
import type { BoingClient } from './client.js';
import type { TransactionInput } from './bincode.js';
import { type Ed25519SecretKey32 } from './transactionBuilder.js';
export declare const BOING_NAME_SUFFIX = ".boing";
export declare const BOING_NAME_MIN_LEN = 3;
export declare const BOING_NAME_MAX_LEN = 32;
export declare const BOING_NAME_MAX_YEARS = 10;
export declare const BOING_NAME_COMMIT_DOMAIN = "BOING_NAME_COMMIT_V1";
export declare const BOING_RESERVED_NAMES: readonly string[];
export type BoingNameValidation = {
    ok: true;
    label: string;
} | {
    ok: false;
    error: string;
};
/** Same rules as the node: trim, lowercase, drop `.boing`, 3-32 of a-z 0-9 '-', no edge/double hyphen, not address-like. */
export declare function normalizeBoingName(input: string): BoingNameValidation;
export declare function isReservedBoingName(label: string): boolean;
/**
 * True when user input should be treated as a name (contains a letter / ends with `.boing`) rather than
 * a hex address. 64-hex (with or without 0x) is always an address.
 */
export declare function looksLikeBoingName(input: string): boolean;
/** Yearly rent in whole BOING (testnet). */
export declare function boingNameRentPerYear(label: string): bigint;
export declare function boingNameRent(label: string, years: number): bigint;
export declare function displayBoingName(label: string): string;
/** `blake3(domain ‖ u32le(len) ‖ label ‖ owner ‖ salt)` (matches Rust `name_commitment`). */
export declare function boingNameCommitment(label: string, ownerHex: string, salt: Uint8Array): Uint8Array;
/** 32 random bytes (WebCrypto). */
export declare function randomBoingNameSalt(): Uint8Array;
export declare function buildNameCommitTransaction(input: {
    nonce: bigint;
    senderHex: string;
    name: string;
    salt: Uint8Array;
}): TransactionInput;
export declare function buildNameRegisterTransaction(input: {
    nonce: bigint;
    senderHex: string;
    name: string;
    salt: Uint8Array;
    targetHex?: string;
    years?: number;
}): TransactionInput;
export declare function buildNameRenewTransaction(input: {
    nonce: bigint;
    senderHex: string;
    name: string;
    years?: number;
}): TransactionInput;
export declare function buildNameTransferTransaction(input: {
    nonce: bigint;
    senderHex: string;
    name: string;
    newOwnerHex: string;
}): TransactionInput;
export declare function buildNameSetTargetTransaction(input: {
    nonce: bigint;
    senderHex: string;
    name: string;
    targetHex: string;
}): TransactionInput;
export declare function buildNameSetPrimaryTransaction(input: {
    nonce: bigint;
    senderHex: string;
    name: string | null;
}): TransactionInput;
export declare function buildNameReleaseTransaction(input: {
    nonce: bigint;
    senderHex: string;
    name: string;
}): TransactionInput;
export type BoingNameStatus = 'active' | 'grace' | 'expired';
export interface BoingNameRecord {
    name: string;
    display: string;
    owner: string;
    target: string;
    registered_at: number;
    expires_at: number;
    status: BoingNameStatus;
}
export interface BoingReverseName {
    name: string;
    display: string;
    address: string;
    verified: true;
}
export interface BoingNameRegistryInfo {
    enabled: boolean;
    active?: boolean;
    activation_height?: number;
    blocks_per_year?: number;
    grace_blocks?: number;
    commit_min_age?: number;
    commit_max_age?: number;
    rent_per_year?: Record<string, string>;
    max_years?: number;
}
export interface BoingNameCheck {
    input: string;
    name?: string;
    display?: string;
    valid: boolean;
    error?: string;
    reserved?: boolean;
    available: boolean;
    status?: BoingNameStatus | null;
    rent_per_year?: string;
}
export interface BoingNameInfo extends BoingNameCheck {
    record?: BoingNameRecord;
    registry: BoingNameRegistryInfo;
    head_height: number;
}
export interface BoingCheckNamesResult {
    registry: BoingNameRegistryInfo;
    results: BoingNameCheck[];
}
/**
 * Resolve user input to an address: 64-hex passes through (`0x` + lowercase); a name resolves via RPC.
 * Returns null when a name is not registered / not active.
 */
export declare function resolveBoingRecipient(client: BoingClient, input: string): Promise<{
    address: string;
    name: string | null;
} | null>;
/**
 * Commit to a name (step 1). Keep `salt` (and the label) until `registerBoingName` succeeds; without
 * them the commitment cannot be revealed.
 */
export declare function commitBoingName(client: BoingClient, secret: Ed25519SecretKey32, name: string, salt?: Uint8Array<ArrayBufferLike>): Promise<{
    salt: Uint8Array<ArrayBufferLike>;
    saltHex: string;
    result: import("./types.js").SubmitTransactionResult;
}>;
/** Reveal + register (step 2), after the commit is included and matured. */
export declare function registerBoingName(client: BoingClient, secret: Ed25519SecretKey32, input: {
    name: string;
    salt: Uint8Array;
    targetHex?: string;
    years?: number;
}): Promise<import("./types.js").SubmitTransactionResult>;
export declare function setBoingPrimaryName(client: BoingClient, secret: Ed25519SecretKey32, name: string | null): Promise<import("./types.js").SubmitTransactionResult>;
//# sourceMappingURL=boingNames.d.ts.map