/**
 * Versioning + tx-object helpers for **pinned** native Boing deploys (form-parity with EVM apps).
 *
 * See `docs/BOING-CANONICAL-DEPLOY-ARTIFACTS.md`. **Fungible** and **NFT collection** templates ship a
 * pinned default in this package (`DEFAULT_REFERENCE_*`); env vars still override when set.
 */

import { DEFAULT_REFERENCE_FUNGIBLE_TEMPLATE_BYTECODE_HEX } from './defaultReferenceFungibleTemplateBytecodeHex.js';
import { DEFAULT_REFERENCE_FUNGIBLE_SECURED_TEMPLATE_BYTECODE_HEX } from './defaultReferenceFungibleSecuredTemplateBytecodeHex.js';
import { DEFAULT_REFERENCE_NFT_COLLECTION_TEMPLATE_BYTECODE_HEX } from './defaultReferenceNftCollectionTemplateBytecodeHex.js';
import {
  descriptionHashHexFromNativeTokenSecurity,
  type NativeTokenSecurityFeaturesInput,
} from './nativeTokenSecurity.js';
import { buildReferenceFungibleSecuredDeployBytecodeHexFromNativeTokenSecurity } from './referenceFungibleSecuredDeployBytecode.js';

/** Logical id for the fungible template line item (docs + telemetry). */
export const REFERENCE_FUNGIBLE_TEMPLATE_ARTIFACT_ID = 'boing.reference_fungible.v0' as const;

/** Bump when default pinned hex in this package changes. */
export const REFERENCE_FUNGIBLE_TEMPLATE_VERSION = '1' as const;

/** Logical id for the secured fungible template (`0xFD` init + runtime toggles). */
export const REFERENCE_FUNGIBLE_SECURED_TEMPLATE_ARTIFACT_ID = 'boing.reference_fungible_secured.v0' as const;

/** Bump when default pinned secured hex in this package changes. */
export const REFERENCE_FUNGIBLE_SECURED_TEMPLATE_VERSION = '1' as const;

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

