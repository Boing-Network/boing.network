/**
 * Versioning + tx-object helpers for **pinned** native Boing deploys (form-parity with EVM apps).
 *
 * See `docs/BOING-CANONICAL-DEPLOY-ARTIFACTS.md`. **Fungible** and **NFT collection** templates ship a
 * pinned default in this package (`DEFAULT_REFERENCE_*`); env vars still override when set.
 */
import { type NativeTokenSecurityFeaturesInput } from './nativeTokenSecurity.js';
/** Logical id for the fungible template line item (docs + telemetry). */
export declare const REFERENCE_FUNGIBLE_TEMPLATE_ARTIFACT_ID: "boing.reference_fungible.v0";
/** Bump when default pinned hex in this package changes. */
export declare const REFERENCE_FUNGIBLE_TEMPLATE_VERSION: "1";
/** Logical id for the secured fungible template (`0xFD` init + runtime toggles). */
export declare const REFERENCE_FUNGIBLE_SECURED_TEMPLATE_ARTIFACT_ID: "boing.reference_fungible_secured.v0";
/** Bump when default pinned secured hex in this package changes. */
export declare const REFERENCE_FUNGIBLE_SECURED_TEMPLATE_VERSION: "1";
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
 * Resolve pinned **secured** fungible deploy bytecode (`0xFD` init + runtime): explicit → env → embedded default.
 */
export declare function resolveReferenceFungibleSecuredTemplateBytecodeHex(opts?: {
    explicitHex?: string | undefined;
    extraEnvKeys?: readonly string[];
}): `0x${string}`;
/**
 * Resolve pinned **reference NFT collection** template bytecode (explicit → env → embedded default,
 * same pattern as {@link resolveReferenceFungibleTemplateBytecodeHex}).
 */
export declare function resolveReferenceNftCollectionTemplateBytecodeHex(opts?: {
    explicitHex?: string | undefined;
    extraEnvKeys?: readonly string[];
}): `0x${string}`;
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
    /**
     * When set and **`descriptionHashHex`** is omitted, commits wizard security toggles into **`description_hash`**
     * (Blake3 over canonical JSON — see {@link descriptionHashHexFromNativeTokenSecurity}).
     */
    nativeTokenSecurity?: NativeTokenSecurityFeaturesInput;
    /** Override pinned template (advanced); default uses {@link resolveReferenceFungibleTemplateBytecodeHex}. */
    bytecodeHexOverride?: string;
    extraEnvKeys?: readonly string[];
};
/**
 * **One call** for wizard-style **Deploy token** on Boing: resolve pinned fungible bytecode + build **`contract_deploy_meta`**.
 * Pass the result to **`boing_sendTransaction`** / **`boing_signTransaction`** (Boing Express).
 */
export declare function buildReferenceFungibleDeployMetaTx(input: BuildReferenceFungibleDeployMetaTxInput): ContractDeployMetaTxObject;
export type BuildReferenceFungibleSecuredDeployMetaTxInput = {
    assetName: string;
    assetSymbol: string;
    purposeCategory?: string;
    descriptionHashHex?: string;
    nativeTokenSecurity?: NativeTokenSecurityFeaturesInput;
    /**
     * When `nativeTokenSecurity` is set, deploy bytecode encodes enforcement flags/limits on-chain.
     * Provide **`chainHeight`** from `boing_chainHeight` when **`timelock`** is enabled.
     */
    chainContext?: {
        chainHeight: bigint;
    };
    /**
     * Initial `mint_first` supply (base units) for deriving **`maxWalletPercentage`** → `max_wallet` cap.
     */
    mintFirstTotalSupplyWei?: bigint;
    bytecodeHexOverride?: string;
    extraEnvKeys?: readonly string[];
};
/**
 * Same as {@link buildReferenceFungibleDeployMetaTx} but uses the secured fungible template.
 * When **`nativeTokenSecurity`** is passed, bytecode is built so wizard toggles map to on-chain
 * `reference_fungible_secured` init storage (not only `description_hash`). When omitted, uses the
 * pinned default secured template (flags off).
 */
export declare function buildReferenceFungibleSecuredDeployMetaTx(input: BuildReferenceFungibleSecuredDeployMetaTxInput): ContractDeployMetaTxObject;
export type BuildReferenceNftCollectionDeployMetaTxInput = {
    collectionName: string;
    collectionSymbol: string;
    purposeCategory?: string;
    descriptionHashHex?: string;
    bytecodeHexOverride?: string;
    extraEnvKeys?: readonly string[];
};
/**
 * **One call** for **native NFT collection** deploy meta tx. Bytecode: **`bytecodeHexOverride`** if set,
 * else env keys (see {@link resolveReferenceNftCollectionTemplateBytecodeHex}), else the SDK embedded default.
 */
export declare function buildReferenceNftCollectionDeployMetaTx(input: BuildReferenceNftCollectionDeployMetaTxInput): ContractDeployMetaTxObject;
/**
 * Resolve **native constant-product pool** bytecode from override or env (same keys as
 * [examples/native-boing-tutorial](../examples/native-boing-tutorial/) **`BOING_NATIVE_AMM_BYTECODE_HEX`**).
 */
export declare function resolveNativeConstantProductPoolBytecodeHex(opts?: {
    explicitHex?: string | undefined;
    extraEnvKeys?: readonly string[];
}): `0x${string}` | undefined;
export type BuildNativeConstantProductPoolDeployMetaTxInput = {
    /** QA / wallet display name (default: `Native CP Pool`). */
    poolLabel?: string;
    /** QA / wallet symbol (default: `POOL`). */
    poolSymbol?: string;
    /**
     * Mempool QA category for pool bytecode ([NATIVE-AMM-CALLDATA.md](../../docs/NATIVE-AMM-CALLDATA.md)).
     * Default **`dapp`** matches canonical testnet pool deploys.
     */
    purposeCategory?: string;
    descriptionHashHex?: string;
    bytecodeHexOverride?: string;
    extraEnvKeys?: readonly string[];
};
/**
 * **One call** for **native CP pool** **`contract_deploy_meta`**: pinned bytecode from env or override,
 * **`purpose_category`** default **`dapp`**, then the same Express shape as token/NFT deploys.
 */
export declare function buildNativeConstantProductPoolDeployMetaTx(input: BuildNativeConstantProductPoolDeployMetaTxInput): ContractDeployMetaTxObject;
//# sourceMappingURL=canonicalDeployArtifacts.d.ts.map