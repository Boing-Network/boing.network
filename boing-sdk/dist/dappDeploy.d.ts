/**
 * dApp deploy helpers: QA preflight for **`contract_deploy_meta`** payloads (wizard “review” steps).
 *
 * See [BOING-CANONICAL-DEPLOY-ARTIFACTS.md](../../docs/BOING-CANONICAL-DEPLOY-ARTIFACTS.md),
 * [BOING-DAPP-INTEGRATION.md](../../docs/BOING-DAPP-INTEGRATION.md).
 */
import type { BoingClient } from './client.js';
import type { BuildNativeConstantProductPoolDeployMetaTxInput, BuildReferenceFungibleDeployMetaTxInput, BuildReferenceNftCollectionDeployMetaTxInput, ContractDeployMetaTxObject } from './canonicalDeployArtifacts.js';
import type { QaCheckResponse, QaCheckResult } from './types.js';
/**
 * When **`boing_qaCheck`** needs **`asset_name` / `asset_symbol`** but the wizard has no description
 * commitment yet, the RPC allows a **placeholder** 32-byte hash ([RPC-API-SPEC.md](../../docs/RPC-API-SPEC.md) § **boing_qaCheck**).
 */
export declare const BOING_QA_PLACEHOLDER_DESCRIPTION_HASH_HEX: `0x${string}`;
/**
 * Run **`boing_qaCheck`** using the same fields as a **`contract_deploy_meta`** tx object.
 * Use on the **review** step of a deploy wizard so users see **allow / reject / unsure** before signing.
 */
export declare function preflightContractDeployMetaQa(client: BoingClient, tx: ContractDeployMetaTxObject): Promise<QaCheckResponse>;
/** UI hints for a **`boing_qaCheck`** result (toasts, banners, review step). */
export type DeployMetaQaUiOutcome = {
    result: QaCheckResult;
    /**
     * Whether the primary action (“Sign” / “Deploy”) should stay enabled.
     * For **`unsure`**, signing may still be valid (pool path); surface {@link DeployMetaQaUiOutcome.detail} first.
     */
    readyToSign: boolean;
    /** Short line for banners / toasts. */
    headline: string;
    /** Longer copy for modals or inline help. */
    detail: string;
    /** Suggested semantic tone for styling. */
    tone: 'positive' | 'neutral' | 'warning' | 'destructive';
};
/**
 * Map a **`boing_qaCheck`** response to user-facing copy (preflight or post-error messaging).
 * Use with **`preflightContractDeployMetaQa`** on the review step, or after submit failures parsed as QA.
 */
export declare function describeContractDeployMetaQaResponse(qa: QaCheckResponse): DeployMetaQaUiOutcome;
export type PreflightContractDeployMetaWithUiResult = {
    qa: QaCheckResponse;
    ui: DeployMetaQaUiOutcome;
};
/**
 * Run **`preflightContractDeployMetaQa`** and attach {@link describeContractDeployMetaQaResponse} for one-shot UI wiring.
 */
export declare function preflightContractDeployMetaWithUi(client: BoingClient, tx: ContractDeployMetaTxObject): Promise<PreflightContractDeployMetaWithUiResult>;
export type BuildAndPreflightReferenceFungibleResult = PreflightContractDeployMetaWithUiResult & {
    tx: ContractDeployMetaTxObject;
};
/**
 * **Wizard shortcut:** build pinned fungible **`contract_deploy_meta`** and run QA preflight with UI copy.
 * Use the returned **`tx`** with **`boing_sendTransaction`** only when **`ui.readyToSign`** fits your product rules for **`unsure`**.
 */
export declare function buildAndPreflightReferenceFungibleDeploy(client: BoingClient, input: BuildReferenceFungibleDeployMetaTxInput): Promise<BuildAndPreflightReferenceFungibleResult>;
export type BuildAndPreflightReferenceNftCollectionResult = PreflightContractDeployMetaWithUiResult & {
    tx: ContractDeployMetaTxObject;
};
/**
 * Same as {@link buildAndPreflightReferenceFungibleDeploy} for the reference NFT collection template.
 */
export declare function buildAndPreflightReferenceNftCollectionDeployMeta(client: BoingClient, input: BuildReferenceNftCollectionDeployMetaTxInput): Promise<BuildAndPreflightReferenceNftCollectionResult>;
/** Which Boing deploy wizard / integration path is in use (drives bytecode + default `purpose_category`). */
export type BoingDeployIntegrationKind = 'token' | 'nft' | 'liquidity_pool';
/**
 * Single input shape for dApps: pick **`kind`** and the fields for that path; QA preflight uses the
 * resulting **`contract_deploy_meta`** automatically (placeholder `description_hash` when omitted).
 */
export type BoingIntegrationDeployInput = ({
    kind: 'token';
} & BuildReferenceFungibleDeployMetaTxInput) | ({
    kind: 'nft';
} & BuildReferenceNftCollectionDeployMetaTxInput) | ({
    kind: 'liquidity_pool';
} & BuildNativeConstantProductPoolDeployMetaTxInput);
/** Default mempool QA **`purpose_category`** per integration kind (pool → **`dapp`** per native AMM docs). */
export declare function defaultPurposeCategoryForBoingDeployKind(kind: BoingDeployIntegrationKind): string;
/**
 * Build **`contract_deploy_meta`** for token, NFT collection, or native CP pool — correct bytecode
 * resolution and **`purpose_category`** for each **`kind`**.
 */
export declare function buildBoingIntegrationDeployMetaTx(input: BoingIntegrationDeployInput): ContractDeployMetaTxObject;
export type PreflightBoingIntegrationDeployResult = PreflightContractDeployMetaWithUiResult & {
    tx: ContractDeployMetaTxObject;
};
/**
 * **One call:** build the meta tx for the chosen **`kind`**, run **`boing_qaCheck`**, return **`{ tx, qa, ui }`**
 * (same as {@link preflightContractDeployMetaWithUi} plus the signed-ready payload).
 */
export declare function preflightBoingIntegrationDeploy(client: BoingClient, input: BoingIntegrationDeployInput): Promise<PreflightBoingIntegrationDeployResult>;
/** Alias of {@link preflightBoingIntegrationDeploy} — same “build + QA UI” one-shot naming as token/NFT helpers. */
export declare const buildAndPreflightBoingIntegrationDeploy: typeof preflightBoingIntegrationDeploy;
export type BuildAndPreflightNativeConstantProductPoolResult = PreflightContractDeployMetaWithUiResult & {
    tx: ContractDeployMetaTxObject;
};
/**
 * **Wizard shortcut** for native CP pool: build meta tx (default **`purpose_category: dapp`**) + QA preflight.
 */
export declare function buildAndPreflightNativeConstantProductPoolDeploy(client: BoingClient, input: BuildNativeConstantProductPoolDeployMetaTxInput): Promise<BuildAndPreflightNativeConstantProductPoolResult>;
//# sourceMappingURL=dappDeploy.d.ts.map