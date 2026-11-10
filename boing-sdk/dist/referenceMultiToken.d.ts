/**
 * Reference **multi-token** (ERC-1155-style shared item types) — calldata, storage keys, log decoding and
 * balance reads. Mirrors `crates/boing-execution/src/reference_multi_token.rs`; spec:
 * `docs/BOING-REFERENCE-MULTI-TOKEN.md`.
 *
 * One contract holds many item types; every holder of item `7` holds the same item. Item ids are u64,
 * amounts are u128. No hashing opcode is needed: slot keys are XOR-tagged words.
 */
import { type ContractDeployMetaTxObject } from './canonicalDeployArtifacts.js';
import { DEFAULT_REFERENCE_MULTI_TOKEN_DEPLOY_BYTECODE_HEX, DEFAULT_REFERENCE_MULTI_TOKEN_RUNTIME_BYTECODE_HEX } from './defaultReferenceMultiTokenBytecodeHex.js';
export declare const SELECTOR_MT_BALANCE_OF = 32;
export declare const SELECTOR_MT_TRANSFER = 33;
export declare const SELECTOR_MT_MINT = 34;
export declare const SELECTOR_MT_SET_ITEM_METADATA = 35;
export declare const SELECTOR_MT_MINT_BATCH = 36;
export declare const SELECTOR_MT_TRANSFER_BATCH = 37;
export declare const SELECTOR_MT_TOTAL_SUPPLY = 38;
export declare const SELECTOR_MT_ITEM_METADATA = 39;
export declare const SELECTOR_MT_ADMIN = 40;
export declare const SELECTOR_MT_TRANSFER_ADMIN = 41;
/** Max entries per `mint_batch` / `transfer_batch` (one log per entry; VM cap 24 logs per tx). */
export declare const MAX_REFERENCE_MULTI_TOKEN_BATCH = 16;
/** Logical artifact id + pinned template version. */
export declare const REFERENCE_MULTI_TOKEN_ARTIFACT_ID: "boing.reference_multi_token.v1";
export declare const REFERENCE_MULTI_TOKEN_TEMPLATE_VERSION: "1";
/** Balance slot XOR tag (`BOING_REFMT_BAL_V1`). */
export declare const REF_MT_BALANCE_XOR_HEX: string;
/** Total-supply slot XOR tag (`BOING_REFMT_SUPPLY_V1`). */
export declare const REF_MT_SUPPLY_XOR_HEX: string;
/** Item metadata slot XOR tag (`BOING_REFMT_META_V1`). */
export declare const REF_MT_METADATA_XOR_HEX: string;
/** `LOG3` topic0 for balance moves: `[topic, from, to]`, data `item_word || amount_word` (mint: from = 0). */
export declare const REF_MT_TOPIC_TRANSFER_HEX: string;
/** `LOG2` topic0 for `set_item_metadata`: `[topic, item_word]`, data = hash. */
export declare const REF_MT_TOPIC_METADATA_HEX: string;
/** `LOG3` topic0 for `transfer_admin`: `[topic, old_admin, new_admin]`. */
export declare const REF_MT_TOPIC_ADMIN_HEX: string;
/** Admin slot (set to the deployer by the init frame). */
export declare const REF_MT_ADMIN_KEY_HEX: string;
/** Item id word (big-endian u64 in the low 8 bytes). */
export declare function multiTokenItemIdWord(itemId: bigint | number): string;
/** Amount word (big-endian u128 in the low 16 bytes). */
export declare function multiTokenAmountWord(amount: bigint | number | string): string;
/** `SLOAD` key for `balance[itemId][account]`: `(account[0..24] || itemId_be8) ^ REF_MT_BALANCE_XOR`. */
export declare function multiTokenBalanceStorageKey(accountHex32: string, itemId: bigint | number): string;
/** `SLOAD` key for `total_supply[itemId]`. */
export declare function multiTokenSupplyStorageKey(itemId: bigint | number): string;
/** `SLOAD` key for `item_metadata[itemId]`. */
export declare function multiTokenMetadataStorageKey(itemId: bigint | number): string;
/** 96-byte `balance_of(account, itemId)`. */
export declare function encodeMultiTokenBalanceOfCalldata(accountHex32: string, itemId: bigint | number): Uint8Array;
/** 128-byte `transfer(to, itemId, amount)` (caller's own balance). */
export declare function encodeMultiTokenTransferCalldata(toHex32: string, itemId: bigint | number, amount: bigint | number | string): Uint8Array;
/** 128-byte `mint(to, itemId, amount)` (admin only). */
export declare function encodeMultiTokenMintCalldata(toHex32: string, itemId: bigint | number, amount: bigint | number | string): Uint8Array;
/** 96-byte `set_item_metadata(itemId, hash)` (admin only). */
export declare function encodeMultiTokenSetItemMetadataCalldata(itemId: bigint | number, hashHex32: string): Uint8Array;
export type MultiTokenBatchEntry = {
    itemId: bigint | number;
    amount: bigint | number | string;
};
/** `mint_batch(to, n, itemIds[n], amounts[n])` — `96 + 64n` bytes (admin only). */
export declare function encodeMultiTokenMintBatchCalldata(toHex32: string, entries: readonly MultiTokenBatchEntry[]): Uint8Array;
/** `transfer_batch(to, n, itemIds[n], amounts[n])` — `96 + 64n` bytes (caller's balances). */
export declare function encodeMultiTokenTransferBatchCalldata(toHex32: string, entries: readonly MultiTokenBatchEntry[]): Uint8Array;
/** 64-byte `total_supply(itemId)`. */
export declare function encodeMultiTokenTotalSupplyCalldata(itemId: bigint | number): Uint8Array;
/** 64-byte `item_metadata(itemId)`. */
export declare function encodeMultiTokenItemMetadataCalldata(itemId: bigint | number): Uint8Array;
/** 64-byte `transfer_admin(newAdmin)` (admin only). */
export declare function encodeMultiTokenTransferAdminCalldata(newAdminHex32: string): Uint8Array;
export type DecodedMultiTokenCall = {
    selector: typeof SELECTOR_MT_BALANCE_OF;
    kind: 'balance_of';
    account: string;
    itemId: bigint;
} | {
    selector: typeof SELECTOR_MT_TRANSFER;
    kind: 'transfer';
    to: string;
    itemId: bigint;
    amount: bigint;
} | {
    selector: typeof SELECTOR_MT_MINT;
    kind: 'mint';
    to: string;
    itemId: bigint;
    amount: bigint;
} | {
    selector: typeof SELECTOR_MT_SET_ITEM_METADATA;
    kind: 'set_item_metadata';
    itemId: bigint;
    metadataHash: string;
} | {
    selector: typeof SELECTOR_MT_MINT_BATCH | typeof SELECTOR_MT_TRANSFER_BATCH;
    kind: 'mint_batch' | 'transfer_batch';
    to: string;
    entries: {
        itemId: bigint;
        amount: bigint;
    }[];
} | {
    selector: typeof SELECTOR_MT_TOTAL_SUPPLY;
    kind: 'total_supply';
    itemId: bigint;
} | {
    selector: typeof SELECTOR_MT_ITEM_METADATA;
    kind: 'item_metadata';
    itemId: bigint;
} | {
    selector: typeof SELECTOR_MT_ADMIN;
    kind: 'admin';
} | {
    selector: typeof SELECTOR_MT_TRANSFER_ADMIN;
    kind: 'transfer_admin';
    newAdmin: string;
};
/**
 * Best-effort decode of reference multi-token calldata (wallet approval copy, explorers). Returns null when the
 * selector or layout does not match. Values that the contract would reject (u64/u128 overflow) also return null.
 */
