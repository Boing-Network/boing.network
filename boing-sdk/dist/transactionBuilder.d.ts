/**
 * High-level transaction construction + Ed25519 signing (browser/Node).
 * Produces hex `SignedTransaction` for `boing_submitTransaction`.
 */
import type { BoingClient } from './client.js';
import { type TransactionInput } from './bincode.js';
/** 32-byte Ed25519 seed / secret key (same format as Rust `SigningKey::from_bytes`). */
export type Ed25519SecretKey32 = Uint8Array;
/** Derive Boing `AccountId` hex (0x + 64 hex) from a 32-byte Ed25519 secret seed. */
export declare function senderHexFromSecretKey(secretKey32: Ed25519SecretKey32): Promise<string>;
/** Next tx nonce from chain (`boing_getAccount`). */
export declare function fetchNextNonce(client: BoingClient, senderHex: string): Promise<bigint>;
export interface BuildTransferInput {
    nonce: bigint;
    /** 32-byte account hex (must match public key of signer). */
    senderHex: string;
    toHex: string;
    amount: bigint;
    accessList?: {
        read: string[];
        write: string[];
    };
}
export declare function buildTransferTransaction(input: BuildTransferInput): TransactionInput;
export interface BuildContractCallInput {
    nonce: bigint;
    senderHex: string;
    contractHex: string;
    calldata: Uint8Array;
    accessList?: {
        read: string[];
        write: string[];
    };
}
export declare function buildContractCallTransaction(input: BuildContractCallInput): TransactionInput;
export interface BuildDeployWithPurposeInput {
    nonce: bigint;
    senderHex: string;
    bytecode: Uint8Array;
    purposeCategory: string;
    descriptionHash?: Uint8Array | null;
    create2Salt?: Uint8Array | null;
    accessList?: {
        read: string[];
        write: string[];
    };
}
export declare function buildDeployWithPurposeTransaction(input: BuildDeployWithPurposeInput): TransactionInput;
export declare function signTransactionInput(tx: TransactionInput, secretKey32: Ed25519SecretKey32): Promise<string>;
/**
 * Sign with a custom async signer (e.g. hardware wallet or IPC to Boing Express).
 * Caller must ensure the signer corresponds to `tx.sender`; no verification here.
 */
export declare function signTransactionInputWithSigner(tx: TransactionInput, signHash: (hash32: Uint8Array) => Promise<Uint8Array>): Promise<string>;
//# sourceMappingURL=transactionBuilder.d.ts.map