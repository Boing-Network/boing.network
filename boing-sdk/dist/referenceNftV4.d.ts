/**
 * Reference **NFT collection template v4** — calldata, storage keys, receipt-log decoding, storage reads and
 * deploy helpers. Mirrors `crates/boing-execution/src/reference_nft_v4.rs`; spec: `docs/BOING-REFERENCE-NFT.md`
 * § Template v4.
 *
 * v4 keeps the v1–v3 selectors (`owner_of` 0x03, `transfer_nft` 0x04, `set_metadata_hash` 0x05, `mint_batch`
 * 0x06) and their calldata layouts, so the encoders in `referenceNft.ts` work unchanged. What differs:
 *
 * - Every failed check faults, so a receipt with `success: true` means the change happened.
 * - Admin is set by the `0xFD` init frame (deploy with {@link buildReferenceNftV4DeployMetaTx}); there is no
 *   lazy admin and no lazy mint through `transfer_nft`.
 * - Every state change emits a log (see the `REF_NFT_V4_TOPIC_*` constants and {@link decodeReferenceNftV4Log}).
 * - Approvals (`approve`, `set_approval_for_all`), `transfer_admin`, metadata freezing and collection reads.
 * - Metadata is admin-only after mint.
 */
import { type ContractDeployMetaTxObject } from './canonicalDeployArtifacts.js';
import { DEFAULT_REFERENCE_NFT_V4_DEPLOY_BYTECODE_HEX, DEFAULT_REFERENCE_NFT_V4_RUNTIME_BYTECODE_HEX } from './defaultReferenceNftV4BytecodeHex.js';
export declare const SELECTOR_NFT_APPROVE = 7;
export declare const SELECTOR_NFT_SET_APPROVAL_FOR_ALL = 8;
export declare const SELECTOR_NFT_GET_APPROVED = 9;
export declare const SELECTOR_NFT_IS_APPROVED_FOR_ALL = 10;
export declare const SELECTOR_NFT_ADMIN = 11;
export declare const SELECTOR_NFT_TRANSFER_ADMIN = 12;
export declare const SELECTOR_NFT_FREEZE_METADATA = 13;
export declare const SELECTOR_NFT_FREEZE_ALL_METADATA = 14;
export declare const SELECTOR_NFT_METADATA_HASH = 15;
export declare const SELECTOR_NFT_TOTAL_MINTED = 16;
export declare const SELECTOR_NFT_IS_METADATA_FROZEN = 17;
/** Max tokens per v4 `mint_batch` (same as v3). */
export declare const MAX_REFERENCE_NFT_V4_MINT_BATCH = 500;
/** Token ids carried by one `MINT_BATCH` log (500 ids → 16 logs). */
export declare const REF_NFT_V4_IDS_PER_MINT_LOG = 32;
export declare const REFERENCE_NFT_V4_ARTIFACT_ID: "boing.reference_nft_collection.v4";
export declare const REFERENCE_NFT_V4_TEMPLATE_VERSION: "4";
/** Per-token approval slot tag: key = `token_id ^ tag`. */
export declare const REF_NFT_V4_APPROVED_XOR_HEX: string;
/** Per-token metadata freeze slot tag (value 1 = frozen). */
export declare const REF_NFT_V4_FROZEN_XOR_HEX: string;
/** Operator slot tag: key = `(owner[0..16] || operator[0..16]) ^ tag` (value 1 = approved for all). */
export declare const REF_NFT_V4_OPERATOR_XOR_HEX: string;
/** `LOG3 [topic, from, to]`, data `token_id`. */
export declare const REF_NFT_V4_TOPIC_TRANSFER_HEX: string;
/** `LOG2 [topic, to]`, data = up to 32 minted `token_id` words. */
export declare const REF_NFT_V4_TOPIC_MINT_BATCH_HEX: string;
/** `LOG3 [topic, owner, spender]`, data `token_id` (spender 0 = cleared). */
export declare const REF_NFT_V4_TOPIC_APPROVAL_HEX: string;
/** `LOG3 [topic, owner, operator]`, data 0/1 word. */
export declare const REF_NFT_V4_TOPIC_APPROVAL_FOR_ALL_HEX: string;
/** `LOG2 [topic, token_id]`, data `metadata_hash`. */
export declare const REF_NFT_V4_TOPIC_METADATA_HEX: string;
/** `LOG2 [topic, token_id]`, no data (`token_id = 0` means the whole collection). */
export declare const REF_NFT_V4_TOPIC_FREEZE_HEX: string;
/** `LOG3 [topic, old_admin, new_admin]`, no data. */
export declare const REF_NFT_V4_TOPIC_ADMIN_HEX: string;
/** Admin slot (same singleton as v1–v3; written by the init frame). */
export declare const REF_NFT_V4_ADMIN_KEY_HEX: string;
/** Collection-wide freeze flag slot. */
export declare const REF_NFT_V4_FROZEN_ALL_KEY_HEX: string;
/** Total-minted counter slot (u64 word). */
export declare const REF_NFT_V4_TOTAL_MINTED_KEY_HEX: string;
/** Approval slot for `tokenId`. */
export declare function referenceNftV4ApprovedStorageKey(tokenIdHex32: string): string;
/** Per-token metadata freeze slot for `tokenId`. */
export declare function referenceNftV4FrozenStorageKey(tokenIdHex32: string): string;
/** Operator slot for `(owner, operator)`. */
export declare function referenceNftV4OperatorStorageKey(ownerHex32: string, operatorHex32: string): string;
/** 96-byte `approve(spender, tokenId)`; spender = zero account clears the approval. */
export declare function encodeReferenceNftApproveCalldata(spenderHex32: string, tokenIdHex32: string): Uint8Array;
/** 96-byte `set_approval_for_all(operator, approved)`. */
export declare function encodeReferenceNftSetApprovalForAllCalldata(operatorHex32: string, approved: boolean): Uint8Array;
/** 64-byte `get_approved(tokenId)`. */
export declare function encodeReferenceNftGetApprovedCalldata(tokenIdHex32: string): Uint8Array;
/** 96-byte `is_approved_for_all(owner, operator)`. */
export declare function encodeReferenceNftIsApprovedForAllCalldata(ownerHex32: string, operatorHex32: string): Uint8Array;
/** 32-byte `admin()`. */
export declare function encodeReferenceNftAdminCalldata(): Uint8Array;
/** 64-byte `transfer_admin(newAdmin)` (admin only; newAdmin must be non-zero). */
export declare function encodeReferenceNftTransferAdminCalldata(newAdminHex32: string): Uint8Array;
/** 64-byte `freeze_metadata(tokenId)` (admin only). */
export declare function encodeReferenceNftFreezeMetadataCalldata(tokenIdHex32: string): Uint8Array;
/** 32-byte `freeze_all_metadata()` (admin only; irreversible). */
export declare function encodeReferenceNftFreezeAllMetadataCalldata(): Uint8Array;
/** 64-byte `metadata_hash(tokenId)`. */
export declare function encodeReferenceNftMetadataHashCalldata(tokenIdHex32: string): Uint8Array;
/** 32-byte `total_minted()`. */
export declare function encodeReferenceNftTotalMintedCalldata(): Uint8Array;
/** 64-byte `is_metadata_frozen(tokenId)`. */
export declare function encodeReferenceNftIsMetadataFrozenCalldata(tokenIdHex32: string): Uint8Array;
export type ReferenceNftV4Log = {
    kind: 'transfer';
    from: string;
    to: string;
    tokenId: string;
} | {
    kind: 'mint_batch';
    to: string;
    tokenIds: string[];
} | {
    kind: 'approval';
    owner: string;
    spender: string;
    tokenId: string;
} | {
    kind: 'approval_for_all';
    owner: string;
    operator: string;
    approved: boolean;
} | {
    kind: 'metadata';
    tokenId: string;
    metadataHash: string;
} | {
    kind: 'freeze';
    tokenId: string;
    all: boolean;
} | {
    kind: 'admin';
    oldAdmin: string;
    newAdmin: string;
};
/**
 * Decode one `{ topics, data }` receipt log emitted by the v4 template. Returns null for other logs.
 *
 * Logs carry no emitter address on Boing today, so callers must only apply these to transactions whose
 * top-level target is a known v4 collection ({@link isReferenceNftV4Bytecode}). v4 makes no nested calls,
 * so every log in such a receipt came from that collection.
 */