const DEFAULT_FUNGIBLE_SECURED_ENV_KEYS = [
  'BOING_REFERENCE_FUNGIBLE_SECURED_TEMPLATE_BYTECODE_HEX',
  'VITE_BOING_REFERENCE_FUNGIBLE_SECURED_TEMPLATE_BYTECODE_HEX',
  'REACT_APP_BOING_REFERENCE_FUNGIBLE_SECURED_TEMPLATE_BYTECODE_HEX',
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
 * Resolve pinned **secured** fungible deploy bytecode (`0xFD` init + runtime): explicit → env → embedded default.
 */
export function resolveReferenceFungibleSecuredTemplateBytecodeHex(opts?: {
  explicitHex?: string | undefined;
  extraEnvKeys?: readonly string[];
}): `0x${string}` {
  if (opts?.explicitHex?.trim()) {
    return ensure0xHex(opts.explicitHex);
  }
  for (const k of DEFAULT_FUNGIBLE_SECURED_ENV_KEYS) {
    const v = readProcessEnv(k);
    if (v) return ensure0xHex(v);
  }
  if (opts?.extraEnvKeys) {
    for (const k of opts.extraEnvKeys) {
      const v = readProcessEnv(k);
      if (v) return ensure0xHex(v);
    }
  }
  return ensure0xHex(DEFAULT_REFERENCE_FUNGIBLE_SECURED_TEMPLATE_BYTECODE_HEX);
}

/**
 * Resolve pinned **reference NFT collection** template bytecode (explicit → env → embedded default,
 * same pattern as {@link resolveReferenceFungibleTemplateBytecodeHex}).
 */
export function resolveReferenceNftCollectionTemplateBytecodeHex(opts?: {
  explicitHex?: string | undefined;
  extraEnvKeys?: readonly string[];
}): `0x${string}` {
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
  return ensure0xHex(DEFAULT_REFERENCE_NFT_COLLECTION_TEMPLATE_BYTECODE_HEX);
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
export function buildReferenceFungibleDeployMetaTx(
  input: BuildReferenceFungibleDeployMetaTxInput,
): ContractDeployMetaTxObject {
  const bytecodeHex = input.bytecodeHexOverride?.trim()
    ? ensure0xHex(input.bytecodeHexOverride)
    : resolveReferenceFungibleTemplateBytecodeHex({ extraEnvKeys: input.extraEnvKeys });
  const explicitDh = input.descriptionHashHex?.trim();
  const securityDh =
    !explicitDh && input.nativeTokenSecurity
      ? descriptionHashHexFromNativeTokenSecurity(input.nativeTokenSecurity)
      : undefined;
  return buildContractDeployMetaTx({
    bytecodeHex,
    assetName: input.assetName,
    assetSymbol: input.assetSymbol,
    purposeCategory: input.purposeCategory,
    descriptionHashHex: explicitDh || securityDh,
  });
}

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
  chainContext?: { chainHeight: bigint };
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
export function buildReferenceFungibleSecuredDeployMetaTx(
  input: BuildReferenceFungibleSecuredDeployMetaTxInput,
): ContractDeployMetaTxObject {
  const bytecodeHex = input.bytecodeHexOverride?.trim()
    ? ensure0xHex(input.bytecodeHexOverride)
    : input.nativeTokenSecurity !== undefined
      ? buildReferenceFungibleSecuredDeployBytecodeHexFromNativeTokenSecurity(
          input.nativeTokenSecurity,
          {
            chainHeight: input.chainContext?.chainHeight,
            mintFirstTotalSupplyWei: input.mintFirstTotalSupplyWei,
          },
        )
      : resolveReferenceFungibleSecuredTemplateBytecodeHex({ extraEnvKeys: input.extraEnvKeys });
  const explicitDh = input.descriptionHashHex?.trim();
  const securityDh =
    !explicitDh && input.nativeTokenSecurity
      ? descriptionHashHexFromNativeTokenSecurity(input.nativeTokenSecurity)
      : undefined;
  return buildContractDeployMetaTx({
    bytecodeHex,
    assetName: input.assetName,
    assetSymbol: input.assetSymbol,
    purposeCategory: input.purposeCategory,
    descriptionHashHex: explicitDh || securityDh,
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
 * **One call** for **native NFT collection** deploy meta tx. Bytecode: **`bytecodeHexOverride`** if set,
 * else env keys (see {@link resolveReferenceNftCollectionTemplateBytecodeHex}), else the SDK embedded default.
 */
export function buildReferenceNftCollectionDeployMetaTx(
  input: BuildReferenceNftCollectionDeployMetaTxInput,
): ContractDeployMetaTxObject {
  const bytecodeHex = input.bytecodeHexOverride?.trim()
    ? ensure0xHex(input.bytecodeHexOverride)
    : resolveReferenceNftCollectionTemplateBytecodeHex({ extraEnvKeys: input.extraEnvKeys });
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
] as const;

/**
 * Resolve **native constant-product pool** bytecode from override or env (same keys as
 * [examples/native-boing-tutorial](../examples/native-boing-tutorial/) **`BOING_NATIVE_AMM_BYTECODE_HEX`**).
 */
export function resolveNativeConstantProductPoolBytecodeHex(opts?: {
  explicitHex?: string | undefined;
  extraEnvKeys?: readonly string[];
}): `0x${string}` | undefined {
  if (opts?.explicitHex?.trim()) {
    return ensure0xHex(opts.explicitHex);
  }
  for (const k of DEFAULT_NATIVE_AMM_POOL_ENV_KEYS) {
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
export function buildNativeConstantProductPoolDeployMetaTx(
  input: BuildNativeConstantProductPoolDeployMetaTxInput,
): ContractDeployMetaTxObject {
  const bytecodeHex = input.bytecodeHexOverride?.trim()
    ? ensure0xHex(input.bytecodeHexOverride)
    : resolveNativeConstantProductPoolBytecodeHex({ extraEnvKeys: input.extraEnvKeys });
  if (!bytecodeHex) {
    throw new Error(
      'buildNativeConstantProductPoolDeployMetaTx: no pool bytecode — set BOING_NATIVE_AMM_BYTECODE_HEX (or VITE_/REACT_APP_ variant), or pass bytecodeHexOverride',
    );
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
