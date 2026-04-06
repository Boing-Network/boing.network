/**
 * Versioning + tx-object helpers for **pinned** native Boing deploys (form-parity with EVM apps).
 *
 * See `docs/BOING-CANONICAL-DEPLOY-ARTIFACTS.md`. Full **fungible / NFT collection** bytecode
 * ships from `boing-execution` for **NFT collections** (`REFERENCE_NFT_COLLECTION_TEMPLATE_VERSION`);
 * the **fungible** template ships a pinned default (`DEFAULT_REFERENCE_FUNGIBLE_TEMPLATE_BYTECODE_HEX`).
 */

import { DEFAULT_REFERENCE_FUNGIBLE_TEMPLATE_BYTECODE_HEX } from './defaultReferenceFungibleTemplateBytecodeHex.js';

/** Logical id for the fungible template line item (docs + telemetry). */
export const REFERENCE_FUNGIBLE_TEMPLATE_ARTIFACT_ID = 'boing.reference_fungible.v0' as const;

/** Bump when default pinned hex in this package changes. */
export const REFERENCE_FUNGIBLE_TEMPLATE_VERSION = '1' as const;

/** Logical id for the NFT collection template. */
export const REFERENCE_NFT_COLLECTION_TEMPLATE_ARTIFACT_ID = 'boing.reference_nft_collection.v0' as const;

/** Matches `reference_nft_collection_template_bytecode()` in `boing-execution` (regenerate via `dump_reference_token_artifacts`). */
export const REFERENCE_NFT_COLLECTION_TEMPLATE_VERSION = '1' as const;

const DEFAULT_NFT_COLLECTION_ENV_KEYS = [
  'BOING_REFERENCE_NFT_COLLECTION_TEMPLATE_BYTECODE_HEX',
  'VITE_BOING_REFERENCE_NFT_COLLECTION_TEMPLATE_BYTECODE_HEX',
  'REACT_APP_BOING_REFERENCE_NFT_COLLECTION_TEMPLATE_BYTECODE_HEX',
] as const;

/** Boing Express / wallet JSON-RPC tx object for `contract_deploy_meta`. */
export type ContractDeployMetaTxObject = {
  type: 'contract_deploy_meta';
  bytecode: `0x${string}`;
  purpose_category: string;
  asset_name: string;
  asset_symbol: string;
  description_hash?: `0x${string}`;
};

const DEFAULT_FUNGIBLE_ENV_KEYS = [
  'BOING_REFERENCE_FUNGIBLE_TEMPLATE_BYTECODE_HEX',
  'VITE_BOING_REFERENCE_FUNGIBLE_TEMPLATE_BYTECODE_HEX',
  'REACT_APP_BOING_REFERENCE_FUNGIBLE_TEMPLATE_BYTECODE_HEX',
] as const;

type NodeishGlobal = { process?: { env?: Record<string, string | undefined> } };

function readProcessEnv(name: string): string | undefined {
  try {
    const proc = (globalThis as NodeishGlobal).process;
    if (proc?.env && typeof proc.env[name] === 'string') {
      const v = proc.env[name]?.trim();
      return v || undefined;
    }
  } catch {
    /* ignore */
  }
  return undefined;
}

/**
 * Normalize hex for wallet RPC payloads (`0x` prefix). Use for deploy bytecode or `description_hash`.
 */
export function ensure0xHex(hex: string): `0x${string}` {
  const t = hex.trim();
  if (!t) {
    throw new Error('ensure0xHex: empty hex');
  }
  const prefixed = t.startsWith('0x') || t.startsWith('0X') ? t : `0x${t}`;
  return prefixed as `0x${string}`;
}

/**
 * Resolve pinned fungible template bytecode: explicit override → known env keys → embedded default
 * (`DEFAULT_REFERENCE_FUNGIBLE_TEMPLATE_BYTECODE_HEX`).
 */
export function resolveReferenceFungibleTemplateBytecodeHex(opts?: {
  explicitHex?: string | undefined;
  /** Extra env var names to try after `explicitHex` (`globalThis.process.env` when present). */
  extraEnvKeys?: readonly string[];
}): `0x${string}` {
  if (opts?.explicitHex?.trim()) {
    return ensure0xHex(opts.explicitHex);
  }
  for (const k of DEFAULT_FUNGIBLE_ENV_KEYS) {
    const v = readProcessEnv(k);
    if (v) return ensure0xHex(v);
  }
  if (opts?.extraEnvKeys) {
    for (const k of opts.extraEnvKeys) {
      const v = readProcessEnv(k);
      if (v) return ensure0xHex(v);
    }
  }
  return ensure0xHex(DEFAULT_REFERENCE_FUNGIBLE_TEMPLATE_BYTECODE_HEX);
}

/**
 * Resolve pinned **reference NFT collection** template bytecode (same pattern as fungible).
 */
export function resolveReferenceNftCollectionTemplateBytecodeHex(opts?: {
  explicitHex?: string | undefined;
  extraEnvKeys?: readonly string[];
}): `0x${string}` | undefined {
  if (opts?.explicitHex?.trim()) {
    return ensure0xHex(opts.explicitHex);
  }
  for (const k of DEFAULT_NFT_COLLECTION_ENV_KEYS) {
    const v = readProcessEnv(k);
    if (v) return ensure0xHex(v);
  }
  if (opts?.extraEnvKeys) {
    for (const k of opts.extraEnvKeys) {
      const v = readProcessEnv(k);
      if (v) return ensure0xHex(v);
    }
  }
  return undefined;
}

/**
 * Build a **`contract_deploy_meta`** object for `boing_sendTransaction` / `boing_signTransaction`.
 */
export function buildContractDeployMetaTx(input: {
  bytecodeHex: string;
  assetName: string;
  assetSymbol: string;
  /** Default `token` matches Express convenience when name/symbol are set. */
  purposeCategory?: string;
  descriptionHashHex?: string;
}): ContractDeployMetaTxObject {
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
  const out: ContractDeployMetaTxObject = {
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
export function buildReferenceFungibleDeployMetaTx(
  input: BuildReferenceFungibleDeployMetaTxInput,
): ContractDeployMetaTxObject {
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
export function buildReferenceNftCollectionDeployMetaTx(
  input: BuildReferenceNftCollectionDeployMetaTxInput,
): ContractDeployMetaTxObject {
  const bytecodeHex = input.bytecodeHexOverride?.trim()
    ? ensure0xHex(input.bytecodeHexOverride)
    : resolveReferenceNftCollectionTemplateBytecodeHex({ extraEnvKeys: input.extraEnvKeys });
  if (!bytecodeHex) {
    throw new Error(
      'buildReferenceNftCollectionDeployMetaTx: no collection bytecode — set BOING_REFERENCE_NFT_COLLECTION_TEMPLATE_BYTECODE_HEX (or VITE_/REACT_APP_ variant), or pass bytecodeHexOverride',
    );
  }
  return buildContractDeployMetaTx({
    bytecodeHex,
    assetName: input.collectionName,
    assetSymbol: input.collectionSymbol,
    purposeCategory: input.purposeCategory ?? 'nft',
    descriptionHashHex: input.descriptionHashHex,
  });
}
