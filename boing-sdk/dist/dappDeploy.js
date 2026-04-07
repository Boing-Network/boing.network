/**
 * dApp deploy helpers: QA preflight for **`contract_deploy_meta`** payloads (wizard “review” steps).
 *
 * See [BOING-CANONICAL-DEPLOY-ARTIFACTS.md](../../docs/BOING-CANONICAL-DEPLOY-ARTIFACTS.md),
 * [BOING-DAPP-INTEGRATION.md](../../docs/BOING-DAPP-INTEGRATION.md).
 */
import { buildNativeConstantProductPoolDeployMetaTx, buildReferenceFungibleDeployMetaTx, buildReferenceFungibleSecuredDeployMetaTx, buildReferenceNftCollectionDeployMetaTx, } from './canonicalDeployArtifacts.js';
/**
 * When **`boing_qaCheck`** needs **`asset_name` / `asset_symbol`** but the wizard has no description
 * commitment yet, the RPC allows a **placeholder** 32-byte hash ([RPC-API-SPEC.md](../../docs/RPC-API-SPEC.md) § **boing_qaCheck**).
 */
export const BOING_QA_PLACEHOLDER_DESCRIPTION_HASH_HEX = `0x${'00'.repeat(32)}`;
/**
 * Run **`boing_qaCheck`** using the same fields as a **`contract_deploy_meta`** tx object.
 * Use on the **review** step of a deploy wizard so users see **allow / reject / unsure** before signing.
 */
export async function preflightContractDeployMetaQa(client, tx) {
    const dh = tx.description_hash?.trim();
    if (dh) {
        return client.qaCheck(tx.bytecode, tx.purpose_category, dh, tx.asset_name, tx.asset_symbol);
    }
    return client.qaCheck(tx.bytecode, tx.purpose_category, BOING_QA_PLACEHOLDER_DESCRIPTION_HASH_HEX, tx.asset_name, tx.asset_symbol);
}
/**
 * Map a **`boing_qaCheck`** response to user-facing copy (preflight or post-error messaging).
 * Use with **`preflightContractDeployMetaQa`** on the review step, or after submit failures parsed as QA.
 */
export function describeContractDeployMetaQaResponse(qa) {
    const outcome = qa.result;
    switch (outcome) {
        case 'allow':
            return {
                result: 'allow',
                readyToSign: true,
                headline: 'Ready to deploy',
                detail: 'This deployment passed automated Boing quality checks. The user can sign when they are satisfied.',
                tone: 'positive',
            };
        case 'reject': {
            const rule = qa.rule_id?.trim();
            const msg = qa.message?.trim();
            const technical = rule && msg ? `${rule}: ${msg}` : msg || rule || 'This deployment does not meet network rules.';
            return {
                result: 'reject',
                readyToSign: false,
                headline: 'Deployment blocked by quality checks',
                detail: `${technical} Adjust the contract, name, symbol, or purpose and try again, or see the network QA checklist for allowed deployments.`,
                tone: 'destructive',
            };
        }
        case 'unsure':
            return {
                result: 'unsure',
                readyToSign: true,
                headline: 'Additional review may apply',
                detail: 'Automated checks could not finalize a decision. After signing, the transaction may enter the community QA pool instead of landing immediately. Explain this to the user before they confirm.',
                tone: 'warning',
            };
        default: {
            const _exhaustive = outcome;
            return _exhaustive;
        }
    }
}
/**
 * Run **`preflightContractDeployMetaQa`** and attach {@link describeContractDeployMetaQaResponse} for one-shot UI wiring.
 */
export async function preflightContractDeployMetaWithUi(client, tx) {
    const qa = await preflightContractDeployMetaQa(client, tx);
    return { qa, ui: describeContractDeployMetaQaResponse(qa) };
}
/**
 * **Wizard shortcut:** build pinned fungible **`contract_deploy_meta`** and run QA preflight with UI copy.
 * Use the returned **`tx`** with **`boing_sendTransaction`** only when **`ui.readyToSign`** fits your product rules for **`unsure`**.
 */
export async function buildAndPreflightReferenceFungibleDeploy(client, input) {
    const tx = buildReferenceFungibleDeployMetaTx(input);
    const { qa, ui } = await preflightContractDeployMetaWithUi(client, tx);
    return { tx, qa, ui };
}
/**
 * Same as {@link buildAndPreflightReferenceFungibleDeploy} for the reference NFT collection template.
 */
export async function buildAndPreflightReferenceNftCollectionDeployMeta(client, input) {
    const tx = buildReferenceNftCollectionDeployMetaTx(input);
    const { qa, ui } = await preflightContractDeployMetaWithUi(client, tx);
    return { tx, qa, ui };
}
/** Default mempool QA **`purpose_category`** per integration kind (pool → **`dapp`** per native AMM docs). */
export function defaultPurposeCategoryForBoingDeployKind(kind) {
    switch (kind) {
        case 'token':
            return 'token';
        case 'nft':
            return 'nft';
        case 'liquidity_pool':
            return 'dapp';
        default: {
            const _exhaustive = kind;
            return _exhaustive;
        }
    }
}
/**
 * Build **`contract_deploy_meta`** for token, NFT collection, or native CP pool — correct bytecode
 * resolution and **`purpose_category`** for each **`kind`**.
 */
export function buildBoingIntegrationDeployMetaTx(input) {
    switch (input.kind) {
        case 'token': {
            const { kind: _k, ...tokenFields } = input;
            return buildReferenceFungibleSecuredDeployMetaTx(tokenFields);
        }
        case 'nft':
            return buildReferenceNftCollectionDeployMetaTx(input);
        case 'liquidity_pool':
            return buildNativeConstantProductPoolDeployMetaTx(input);
        default: {
            const _exhaustive = input;
            return _exhaustive;
        }
    }
}
/**
 * **One call:** build the meta tx for the chosen **`kind`**, run **`boing_qaCheck`**, return **`{ tx, qa, ui }`**
 * (same as {@link preflightContractDeployMetaWithUi} plus the signed-ready payload).
 */
export async function preflightBoingIntegrationDeploy(client, input) {
    const tx = buildBoingIntegrationDeployMetaTx(input);
    const { qa, ui } = await preflightContractDeployMetaWithUi(client, tx);
    return { tx, qa, ui };
}
/** Alias of {@link preflightBoingIntegrationDeploy} — same “build + QA UI” one-shot naming as token/NFT helpers. */
export const buildAndPreflightBoingIntegrationDeploy = preflightBoingIntegrationDeploy;
/**
 * **Wizard shortcut** for native CP pool: build meta tx (default **`purpose_category: dapp`**) + QA preflight.
 */
export async function buildAndPreflightNativeConstantProductPoolDeploy(client, input) {
    return preflightBoingIntegrationDeploy(client, { kind: 'liquidity_pool', ...input });
}