export declare function decodeReferenceNftV4Log(log: {
    topics: readonly string[];
    data: string | Uint8Array;
}): ReferenceNftV4Log | null;
/** Ownership moves (mints have `from` = zero account) from a v4 receipt's logs, in order. */
export declare function referenceNftV4OwnershipChanges(logs: readonly {
    topics: readonly string[];
    data: string | Uint8Array;
}[]): {
    tokenId: string;
    from: string;
    to: string;
}[];
type StorageReader = {
    getContractStorage(contractHex32: string, keyHex32: string): Promise<unknown>;
};
export type ReferenceNftV4TokenState = {
    tokenId: string;
    /** Zero account when the token was never minted. */
    owner: string;
    approved: string;
    metadataHash: string;
    /** True when this token or the whole collection is frozen. */
    metadataFrozen: boolean;
};
/** Read one token's owner, approval, metadata hash and freeze state through `boing_getContractStorage`. */
export declare function fetchReferenceNftV4Token(client: StorageReader, contractHex32: string, tokenIdHex32: string): Promise<ReferenceNftV4TokenState>;
/** Read the collection admin, total minted and the collection-wide freeze flag. */
export declare function fetchReferenceNftV4CollectionInfo(client: StorageReader, contractHex32: string): Promise<{
    admin: string;
    totalMinted: bigint;
    allMetadataFrozen: boolean;
}>;
/** True when `operator` is approved for all of `owner`'s tokens. */
export declare function fetchReferenceNftV4IsApprovedForAll(client: StorageReader, contractHex32: string, ownerHex32: string, operatorHex32: string): Promise<boolean>;
/** True when `codeHex` is the pinned v4 runtime (or deploy payload). */
export declare function isReferenceNftV4Bytecode(codeHex: string): boolean;
export type BuildReferenceNftV4DeployMetaTxInput = {
    name: string;
    symbol: string;
    descriptionHashHex?: string;
};
/** Express `contract_deploy_meta` tx for the pinned v4 collection (`0xFD` init; admin = deployer; purpose `nft`). */
export declare function buildReferenceNftV4DeployMetaTx(input: BuildReferenceNftV4DeployMetaTxInput): ContractDeployMetaTxObject;
export { DEFAULT_REFERENCE_NFT_V4_DEPLOY_BYTECODE_HEX, DEFAULT_REFERENCE_NFT_V4_RUNTIME_BYTECODE_HEX };
//# sourceMappingURL=referenceNftV4.d.ts.map