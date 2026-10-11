/**
 * On-chain **builder attestation** registry (enforced builder signal).
 * Matches `boing_execution::builder_attestation_registry`.
 * See `docs/BOING-BUILDER-ATTESTATION.md`.
 *
 * CREATE2 salt `BOING_BUILDER_ATTEST_REG_V1`, selectors `0xE0`–`0xE6`.
 * Canonical testnet AccountId: set after first public deploy (env-pin apps).
 */
import { type ContractDeployMetaTxObject } from './canonicalDeployArtifacts.js';
import type { SimulateResult } from './types.js';
/** `claim_asset(asset)` — **64** bytes. */
export declare const SELECTOR_BUILDER_ATTEST_CLAIM_ASSET = 224;
/** `attest(collection, token, note_hash)` — **128** bytes. */
export declare const SELECTOR_BUILDER_ATTEST_ATTEST = 225;
/** `revoke_at(index)` — **64** bytes. */
export declare const SELECTOR_BUILDER_ATTEST_REVOKE_AT = 226;
/** `attestations_count` — **32** bytes. */
export declare const SELECTOR_BUILDER_ATTEST_COUNT = 227;
/** `get_attestation_at(index)` — **64** bytes; returns **128** bytes. */
export declare const SELECTOR_BUILDER_ATTEST_GET_AT = 228;
/** `get_asset_claimer(asset)` — **64** bytes. */
export declare const SELECTOR_BUILDER_ATTEST_GET_CLAIMER = 229;
/** `transfer_asset_claimer(asset, new_claimer)` — **96** bytes. */
export declare const SELECTOR_BUILDER_ATTEST_TRANSFER_CLAIMER = 230;
export declare const BUILDER_ATTESTATION_REGISTRY_MAX_SLOTS = 4096;
export declare const BUILDER_ATTEST_TOPIC_ATTEST_HEX: string;
export declare const BUILDER_ATTEST_TOPIC_REVOKE_HEX: string;
export declare const BUILDER_ATTESTATION_REGISTRY_CREATE2_SALT_V1_HEX: string;
/** Count storage key (`builder_attestation_registry_count_key`). */
export declare const BUILDER_ATTESTATION_REGISTRY_COUNT_KEY_HEX: `0x${string}424f494e4741545400000000ffffffff`;
export declare function encodeBuilderAttestClaimAssetCalldata(assetHex32: string): Uint8Array;
export declare function encodeBuilderAttestClaimAssetCalldataHex(assetHex32: string): string;
export declare function encodeBuilderAttestAttestCalldata(collectionHex32: string, tokenHex32: string, noteHashHex32?: string): Uint8Array;
export declare function encodeBuilderAttestAttestCalldataHex(collectionHex32: string, tokenHex32: string, noteHashHex32?: string): string;
export declare function encodeBuilderAttestRevokeAtCalldata(index: number): Uint8Array;
export declare function encodeBuilderAttestRevokeAtCalldataHex(index: number): string;
export declare function encodeBuilderAttestCountCalldata(): Uint8Array;
export declare function encodeBuilderAttestCountCalldataHex(): string;
export declare function encodeBuilderAttestGetAtCalldata(index: number): Uint8Array;
export declare function encodeBuilderAttestGetAtCalldataHex(index: number): string;
export declare function encodeBuilderAttestGetClaimerCalldata(assetHex32: string): Uint8Array;
export declare function encodeBuilderAttestGetClaimerCalldataHex(assetHex32: string): string;
export declare function encodeBuilderAttestTransferClaimerCalldata(assetHex32: string, newClaimerHex32: string): Uint8Array;
export declare function encodeBuilderAttestTransferClaimerCalldataHex(assetHex32: string, newClaimerHex32: string): string;
export declare function decodeBuilderAttestCountReturnData(returnDataHex: string): bigint;
export declare function decodeBuilderAttestGetAtReturnData(returnDataHex: string): {
    collectionHex: string;
    tokenHex: string;
    builderHex: string;
    noteHashHex: string;
    /** False when collection is zero (tombstone / empty slot). */
    active: boolean;
};
export declare function resolveBuilderAttestationRegistryBytecodeHex(opts?: {
    explicitHex?: string;
}): `0x${string}`;
export declare function buildBuilderAttestationRegistryDeployMetaTx(input?: {
    assetName?: string;
    assetSymbol?: string;
    purposeCategory?: string;
    descriptionHashHex?: string;
    create2SaltHex?: string;
    bytecodeHexOverride?: string;
}): ContractDeployMetaTxObject;
export declare function buildBuilderAttestationRegistryAccessList(senderHex32: string, registryHex32: string): {
    read: string[];
    write: string[];
};
export declare function buildBuilderAttestationRegistryContractCallTx(senderHex32: string, registryHex32: string, calldataHex: string): {
    type: 'contract_call';
    contract: string;
    calldata: string;
    access_list: {
        read: string[];
        write: string[];
    };
};
export declare function mergeBuilderAttestationRegistryAccessListWithSimulation(senderHex32: string, registryHex32: string, sim: SimulateResult): {
    read: string[];
    write: string[];
};
/**
 * Build Express `contract_call` txs to claim both assets and attest once.
 * Caller must be (or become) claimer of both AccountIds — typically the joint deployer.
 * Companion links are app policy only (not checked on-chain).
 */
export declare function buildBuilderAttestationAttestFlowTxs(input: {
    senderHex32: string;
    registryHex32: string;
    collectionHex32: string;
    tokenHex32: string;
    noteHashHex32?: string;
}): Array<ReturnType<typeof buildBuilderAttestationRegistryContractCallTx>>;
//# sourceMappingURL=builderAttestationRegistry.d.ts.map