export declare function decodeMultiTokenCalldata(calldata: string | Uint8Array): DecodedMultiTokenCall | null;
/** One decoded balance move from a receipt log (mint when `from` is the zero account). */
export type MultiTokenTransferLog = {
    from: string;
    to: string;
    itemId: bigint;
    amount: bigint;
    isMint: boolean;
};
/** Decode a `{ topics, data }` receipt log emitted by the template; null for other logs. */
export declare function decodeMultiTokenTransferLog(log: {
    topics: readonly string[];
    data: string | Uint8Array;
}): MultiTokenTransferLog | null;
/** Parse a `boing_getContractStorage` result (`{ value }` or bare hex) as a u128 amount. */
export declare function multiTokenAmountFromStorageWord(word: unknown): bigint;
type StorageReader = {
    getContractStorage(contractHex32: string, keyHex32: string): Promise<unknown>;
};
/** Read `balance[itemId][account]` via `boing_getContractStorage` (no tx, no simulation). */
export declare function fetchMultiTokenBalance(client: StorageReader, contractHex32: string, accountHex32: string, itemId: bigint | number): Promise<bigint>;
/** Read balances for several item ids in parallel. */
export declare function fetchMultiTokenBalances(client: StorageReader, contractHex32: string, accountHex32: string, itemIds: readonly (bigint | number)[]): Promise<{
    itemId: bigint;
    balance: bigint;
}[]>;
/** Read `total_supply[itemId]` and the type-level metadata hash. */
export declare function fetchMultiTokenItemInfo(client: StorageReader, contractHex32: string, itemId: bigint | number): Promise<{
    itemId: bigint;
    totalSupply: bigint;
    metadataHash: string;
}>;
/** True when `codeHex` is the pinned multi-token runtime (or deploy payload) — wallet / explorer identity. */
export declare function isReferenceMultiTokenBytecode(codeHex: string): boolean;
export type BuildReferenceMultiTokenDeployMetaTxInput = {
    name: string;
    symbol: string;
    descriptionHashHex?: string;
};
/** Express `contract_deploy_meta` tx for the pinned multi-token (`0xFD` init; admin = deployer; purpose `nft`). */
export declare function buildReferenceMultiTokenDeployMetaTx(input: BuildReferenceMultiTokenDeployMetaTxInput): ContractDeployMetaTxObject;
export { DEFAULT_REFERENCE_MULTI_TOKEN_DEPLOY_BYTECODE_HEX, DEFAULT_REFERENCE_MULTI_TOKEN_RUNTIME_BYTECODE_HEX };
//# sourceMappingURL=referenceMultiToken.d.ts.map