/**
 * Versioning + tx-object helpers for **pinned** native Boing deploys (form-parity with EVM apps).
 *
 * See `docs/BOING-CANONICAL-DEPLOY-ARTIFACTS.md`. Full **fungible / NFT collection** bytecode
 * ships from `boing-execution` for **NFT collections** (`REFERENCE_NFT_COLLECTION_TEMPLATE_VERSION`);
 * the **fungible** template ships a pinned default (`DEFAULT_REFERENCE_FUNGIBLE_TEMPLATE_BYTECODE_HEX`).
 */
import { DEFAULT_REFERENCE_FUNGIBLE_TEMPLATE_BYTECODE_HEX } from './defaultReferenceFungibleTemplateBytecodeHex.js';
/** Logical id for the fungible template line item (docs + telemetry). */
export const REFERENCE_FUNGIBLE_TEMPLATE_ARTIFACT_ID = 'boing.reference_fungible.v0';
/** Bump when default pinned hex in this package changes. */
export const REFERENCE_FUNGIBLE_TEMPLATE_VERSION = '1';
/** Logical id for the NFT collection template. */
export const REFERENCE_NFT_COLLECTION_TEMPLATE_ARTIFACT_ID = 'boing.reference_nft_collection.v0';
/** Matches `reference_nft_collection_template_bytecode()` in `boing-execution` (regenerate via `dump_reference_token_artifacts`). */
export const REFERENCE_NFT_COLLECTION_TEMPLATE_VERSION = '1';
const DEFAULT_NFT_COLLECTION_ENV_KEYS = [
    'BOING_REFERENCE_NFT_COLLECTION_TEMPLATE_BYTECODE_HEX',
    'VITE_BOING_REFERENCE_NFT_COLLECTION_TEMPLATE_BYTECODE_HEX',
    'REACT_APP_BOING_REFERENCE_NFT_COLLECTION_TEMPLATE_BYTECODE_HEX',
];
const DEFAULT_FUNGIBLE_ENV_KEYS = [
    'BOING_REFERENCE_FUNGIBLE_TEMPLATE_BYTECODE_HEX',
    'VITE_BOING_REFERENCE_FUNGIBLE_TEMPLATE_BYTECODE_HEX',
    'REACT_APP_BOING_REFERENCE_FUNGIBLE_TEMPLATE_BYTECODE_HEX',
];
function readProcessEnv(name) {
    try {
        const proc = globalThis.process;
        if (proc?.env && typeof proc.env[name] === 'string') {
            const v = proc.env[name]?.trim();
            return v || undefined;
        }
    }
    catch {
        /* ignore */
    }
    return undefined;
}
/**
 * Normalize hex for wallet RPC payloads (`0x` prefix). Use for deploy bytecode or `description_hash`.
 */
export function ensure0xHex(hex) {
    const t = hex.trim();
    if (!t) {
        throw new Error('ensure0xHex: empty hex');
    }
    const prefixed = t.startsWith('0x') || t.startsWith('0X') ? t : `0x${t}`;
    return prefixed;
}
/**
 * Resolve pinned fungible template bytecode: explicit override → known env keys → embedded default
 * (`DEFAULT_REFERENCE_FUNGIBLE_TEMPLATE_BYTECODE_HEX`).
 */
export function resolveReferenceFungibleTemplateBytecodeHex(opts) {
    if (opts?.explicitHex?.trim()) {
        return ensure0xHex(opts.explicitHex);
    }
    for (const k of DEFAULT_FUNGIBLE_ENV_KEYS) {
        const v = readProcessEnv(k);
        if (v)
            return ensure0xHex(v);
    }
    if (opts?.extraEnvKeys) {
        for (const k of opts.extraEnvKeys) {
            const v = readProcessEnv(k);
            if (v)
                return ensure0xHex(v);
        }
    }
    return ensure0xHex(DEFAULT_REFERENCE_FUNGIBLE_TEMPLATE_BYTECODE_HEX);
}
/**
 * Resolve pinned **reference NFT collection** template bytecode (same pattern as fungible).
 */
export function resolveReferenceNftCollectionTemplateBytecodeHex(opts) {
    if (opts?.explicitHex?.trim()) {
        return ensure0xHex(opts.explicitHex);
    }
    for (const k of DEFAULT_NFT_COLLECTION_ENV_KEYS) {
        const v = readProcessEnv(k);
        if (v)
            return ensure0xHex(v);
    }
    if (opts?.extraEnvKeys) {
        for (const k of opts.extraEnvKeys) {
            const v = readProcessEnv(k);
            if (v)
                return ensure0xHex(v);
        }
    }
    return undefined;
}
/**
 * Build a **`contract_deploy_meta`** object for `boing_sendTransaction` / `boing_signTransaction`.
 */
