/**
 * Versioning + tx-object helpers for **pinned** native Boing deploys (form-parity with EVM apps).
 *
 * See `docs/BOING-CANONICAL-DEPLOY-ARTIFACTS.md`. Full **fungible / NFT collection** bytecode
 * ships from `boing-execution` for **NFT collections** (`REFERENCE_NFT_COLLECTION_TEMPLATE_VERSION`);
 * the **fungible** template ships a pinned default (`DEFAULT_REFERENCE_FUNGIBLE_TEMPLATE_BYTECODE_HEX`).
 */
/** Logical id for the fungible template line item (docs + telemetry). */
export declare const REFERENCE_FUNGIBLE_TEMPLATE_ARTIFACT_ID: "boing.reference_fungible.v0";
/** Bump when default pinned hex in this package changes. */
export declare const REFERENCE_FUNGIBLE_TEMPLATE_VERSION: "1";
/** Logical id for the NFT collection template. */
export declare const REFERENCE_NFT_COLLECTION_TEMPLATE_ARTIFACT_ID: "boing.reference_nft_collection.v0";
/** Matches `reference_nft_collection_template_bytecode()` in `boing-execution` (regenerate via `dump_reference_token_artifacts`). */
export declare const REFERENCE_NFT_COLLECTION_TEMPLATE_VERSION: "1";
/** Boing Express / wallet JSON-RPC tx object for `contract_deploy_meta`. */
export type ContractDeployMetaTxObject = {
    type: 'contract_deploy_meta';
    bytecode: `0x${string}`;
    purpose_category: string;
    asset_name: string;
    asset_symbol: string;
    description_hash?: `0x${string}`;
};
/**
 * Normalize hex for wallet RPC payloads (`0x` prefix). Use for deploy bytecode or `description_hash`.
 */
export declare function ensure0xHex(hex: string): `0x${string}`;
/**
 * Resolve pinned fungible template bytecode: explicit override → known env keys → embedded default
 * (`DEFAULT_REFERENCE_FUNGIBLE_TEMPLATE_BYTECODE_HEX`).
 */
export declare function resolveReferenceFungibleTemplateBytecodeHex(opts?: {
    explicitHex?: string | undefined;
    /** Extra env var names to try after `explicitHex` (`globalThis.process.env` when present). */
    extraEnvKeys?: readonly string[];
}): `0x${string}`;
/**
 * Resolve pinned **reference NFT collection** template bytecode (same pattern as fungible).
 */
export declare function resolveReferenceNftCollectionTemplateBytecodeHex(opts?: {
    explicitHex?: string | undefined;
    extraEnvKeys?: readonly string[];
}): `0x${string}` | undefined;
/**
 * Build a **`contract_deploy_meta`** object for `boing_sendTransaction` / `boing_signTransaction`.
 */
export declare function buildContractDeployMetaTx(input: {
    bytecodeHex: string;
    assetName: string;
    assetSymbol: string;
    /** Default `token` matches Express convenience when name/symbol are set. */
    purposeCategory?: string;
    descriptionHashHex?: string;
}): ContractDeployMetaTxObject;
export type BuildReferenceFungibleDeployMetaTxInput = {
    assetName: string;
    assetSymbol: string;
    purposeCategory?: string;
    descriptionHashHex?: string;
    /** Override pinned template (advanced); default uses {@link resolveReferenceFungibleTemplateBytecodeHex}. */
    bytecodeHexOverride?: string;
    extraEnvKeys?: readonly string[];
};
/**
 * **One call** for wizard-style **Deploy token** on Boing: resolve pinned fungible bytecode + build **`contract_deploy_meta`**.
 * Pass the result to **`boing_sendTransaction`** / **`boing_signTransaction`** (Boing Express).
 */
export declare function buildReferenceFungibleDeployMetaTx(input: BuildReferenceFungibleDeployMetaTxInput): ContractDeployMetaTxObject;
export type BuildReferenceNftCollectionDeployMetaTxInput = {
    collectionName: string;
    collectionSymbol: string;
    purposeCategory?: string;
    descriptionHashHex?: string;
    bytecodeHexOverride?: string;
    extraEnvKeys?: readonly string[];
};
/**
 * **One call** for **native NFT collection** deploy meta tx. Requires pinned collection bytecode
 * (env or **`bytecodeHexOverride`**); throws a clear error if unresolved — same constraint as manual **`resolve` + `build`**.
 */
export declare function buildReferenceNftCollectionDeployMetaTx(input: BuildReferenceNftCollectionDeployMetaTxInput): ContractDeployMetaTxObject;
//# sourceMappingURL=canonicalDeployArtifacts.d.ts.map