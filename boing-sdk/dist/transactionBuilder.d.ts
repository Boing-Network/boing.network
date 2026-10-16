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
/** One step of {@link buildBatchTransaction} (hex addresses). */
export type BuildBatchCall = {
    kind: 'transfer';
    toHex: string;
    amount: bigint;
} | {
    kind: 'contractCall';
    contractHex: string;
    calldata: Uint8Array;
};
export interface BuildBatchInput {
    nonce: bigint;
    senderHex: string;
    /** 1..MAX_BATCH_CALLS steps, applied in order; any failure reverts all of them. */
    calls: BuildBatchCall[];
    /** Defaults to sender + every recipient / contract (read and write). */
    accessList?: {
        read: string[];
        write: string[];
    };
}
/**
 * Native atomic batch (`TransactionPayload::Batch`): BOING transfers and/or contract calls
 * (e.g. token `transfer`, NFT transfer calldata) under one signature and one nonce.
 * Requires nodes running the batch-capable release.
 */
export declare function buildBatchTransaction(input: BuildBatchInput): TransactionInput;
/**
 * `true` when `boing_health.rpc_surface` advertises native batch support. Older nodes omit the
 * flag (→ `false`), so callers should fall back to sequential transfers.
 */
export declare function nodeSupportsNativeBatch(health: {
    rpc_surface?: {
        tx_payload_batch?: boolean;
    } | null;
} | null | undefined): boolean;
/** Batch step cap advertised by the node, else the protocol default (16). */
export declare function nodeBatchMaxCalls(health: {
    rpc_surface?: {
        tx_batch_max_calls?: number;
    } | null;
} | null | undefined): number;
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
/** Signing options. `chainId` selects the v2 (chain-bound) domain; `null` forces the legacy format. */
export interface SignOptions {
    chainId?: bigint | number | null;
}
/**
 * Process default chain id for v2 signing (used when `SignOptions.chainId` is omitted).
 * `null` = legacy (pre-chain-id) signatures, which nodes reject after their v2 activation height.
 */
export declare function setDefaultSigningChainId(chainId: bigint | number | null): void;
export declare function getDefaultSigningChainId(): bigint | null;
/** Chain id a node advertises for v2 signing (`boing_health.tx_signing`), or `null` (older node). */
export declare function fetchSigningChainId(client: BoingClient): Promise<bigint | null>;
/** Sets the default signing chain id from the node once (no-op when a default is already set). */
export declare function configureSigningFromNode(client: BoingClient): Promise<bigint | null>;
/** Digest to sign for `tx` under `opts` (v2 when a chain id is known, else legacy). */
export declare function signingHashFor(tx: TransactionInput, opts?: SignOptions): Uint8Array;
/** Like `signTransactionInput`, but first adopts the node's advertised chain id (v2) when `client` is given. */
export declare function signTransactionInputForClient(client: BoingClient | null | undefined, tx: TransactionInput, secretKey32: Ed25519SecretKey32): Promise<string>;
export declare function signTransactionInput(tx: TransactionInput, secretKey32: Ed25519SecretKey32, opts?: SignOptions): Promise<string>;
/**
 * Sign with a custom async signer (e.g. hardware wallet or IPC to Boing Express).
 * Caller must ensure the signer corresponds to `tx.sender`; no verification here.
 */
export declare function signTransactionInputWithSigner(tx: TransactionInput, signHash: (hash32: Uint8Array) => Promise<Uint8Array>, opts?: SignOptions): Promise<string>;
//# sourceMappingURL=transactionBuilder.d.ts.map