export function buildContractDeployMetaTx(input) {
    const name = input.assetName.trim();
    const sym = input.assetSymbol.trim().toUpperCase();
    if (!name) {
        throw new Error('buildContractDeployMetaTx: assetName required');
    }
    if (!sym) {
        throw new Error('buildContractDeployMetaTx: assetSymbol required');
    }
    const bytecode = ensure0xHex(input.bytecodeHex);
    const purpose_category = (input.purposeCategory ?? 'token').trim();
    const out = {
        type: 'contract_deploy_meta',
        bytecode,
        purpose_category,
        asset_name: name,
        asset_symbol: sym,
    };
    const dh = input.descriptionHashHex?.trim();
    if (dh) {
        out.description_hash = ensure0xHex(dh);
    }
    return out;
}
/**
 * **One call** for wizard-style **Deploy token** on Boing: resolve pinned fungible bytecode + build **`contract_deploy_meta`**.
 * Pass the result to **`boing_sendTransaction`** / **`boing_signTransaction`** (Boing Express).
 */
export function buildReferenceFungibleDeployMetaTx(input) {
    const bytecodeHex = input.bytecodeHexOverride?.trim()
        ? ensure0xHex(input.bytecodeHexOverride)
        : resolveReferenceFungibleTemplateBytecodeHex({ extraEnvKeys: input.extraEnvKeys });
    return buildContractDeployMetaTx({
        bytecodeHex,
        assetName: input.assetName,
        assetSymbol: input.assetSymbol,
        purposeCategory: input.purposeCategory,
        descriptionHashHex: input.descriptionHashHex,
    });
}
/**
 * **One call** for **native NFT collection** deploy meta tx. Requires pinned collection bytecode
 * (env or **`bytecodeHexOverride`**); throws a clear error if unresolved — same constraint as manual **`resolve` + `build`**.
 */
export function buildReferenceNftCollectionDeployMetaTx(input) {
    const bytecodeHex = input.bytecodeHexOverride?.trim()
        ? ensure0xHex(input.bytecodeHexOverride)
        : resolveReferenceNftCollectionTemplateBytecodeHex({ extraEnvKeys: input.extraEnvKeys });
    if (!bytecodeHex) {
        throw new Error('buildReferenceNftCollectionDeployMetaTx: no collection bytecode — set BOING_REFERENCE_NFT_COLLECTION_TEMPLATE_BYTECODE_HEX (or VITE_/REACT_APP_ variant), or pass bytecodeHexOverride');
    }
    return buildContractDeployMetaTx({
        bytecodeHex,
        assetName: input.collectionName,
        assetSymbol: input.collectionSymbol,
        purposeCategory: input.purposeCategory ?? 'nft',
        descriptionHashHex: input.descriptionHashHex,
    });
}
const DEFAULT_NATIVE_AMM_POOL_ENV_KEYS = [
    'BOING_NATIVE_AMM_BYTECODE_HEX',
    'VITE_BOING_NATIVE_AMM_BYTECODE_HEX',
    'REACT_APP_BOING_NATIVE_AMM_BYTECODE_HEX',
];
/**
 * Resolve **native constant-product pool** bytecode from override or env (same keys as
 * [examples/native-boing-tutorial](../examples/native-boing-tutorial/) **`BOING_NATIVE_AMM_BYTECODE_HEX`**).
 */
export function resolveNativeConstantProductPoolBytecodeHex(opts) {
    if (opts?.explicitHex?.trim()) {
        return ensure0xHex(opts.explicitHex);
    }
    for (const k of DEFAULT_NATIVE_AMM_POOL_ENV_KEYS) {
        const v = readProcessEnv(k);
        if (v)
            return ensure0xHex(v);
    }
    if (opts?.extraEnvKeys) {
        for (const k of opts.extraEnvKeys) {
            const v = readProcessEnv(k);
            if (v)
                return ensure0xHex(v);
        }
    }
    return undefined;
}
/**
 * **One call** for **native CP pool** **`contract_deploy_meta`**: pinned bytecode from env or override,
 * **`purpose_category`** default **`dapp`**, then the same Express shape as token/NFT deploys.
 */
export function buildNativeConstantProductPoolDeployMetaTx(input) {
    const bytecodeHex = input.bytecodeHexOverride?.trim()
        ? ensure0xHex(input.bytecodeHexOverride)
        : resolveNativeConstantProductPoolBytecodeHex({ extraEnvKeys: input.extraEnvKeys });
    if (!bytecodeHex) {
        throw new Error('buildNativeConstantProductPoolDeployMetaTx: no pool bytecode — set BOING_NATIVE_AMM_BYTECODE_HEX (or VITE_/REACT_APP_ variant), or pass bytecodeHexOverride');
    }
    const name = (input.poolLabel ?? 'Native CP Pool').trim();
    const sym = (input.poolSymbol ?? 'POOL').trim().toUpperCase();
    if (!name) {
        throw new Error('buildNativeConstantProductPoolDeployMetaTx: poolLabel required when set to empty');
    }
    if (!sym) {
        throw new Error('buildNativeConstantProductPoolDeployMetaTx: poolSymbol required when set to empty');
    }
    return buildContractDeployMetaTx({
        bytecodeHex,
        assetName: name,
        assetSymbol: sym,
        purposeCategory: input.purposeCategory ?? 'dapp',
        descriptionHashHex: input.descriptionHashHex,
    });
}
