/**
 * On-chain **linked NFT ↔ fungible token** registry (enforced).
 * Matches `boing_execution::linked_nft_token_registry`.
 * See `docs/BOING-LINKED-NFT-TOKEN.md`.
 */
import { type ContractDeployMetaTxObject } from './canonicalDeployArtifacts.js';
import type { SimulateResult } from './types.js';
/** `claim_asset(asset)` — **64** bytes. */
export declare const SELECTOR_LINKED_NFT_TOKEN_CLAIM_ASSET = 224;
/** `register_link(collection, token)` — **96** bytes. */
export declare const SELECTOR_LINKED_NFT_TOKEN_REGISTER_LINK = 225;
/** `unlink_at(index)` — **64** bytes. */
export declare const SELECTOR_LINKED_NFT_TOKEN_UNLINK_AT = 226;
/** `links_count` — **32** bytes. */
export declare const SELECTOR_LINKED_NFT_TOKEN_LINKS_COUNT = 227;
/** `get_link_at(index)` — **64** bytes; returns **64** bytes. */
export declare const SELECTOR_LINKED_NFT_TOKEN_GET_LINK_AT = 228;
/** `get_asset_claimer(asset)` — **64** bytes. */
export declare const SELECTOR_LINKED_NFT_TOKEN_GET_ASSET_CLAIMER = 229;
/** `transfer_asset_claimer(asset, new_claimer)` — **96** bytes. */
export declare const SELECTOR_LINKED_NFT_TOKEN_TRANSFER_ASSET_CLAIMER = 230;
export declare const LINKED_NFT_TOKEN_REGISTRY_MAX_LINKS = 4096;
export declare const LINKED_NFT_TOKEN_TOPIC_REGISTER_HEX: string;
export declare const LINKED_NFT_TOKEN_TOPIC_UNLINK_HEX: string;
export declare const LINKED_NFT_TOKEN_REGISTRY_CREATE2_SALT_V1_HEX: string;
/** Count storage key (`linked_nft_token_registry_count_key`). */
export declare const LINKED_NFT_TOKEN_REGISTRY_COUNT_KEY_HEX: `0x${string}424f494e474c4e4b00000000ffffffff`;
export declare function encodeLinkedNftTokenClaimAssetCalldata(assetHex32: string): Uint8Array;
export declare function encodeLinkedNftTokenClaimAssetCalldataHex(assetHex32: string): string;
export declare function encodeLinkedNftTokenRegisterLinkCalldata(collectionHex32: string, tokenHex32: string): Uint8Array;
export declare function encodeLinkedNftTokenRegisterLinkCalldataHex(collectionHex32: string, tokenHex32: string): string;
export declare function encodeLinkedNftTokenUnlinkAtCalldata(index: number): Uint8Array;
export declare function encodeLinkedNftTokenUnlinkAtCalldataHex(index: number): string;
export declare function encodeLinkedNftTokenLinksCountCalldata(): Uint8Array;
export declare function encodeLinkedNftTokenLinksCountCalldataHex(): string;
export declare function encodeLinkedNftTokenGetLinkAtCalldata(index: number): Uint8Array;
export declare function encodeLinkedNftTokenGetLinkAtCalldataHex(index: number): string;
export declare function encodeLinkedNftTokenGetAssetClaimerCalldata(assetHex32: string): Uint8Array;
export declare function encodeLinkedNftTokenGetAssetClaimerCalldataHex(assetHex32: string): string;
export declare function encodeLinkedNftTokenTransferAssetClaimerCalldata(assetHex32: string, newClaimerHex32: string): Uint8Array;
export declare function encodeLinkedNftTokenTransferAssetClaimerCalldataHex(assetHex32: string, newClaimerHex32: string): string;
export declare function resolveLinkedNftTokenRegistryBytecodeHex(opts?: {
    explicitHex?: string;
}): `0x${string}`;
export declare function buildLinkedNftTokenRegistryDeployMetaTx(input?: {
    assetName?: string;
    assetSymbol?: string;
    purposeCategory?: string;
    descriptionHashHex?: string;
    create2SaltHex?: string;
    bytecodeHexOverride?: string;
}): ContractDeployMetaTxObject;
export declare function buildLinkedNftTokenRegistryAccessList(senderHex32: string, registryHex32: string): {
    read: string[];
    write: string[];
};
export declare function buildLinkedNftTokenRegistryContractCallTx(senderHex32: string, registryHex32: string, calldataHex: string): {
    type: 'contract_call';
    contract: string;
    calldata: string;
    access_list: {
        read: string[];
        write: string[];
    };
};
export declare function mergeLinkedNftTokenRegistryAccessListWithSimulation(senderHex32: string, registryHex32: string, sim: SimulateResult): {
    read: string[];
    write: string[];
};
export declare function decodeLinkedNftTokenLinksCountReturnData(returnDataHex: string): bigint;
export declare function decodeLinkedNftTokenGetLinkAtReturnData(returnDataHex: string): {
    collectionHex: string;
    tokenHex: string;
    /** False when both ids are zero (tombstone / unlinked slot). */
    active: boolean;
};
/**
 * Build Express `contract_call` txs to claim both assets and register one edge.
 * Caller must be (or become) claimer of both AccountIds — typically the joint deployer.
 */
export declare function buildLinkedNftTokenRegisterFlowTxs(input: {
    senderHex32: string;
    registryHex32: string;
    collectionHex32: string;
    tokenHex32: string;
}): Array<ReturnType<typeof buildLinkedNftTokenRegistryContractCallTx>>;
//# sourceMappingURL=linkedNftTokenRegistry.d.ts.map