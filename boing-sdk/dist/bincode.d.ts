/**
 * bincode 1.3 layout matching `crates/boing-primitives` (serde derive).
 * Used for `Transaction`, `SignedTransaction`, and signable hash preimage.
 * @see docs/BOING-SIGNED-TRANSACTION-ENCODING.md
 */
/** Payload variants — discriminant u32 LE must match Rust enum declaration order. */
export declare const PayloadVariant: {
    readonly Transfer: 0;
    readonly ContractCall: 1;
    readonly ContractDeploy: 2;
    readonly ContractDeployWithPurpose: 3;
    readonly ContractDeployWithPurposeAndMetadata: 4;
    readonly Bond: 5;
    readonly Unbond: 6;
    readonly ClaimUnbond: 7;
    readonly QaPoolVote: 8;
};
export declare function concatBytes(...parts: Uint8Array[]): Uint8Array;
export declare function writeU32Le(n: number): Uint8Array;
export declare function writeU64Le(n: bigint): Uint8Array;
export declare function writeU128Le(n: bigint): Uint8Array;
/** `AccessList` — two length-prefixed vectors of 32-byte accounts. */
export declare function encodeAccessList(read: Uint8Array[], write: Uint8Array[]): Uint8Array;
/** `Vec<u8>` — u64 LE length + raw bytes. */
export declare function encodeByteVec(data: Uint8Array): Uint8Array;
/** UTF-8 string: u64 LE byte length + UTF-8 payload. */
export declare function encodeBincodeString(s: string): Uint8Array;
/** `Option<[u8;32]>` — tag u8 + optional fixed 32 bytes. */
export declare function encodeOptionFixed32(salt: Uint8Array | null | undefined): Uint8Array;
/** `Option<Vec<u8>>` — tag u8 + optional vec. */
export declare function encodeOptionByteVec(opt: Uint8Array | null | undefined): Uint8Array;
/** `Option<String>` — tag u8 + optional string. */
export declare function encodeOptionString(opt: string | null | undefined): Uint8Array;
export type TransactionPayloadInput = {
    kind: 'transfer';
    to: Uint8Array;
    amount: bigint;
} | {
    kind: 'contractCall';
    contract: Uint8Array;
    calldata: Uint8Array;
} | {
    kind: 'contractDeploy';
    bytecode: Uint8Array;
    create2Salt?: Uint8Array | null;
} | {
    kind: 'contractDeployWithPurpose';
    bytecode: Uint8Array;
    purposeCategory: string;
    descriptionHash?: Uint8Array | null;
    create2Salt?: Uint8Array | null;
} | {
    kind: 'contractDeployWithPurposeAndMetadata';
    bytecode: Uint8Array;
    purposeCategory: string;
    descriptionHash?: Uint8Array | null;
    assetName?: string | null;
    assetSymbol?: string | null;
    create2Salt?: Uint8Array | null;
} | {
    kind: 'bond';
    amount: bigint;
} | {
    kind: 'unbond';
    amount: bigint;
} | {
    kind: 'claimUnbond';
} | {
    kind: 'qaPoolVote';
    subject: Uint8Array;
    vote: 'allow' | 'reject' | 'abstain';
};
export declare function encodeTransactionPayload(payload: TransactionPayloadInput): Uint8Array;
export interface TransactionInput {
    nonce: bigint;
    sender: Uint8Array;
    payload: TransactionPayloadInput;
    accessList: {
        read: Uint8Array[];
        write: Uint8Array[];
    };
}
export declare function encodeTransaction(tx: TransactionInput): Uint8Array;
/** `Signature` — serde `serialize_bytes` → u64 LE length + 64 raw bytes. */
export declare function encodeSignature(signature64: Uint8Array): Uint8Array;
export declare function encodeSignedTransaction(tx: TransactionInput, signature64: Uint8Array): Uint8Array;
/**
 * BLAKE3 hash signed by the node for `SignedTransaction` verification.
 * preimage = nonce_le || sender(32) || bincode(payload) || bincode(access_list)
 */
export declare function signableTransactionHash(tx: TransactionInput): Uint8Array;
/**
 * Derives the mempool / receipt **`tx_id`** (32-byte BLAKE3) from **`0x` + bincode(`SignedTransaction`)** hex
 * returned by Boing Express after `boing_signTransaction`. Matches **`Transaction::id()`** / **`boing_getTransactionReceipt`** param
 * per [RPC-API-SPEC.md](https://github.com/Boing-Network/boing.network/blob/main/docs/RPC-API-SPEC.md) (signable body excludes the trailing serde `Signature` block).
 *
 * @throws if hex is invalid or the trailing signature length field is not 64
 */
export declare function transactionIdFromSignedTransactionHex(signedTxHex: string): string;
//# sourceMappingURL=bincode.d.ts.map