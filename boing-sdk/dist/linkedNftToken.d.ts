/**
 * Off-chain **cache helpers** for linked NFT collection ↔ fungible token pairs.
 *
 * **Source of truth** is the on-chain registry (`linkedNftTokenRegistry.ts` /
 * `boing_execution::linked_nft_token_registry`). Schema **`boing.linked_nft_token.v1`** and
 * companion JSON keys are optional indexer/UI caches only — not authoritative.
 *
 * See `docs/BOING-LINKED-NFT-TOKEN.md`.
 */
import { type ContractDeployMetaTxObject } from './canonicalDeployArtifacts.js';
import type { NativeTokenSecurityFeaturesInput } from './nativeTokenSecurity.js';
/** Canonical schema id committed into `description_hash` / off-chain JSON. */
export declare const LINKED_NFT_TOKEN_SCHEMA: "boing.linked_nft_token.v1";
/** Domain label for derived CREATE2 salt (collection salt → token salt). */
export declare const LINKED_NFT_TOKEN_SALT_DOMAIN: "boing.nft_token_pair.v1";
/** Full schema object key in off-chain NFT / project JSON. */
export declare const LINKED_NFT_TOKEN_OFFCHAIN_KEY: "linked_nft_token";
/** Convenience peer list on NFT / project metadata (AccountId hex strings). */
export declare const COMPANION_TOKENS_OFFCHAIN_KEY: "companion_tokens";
/** Convenience peer list on fungible / project metadata (AccountId hex strings). */
export declare const COMPANION_COLLECTIONS_OFFCHAIN_KEY: "companion_collections";
export type LinkedNftTokenRole = 'nft_collection' | 'fungible_token';
export type LinkedNftTokenInput = {
    role: LinkedNftTokenRole;
    /** Peer AccountIds (other side of the link). Many-to-many: no hard cap. */
    peers?: readonly string[];
    /** Optional self AccountId (this contract). */
    self?: string;
    /** Preferred same-deployer / attester hint for soft-gate (AccountId). */
    attester?: string;
    /** Bump when mutating peers after create (off-chain). */
    revision?: number;
    note?: string;
};
/** Canonical JSON-ready object (fixed key order for stable hashing). */
export type LinkedNftTokenNormalized = {
    schema: typeof LINKED_NFT_TOKEN_SCHEMA;
    role: LinkedNftTokenRole;
    self: string;
    peers: string[];
    attester: string;
    revision: number;
    note: string;
};
/**
 * Stable object for hashing / logging (explicit key order).
 * Peers are lowercased, deduped, and sorted.
 */
export declare function normalizeLinkedNftToken(input: LinkedNftTokenInput): LinkedNftTokenNormalized;
/** UTF-8 JSON of the normalized document (canonical key order). */
export declare function encodeLinkedNftTokenJson(input: LinkedNftTokenInput): string;
/**
 * Decode a `boing.linked_nft_token.v1` JSON string or object.
 * @throws if schema/role invalid or peer ids malformed.
 */
export declare function decodeLinkedNftTokenJson(json: string | Record<string, unknown>): LinkedNftTokenNormalized;
/**
 * Blake3-256 digest of UTF-8 canonical JSON, as **`0x` + 64 hex** (fits `description_hash`).
 */
export declare function descriptionHashHexFromLinkedNftToken(input: LinkedNftTokenInput): `0x${string}`;
/** Attach one peer AccountId; increments `revision` by default. */
export declare function attachLinkedNftTokenPeer(doc: LinkedNftTokenInput, peerHex: string, opts?: {
    bumpRevision?: boolean;
}): LinkedNftTokenNormalized;
/** Replace the full peer list (many-to-many update). */
export declare function updateLinkedNftTokenPeers(doc: LinkedNftTokenInput, peers: readonly string[], opts?: {
    bumpRevision?: boolean;
}): LinkedNftTokenNormalized;
/** Remove one peer; no-op if absent (still bumps revision when `bumpRevision`). */
export declare function unlinkLinkedNftTokenPeer(doc: LinkedNftTokenInput, peerHex: string, opts?: {
    bumpRevision?: boolean;
}): LinkedNftTokenNormalized;
/**
 * Build / merge off-chain JSON keys for display clients.
 * Writes full schema under {@link LINKED_NFT_TOKEN_OFFCHAIN_KEY} and convenience peer arrays.
 */
