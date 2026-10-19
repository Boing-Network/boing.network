/**
 * On-chain **linked NFT ↔ fungible token** registry (enforced).
 * Matches `boing_execution::linked_nft_token_registry`.
 * See `docs/BOING-LINKED-NFT-TOKEN.md`.
 *
 * **Public testnet** registry AccountId: {@link CANONICAL_BOING_TESTNET_LINKED_NFT_TOKEN_REGISTRY_HEX}
 * (v2, CREATE2 salt `BOING_NFT_TOKEN_LINK_REG_V2`, selectors `0xE0`–`0xE8`).
 *
 * **v2 (current)** — only an asset's creator can claim it (`boing_execution::linked_nft_token_registry_v2`):
 * - the asset's own init code calls `claim_self(claimer)` ({@link SELECTOR_LINKED_NFT_TOKEN_CLAIM_SELF}) inside
 *   the deploy tx, so the creator is the claimer from the first block (see
 *   {@link buildLinkedNftTokenClaimSelfInitSnippetHex});
 * - `claim_asset(asset)` only works for the deployer recorded by the registry's own deploy (assets that
 *   existed when v2 went live), or as a no-op for the current claimer;
 * - failed writes fail the transaction (v1 silently did nothing).
 *
 * v1 ({@link CANONICAL_BOING_TESTNET_LINKED_NFT_TOKEN_REGISTRY_V1_HEX}) let the first caller claim any
 * AccountId. Its claims and links were copied into v2 when v2 was deployed.
 */
import { type ContractDeployMetaTxObject } from './canonicalDeployArtifacts.js';
import type { SimulateResult } from './types.js';
/**
 * Live linked NFT↔token registry on **public Boing testnet** (`https://testnet-rpc.boing.network/`):
 * **v2** (creator-only claims), CREATE2 salt `BOING_NFT_TOKEN_LINK_REG_V2`, seeded with the v1 claims,
 * links and a deployer record for every contract that existed at the copy height. Mainnet: not deployed.
 */
export declare const CANONICAL_BOING_TESTNET_LINKED_NFT_TOKEN_REGISTRY_HEX: string;
/**
 * Previous (v1) testnet registry: first caller of `claim_asset` became the claimer. Read-only history;
 * do not send new claims or links there.
 */
export declare const CANONICAL_BOING_TESTNET_LINKED_NFT_TOKEN_REGISTRY_V1_HEX: string;
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
/** v2: `claim_self(claimer)` — **64** bytes; `CALLER` is the asset (call it from the asset's init code). */
export declare const SELECTOR_LINKED_NFT_TOKEN_CLAIM_SELF = 231;
/** v2: `get_asset_deployer(asset)` — **64** bytes; seeded deployer record (zero if none). */
export declare const SELECTOR_LINKED_NFT_TOKEN_GET_ASSET_DEPLOYER = 232;
export declare const LINKED_NFT_TOKEN_REGISTRY_MAX_LINKS = 4096;
export declare const LINKED_NFT_TOKEN_TOPIC_REGISTER_HEX: string;
export declare const LINKED_NFT_TOKEN_TOPIC_UNLINK_HEX: string;
export declare const LINKED_NFT_TOKEN_REGISTRY_CREATE2_SALT_V1_HEX: string;
/** v2: `Log2` topic0 after a claim (topic1 = asset, data = claimer). */
export declare const LINKED_NFT_TOKEN_TOPIC_CLAIM_V2_HEX: string;
export declare const LINKED_NFT_TOKEN_REGISTRY_CREATE2_SALT_V2_HEX: string;
/** Claimer slot of `asset` in the registry storage (v1 and v2): `asset ^ "BOING_LNK_CLAIMER_V1"`. */
export declare function linkedNftTokenClaimerStorageKeyHex(assetHex32: string): string;
/** v2 deployer-record slot of `asset`: `asset ^ "BOING_LNK_DEPLOYER_V2"`. */
export declare function linkedNftTokenDeployerStorageKeyHex(assetHex32: string): string;
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
export declare function encodeLinkedNftTokenClaimSelfCalldata(claimerHex32: string): Uint8Array;
export declare function encodeLinkedNftTokenClaimSelfCalldataHex(claimerHex32: string): string;
export declare function encodeLinkedNftTokenGetAssetDeployerCalldata(assetHex32: string): Uint8Array;
export declare function encodeLinkedNftTokenGetAssetDeployerCalldataHex(assetHex32: string): string;
/**
 * Init-code snippet (hex, no `0x` marker byte) for an **asset** deploy that runs `0xFD` init code:
 * `CALL registry.claim_self(CALLER)`, so the deploy tx sender becomes the asset's claimer in the same
 * transaction that creates it. Same bytes as `linked_nft_token_registry_v2_claim_self_init_snippet` in
 * `boing_execution`. Uses memory `[0, 64)` and leaves the CALL flag on the stack, so place it before the
 * init writes the runtime to memory. If `registry` has no code the call does nothing.
 */
export declare function buildLinkedNftTokenClaimSelfInitSnippetHex(registryHex32: string): string;
/** v2 runtime bytecode (a plain deploy of it is an empty v2 registry for a new network). */
export declare function resolveLinkedNftTokenRegistryV2BytecodeHex(opts?: {
    explicitHex?: string;
}): `0x${string}`;
/** Deploy an empty v2 registry (salt `BOING_NFT_TOKEN_LINK_REG_V2`). */
export declare function buildLinkedNftTokenRegistryV2DeployMetaTx(input?: {
    assetName?: string;
    assetSymbol?: string;
    purposeCategory?: string;
    descriptionHashHex?: string;
    create2SaltHex?: string;
    bytecodeHexOverride?: string;
}): ContractDeployMetaTxObject;
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
 *
 * With `includeClaims: false` only `register_link` is returned: use it when both assets claimed
 * themselves at deploy (v2 `claim_self` in their init code). On v2 a `claim_asset` by the current
 * claimer is a no-op, so the default three-tx flow still works for such assets.
 */
export declare function buildLinkedNftTokenRegisterFlowTxs(input: {
    senderHex32: string;
    registryHex32: string;
    collectionHex32: string;
    tokenHex32: string;
    includeClaims?: boolean;
}): Array<ReturnType<typeof buildLinkedNftTokenRegistryContractCallTx>>;
//# sourceMappingURL=linkedNftTokenRegistry.d.ts.map