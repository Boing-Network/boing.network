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
    /** Atomic batch of transfers / contract calls under one signature + nonce (node ≥ batch release). */
    readonly Batch: 9;
    /** Open validator registration (owner signs tx; node key signs the registration proof). */
    readonly ValidatorRegister: 10;
    readonly ValidatorDeregister: 11;
    readonly ValidatorUnjail: 12;
    /** Name registry (`label.boing`), see `boingNames.ts`. */
    readonly NameCommit: 13;
    readonly NameRegister: 14;
    readonly NameRenew: 15;
    readonly NameTransfer: 16;
    readonly NameSetTarget: 17;
    readonly NameSetPrimary: 18;
    readonly NameRelease: 19;
    /** Contract call carrying native BOING (`value`, base units). Testnet: from height 1150. */
    readonly ContractCallWithValue: 20;
    /** L1 media blob publish (`docs/BOING-L1-MEDIA-BLOBS.md`). Phase 2 cap 1 MiB. */
    readonly PutBlob: 21;
    /** Announce blob id + size without inline bytes (P2P / sidecar data plane). */
    readonly BlobAnnounce: 22;
    /** Extend media blob pin horizon (Phase 5). */
    readonly RenewMediaBlobPin: 23;
    /** Pay for one DAS chunk sample (Phase 5 market scaffold). */
    readonly PayBlobSample: 24;
    /** Register on-chain sample offer (Phase 7). */
    readonly RegisterBlobSampleOffer: 25;
    /** Settle sample match against a registered provider (Phase 7). */
    readonly SettleBlobSampleMatch: 26;
    /** Cancel sample offer and refund escrow (Phase 9). */
    readonly CancelBlobSampleOffer: 27;
    /** Challenge sample serve availability (Phase 10). */
    readonly ChallengeBlobSampleServe: 28;
    /** Respond to serve challenge with DAS sample (Phase 10). */
    readonly RespondBlobSampleServe: 29;
    /** Submit consensus slash appeal (Phase 14). */
    readonly SubmitSlashAppeal: 30;
    /** Resolve consensus slash appeal (Phase 14; unattested). */
    readonly ResolveSlashAppeal: 31;
    /** Attested resolve with Ed25519 ballot (Phase 18). */
    readonly ResolveSlashAppealAttested: 32;
    /** BLS aggregate resolve with self-contained pubkeys (Phase 30; not consensus-admitted). */
    readonly ResolveSlashAppealBlsAggregate: 33;
    /** Grant media-blob download ACL (Phase 34; publisher-only). */
    readonly GrantBlobDownload: 34;
    /** Revoke media-blob download ACL (Phase 34; publisher-only). */
    readonly RevokeBlobDownload: 35;
};
/** `BlobRetention` discriminants (Rust `boing_primitives::BlobRetention`). */
export declare const BlobRetentionVariant: {
    readonly Prunable: 0;
    readonly Forever: 1;
};
/** `BatchCall` discriminants (Rust `boing_primitives::BatchCall`). */
export declare const BatchCallVariant: {
    readonly Transfer: 0;
    readonly ContractCall: 1;
    /** Contract call step carrying native BOING (testnet height 1150+). */
    readonly ContractCallWithValue: 2;
};
/** Protocol cap on steps in one batch (`boing_primitives::MAX_BATCH_CALLS`). */
export declare const MAX_BATCH_CALLS = 16;
/** Protocol cap on summed contract-call calldata in one batch (`MAX_BATCH_CALLDATA_BYTES`). */
export declare const MAX_BATCH_CALLDATA_BYTES: number;
export type BatchCallInput = {
    kind: 'transfer';
    to: Uint8Array;
    amount: bigint;
} | {
    kind: 'contractCall';
    contract: Uint8Array;
    calldata: Uint8Array;
} | {
    kind: 'contractCallWithValue';
    contract: Uint8Array;
    calldata: Uint8Array;
    value: bigint;
};
/** Throws with the same reasons the node uses (`BatchValidationError`). */
export declare function validateBatchCalls(calls: BatchCallInput[]): void;
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
}
/** Native-value rules (testnet height 1150+): moves `value` base units to the contract before it runs. */
 | {
    kind: 'contractCallWithValue';
    contract: Uint8Array;
    calldata: Uint8Array;
    value: bigint;
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
} | {
    kind: 'batch';
    calls: BatchCallInput[];
} | {
    kind: 'validatorRegister';
    nodeId: Uint8Array;
    nodeSignature: Uint8Array;
} | {
    kind: 'validatorDeregister';
    nodeId: Uint8Array;
} | {
    kind: 'validatorUnjail';
    nodeId: Uint8Array;
} | {
    kind: 'nameCommit';
    commitment: Uint8Array;
} | {
    kind: 'nameRegister';
    name: string;
    salt: Uint8Array;
    target: Uint8Array;
    years: number;
} | {
    kind: 'nameRenew';
    name: string;
    years: number;
} | {
    kind: 'nameTransfer';
    name: string;
    newOwner: Uint8Array;
} | {
    kind: 'nameSetTarget';
    name: string;
    target: Uint8Array;
} | {
    kind: 'nameSetPrimary';
    name: string | null;
} | {
    kind: 'nameRelease';
    name: string;
} | {
    kind: 'putBlob';
    retention: {
        kind: 'forever';
    } | {
        kind: 'prunable';
        ttlBlocks: bigint;
    };
    contentType?: string;
    data: Uint8Array;
} | {
    kind: 'blobAnnounce';
    blobId: Uint8Array;
    size: number;
    retention: {
        kind: 'forever';
    } | {
        kind: 'prunable';
        ttlBlocks: bigint;
    };
    contentType?: string;
} | {
    kind: 'renewMediaBlobPin';
    blobId: Uint8Array;
    size: number;
    extendBlocks: bigint;
} | {
    kind: 'payBlobSample';
    blobId: Uint8Array;
    chunkIndex: number;
    chunkCount: number;
} | {
    kind: 'registerBlobSampleOffer';
    blobId: Uint8Array;
    chunkIndex: number;
    chunkCount: number;
    askFee: bigint;
    expiresAtHeight: bigint;
} | {
    kind: 'settleBlobSampleMatch';
    blobId: Uint8Array;
    chunkIndex: number;
    chunkCount: number;
    provider: Uint8Array;
} | {
    kind: 'cancelBlobSampleOffer';
    blobId: Uint8Array;
    chunkIndex: number;
} | {
    kind: 'challengeBlobSampleServe';
    blobId: Uint8Array;
    chunkIndex: number;
    chunkCount: number;
    provider: Uint8Array;
} | {
    kind: 'respondBlobSampleServe';
    blobId: Uint8Array;
    chunkIndex: number;
    chunkCount: number;
    challenger: Uint8Array;
    chunkHash: Uint8Array;
    chunksRoot: Uint8Array;
    siblings: Uint8Array[];
} | {
    kind: 'submitSlashAppeal';
    slashId: bigint;
    evidence: Uint8Array;
} | {
    kind: 'resolveSlashAppeal';
    appealId: bigint;
    approved: boolean;
} | {
    kind: 'resolveSlashAppealAttested';
    appealId: bigint;
    approved: boolean;
    /** 64-byte Ed25519 ballot signature (raw `[u8; 64]`, no length prefix). */
    ballotSignature: Uint8Array;
} | {
    kind: 'resolveSlashAppealBlsAggregate';
    appealId: bigint;
    approved: boolean;
    signerBitset: Uint8Array;
    /** 96-byte compressed BLS12-381 G2 aggregate signature. */
    aggregateSignature: Uint8Array;
    /** 48-byte compressed G1 pubkeys, ordered with decoded bitset. */
    signerBlsPubkeys: Uint8Array[];
} | {
    kind: 'grantBlobDownload';
    blobId: Uint8Array;
    grantee: Uint8Array;
} | {
    kind: 'revokeBlobDownload';
    blobId: Uint8Array;
    grantee: Uint8Array;
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
    /**
     * Optional priority tip (ledger units). `0` / omitted keeps historical wire encoding and v2
     * signing preimage. Non-zero tips use the `BOING_TX_PRIORITY` domain and append tip to the
     * signed-tx encoding (matches Rust `SignedTransaction.priority_tip`).
     */
    priorityTip?: bigint;
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
/** Domain tag of the v2 (chain-bound) signing preimage. */
export declare const TX_SIGNING_DOMAIN_V2: Uint8Array<ArrayBuffer>;
/** Domain tag when a non-zero priority tip is bound into the signature. */
export declare const TX_SIGNING_DOMAIN_PRIORITY: Uint8Array<ArrayBuffer>;
/**
 * v2 signable hash (security fix #2, cross-chain replay): BLAKE3( "BOING_TX_V2" || chain_id_le(8) ||
 * nonce_le || sender(32) || bincode(payload) || bincode(access_list) ). Matches Rust
 * `signable_transaction_hash_v2`. Required by nodes after the network's v2 activation height
 * (testnet 6913: height 900; mainnet 6914: genesis).
 */
export declare function signableTransactionHashV2(tx: TransactionInput, chainId: bigint | number): Uint8Array;
/**
 * Signable digest including an optional priority tip. Tip `0` == [`signableTransactionHashV2`].
 * Non-zero: BLAKE3(`BOING_TX_PRIORITY` || chain_id || nonce || sender || payload || access_list || tip_le16).
 */
export declare function signableTransactionHashWithTip(tx: TransactionInput, chainId: bigint | number, priorityTip?: bigint): Uint8Array;
/**
 * Derives the mempool / receipt **`tx_id`** (32-byte BLAKE3) from **`0x` + bincode(`SignedTransaction`)** hex
 * returned by Boing Express after `boing_signTransaction`. Matches **`Transaction::id()`** / **`boing_getTransactionReceipt`** param
 * per [RPC-API-SPEC.md](https://github.com/Boing-Network/boing.network/blob/main/docs/RPC-API-SPEC.md) (signable body excludes the trailing serde `Signature` block
 * and optional priority tip).
 *
 * @throws if hex is invalid or the trailing signature length field is not 64
 */
export declare function transactionIdFromSignedTransactionHex(signedTxHex: string): string;
//# sourceMappingURL=bincode.d.ts.map