export declare function applyLinkedNftTokenOffchainKeys(metadata: Record<string, unknown>, doc: LinkedNftTokenInput): Record<string, unknown>;
/**
 * Read link document from off-chain metadata (full key preferred; falls back to convenience arrays).
 */
export declare function readLinkedNftTokenFromOffchainMetadata(metadata: Record<string, unknown>, roleHint?: LinkedNftTokenRole): LinkedNftTokenNormalized | null;
/** 32 random bytes as `0x` + 64 hex (collection CREATE2 salt). */
export declare function randomLinkedNftTokenCollectionSaltHex(): `0x${string}`;
/**
 * Token CREATE2 salt: `BLAKE3("boing.nft_token_pair.v1" ‖ collectionSalt32)`.
 */
export declare function deriveLinkedNftTokenFungibleSaltHex(collectionSaltHex: string): `0x${string}`;
export type LinkedNftTokenPairSalts = {
    collectionSaltHex: `0x${string}`;
    tokenSaltHex: `0x${string}`;
};
export declare function linkedNftTokenPairSalts(collectionSaltHex?: string): LinkedNftTokenPairSalts;
export type LinkedPeerSoftGateInput = {
    /** Deployer of the subject contract (NFT or token). */
    subjectDeployerHex?: string;
    /** Deployer of the claimed peer. */
    peerDeployerHex?: string;
    /** Optional attester from the link document. */
    attesterHex?: string;
    /**
     * When true, mismatch → `ok: false`. When false (default), mismatch still returns `ok: true`
     * with `spoofRisk: true` (soft prefer).
     */
    requireMatch?: boolean;
};
export type LinkedPeerSoftGateResult = {
    ok: boolean;
    spoofRisk: boolean;
    matched: boolean;
    reason: string;
};
/**
 * Soft-gate for display-only links: prefer same deployer / attester.
 * Does **not** prove authenticity on-chain — document spoof risk to UI consumers.
 */
export declare function softGateLinkedNftTokenPeers(input: LinkedPeerSoftGateInput): LinkedPeerSoftGateResult;
export type BuildLinkedNftTokenPairDeploysInput = {
    /** Deployer AccountId (CREATE2 predictor + link attester). */
    deployerHex: string;
    collectionName: string;
    collectionSymbol: string;
    tokenName: string;
    tokenSymbol: string;
    /** Optional 32-byte collection CREATE2 salt; random when omitted. Token salt is derived. */
    collectionSaltHex?: string;
    /** When true, use secured fungible template (+ optional `nativeTokenSecurity`). */
    securedFungible?: boolean;
    nativeTokenSecurity?: NativeTokenSecurityFeaturesInput;
    chainContext?: {
        chainHeight: bigint;
    };
    mintFirstTotalSupplyWei?: bigint;
    note?: string;
    collectionBytecodeHexOverride?: string;
    tokenBytecodeHexOverride?: string;
    collectionExtraEnvKeys?: readonly string[];
    tokenExtraEnvKeys?: readonly string[];
    collectionPurposeCategory?: string;
    tokenPurposeCategory?: string;
};
export type LinkedNftTokenPairDeploys = {
    collectionSaltHex: `0x${string}`;
    tokenSaltHex: `0x${string}`;
    predictedCollectionAddress: `0x${string}`;
    predictedTokenAddress: `0x${string}`;
    collectionLink: LinkedNftTokenNormalized;
    tokenLink: LinkedNftTokenNormalized;
    collectionDeployTx: ContractDeployMetaTxObject;
    tokenDeployTx: ContractDeployMetaTxObject;
};
/**
 * Joint deploy helper: predict both CREATE2 addresses, emit **two** `contract_deploy_meta` txs
 * (NFT + fungible) with mutual `description_hash` commits and documented salts.
 *
 * Submit order is up to the dApp (either first); both hashes already name both addresses.
 * Handle partial success in UI (one of two mempool/QA deploys may land alone).
 */
export declare function buildLinkedNftTokenPairDeploys(input: BuildLinkedNftTokenPairDeploysInput): LinkedNftTokenPairDeploys;
//# sourceMappingURL=linkedNftToken.d.ts.map