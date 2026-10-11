/**
 * On-chain **builder attestation** registry (enforced builder signal).
 * Matches `boing_execution::builder_attestation_registry`.
 * See `docs/BOING-BUILDER-ATTESTATION.md`.
 *
 * CREATE2 salt `BOING_BUILDER_ATTEST_REG_V1`, selectors `0xE0`–`0xE6`.
 * Canonical testnet AccountId: set after first public deploy (env-pin apps).
 */

import { mergeAccessListWithSimulation } from './accessList.js';
import {
  buildContractDeployMetaTx,
  ensure0xHex,
  type ContractDeployMetaTxObject,
} from './canonicalDeployArtifacts.js';
import { DEFAULT_BUILDER_ATTESTATION_REGISTRY_BYTECODE_HEX } from './defaultBuilderAttestationRegistryBytecodeHex.js';
import { bytesToHex, ensureHex, hexToBytes, validateHex32 } from './hex.js';
import { decodeBoingStorageWordU128 } from './nativeAmmPool.js';
import type { SimulateResult } from './types.js';

/** `claim_asset(asset)` — **64** bytes. */
export const SELECTOR_BUILDER_ATTEST_CLAIM_ASSET = 0xe0;
/** `attest(collection, token, note_hash)` — **128** bytes. */
export const SELECTOR_BUILDER_ATTEST_ATTEST = 0xe1;
/** `revoke_at(index)` — **64** bytes. */
export const SELECTOR_BUILDER_ATTEST_REVOKE_AT = 0xe2;
/** `attestations_count` — **32** bytes. */
export const SELECTOR_BUILDER_ATTEST_COUNT = 0xe3;
/** `get_attestation_at(index)` — **64** bytes; returns **128** bytes. */
export const SELECTOR_BUILDER_ATTEST_GET_AT = 0xe4;
/** `get_asset_claimer(asset)` — **64** bytes. */
export const SELECTOR_BUILDER_ATTEST_GET_CLAIMER = 0xe5;
/** `transfer_asset_claimer(asset, new_claimer)` — **96** bytes. */
export const SELECTOR_BUILDER_ATTEST_TRANSFER_CLAIMER = 0xe6;

export const BUILDER_ATTESTATION_REGISTRY_MAX_SLOTS = 4096;

export const BUILDER_ATTEST_TOPIC_ATTEST_HEX = (() => {
  const u8 = new Uint8Array(32);
  u8.set(new TextEncoder().encode('BOING_BLDR_ATTEST_REG1'));
  return bytesToHex(u8);
})();

export const BUILDER_ATTEST_TOPIC_REVOKE_HEX = (() => {
  const u8 = new Uint8Array(32);
  u8.set(new TextEncoder().encode('BOING_BLDR_ATTEST_UNL1'));
  return bytesToHex(u8);
})();

export const BUILDER_ATTESTATION_REGISTRY_CREATE2_SALT_V1_HEX = (() => {
  const u8 = new Uint8Array(32);
  u8.set(new TextEncoder().encode('BOING_BUILDER_ATTEST_REG_V1'));
  return bytesToHex(u8);
})();

/** Count storage key (`builder_attestation_registry_count_key`). */
export const BUILDER_ATTESTATION_REGISTRY_COUNT_KEY_HEX =
  `0x${'00'.repeat(16)}424f494e4741545400000000ffffffff` as const;

const HEX_RE = /^[0-9a-fA-F]+$/;
const ZERO32 = validateHex32(`0x${'00'.repeat(32)}`);

function selectorWord(selector: number): Uint8Array {
  const w = new Uint8Array(32);
  w[31] = selector & 0xff;
  return w;
}

function u64Word(n: number): Uint8Array {
  if (!Number.isInteger(n) || n < 0 || n > Number.MAX_SAFE_INTEGER) {
    throw new RangeError('index must be a non-negative safe integer');
  }
  const w = new Uint8Array(32);
  new DataView(w.buffer).setBigUint64(24, BigInt(n), false);
  return w;
}

function concatWords(parts: Uint8Array[]): Uint8Array {
  const n = parts.reduce((a, p) => a + p.length, 0);
  const out = new Uint8Array(n);
  let o = 0;
  for (const p of parts) {
    out.set(p, o);
    o += p.length;
  }
  return out;
}

export function encodeBuilderAttestClaimAssetCalldata(assetHex32: string): Uint8Array {
  return concatWords([
    selectorWord(SELECTOR_BUILDER_ATTEST_CLAIM_ASSET),
    hexToBytes(validateHex32(assetHex32)),
  ]);
}

export function encodeBuilderAttestClaimAssetCalldataHex(assetHex32: string): string {
  return bytesToHex(encodeBuilderAttestClaimAssetCalldata(assetHex32));
}

export function encodeBuilderAttestAttestCalldata(
  collectionHex32: string,
  tokenHex32: string,
  noteHashHex32: string = ZERO32,
): Uint8Array {
  return concatWords([
    selectorWord(SELECTOR_BUILDER_ATTEST_ATTEST),
    hexToBytes(validateHex32(collectionHex32)),
    hexToBytes(validateHex32(tokenHex32)),
    hexToBytes(validateHex32(noteHashHex32)),
  ]);
}

export function encodeBuilderAttestAttestCalldataHex(
  collectionHex32: string,
  tokenHex32: string,
  noteHashHex32: string = ZERO32,
): string {
  return bytesToHex(
    encodeBuilderAttestAttestCalldata(collectionHex32, tokenHex32, noteHashHex32),
  );
}

export function encodeBuilderAttestRevokeAtCalldata(index: number): Uint8Array {
  return concatWords([selectorWord(SELECTOR_BUILDER_ATTEST_REVOKE_AT), u64Word(index)]);
}

export function encodeBuilderAttestRevokeAtCalldataHex(index: number): string {
  return bytesToHex(encodeBuilderAttestRevokeAtCalldata(index));
}

export function encodeBuilderAttestCountCalldata(): Uint8Array {
  return selectorWord(SELECTOR_BUILDER_ATTEST_COUNT);
}

export function encodeBuilderAttestCountCalldataHex(): string {
  return bytesToHex(encodeBuilderAttestCountCalldata());
}

export function encodeBuilderAttestGetAtCalldata(index: number): Uint8Array {
  return concatWords([selectorWord(SELECTOR_BUILDER_ATTEST_GET_AT), u64Word(index)]);
}

export function encodeBuilderAttestGetAtCalldataHex(index: number): string {
  return bytesToHex(encodeBuilderAttestGetAtCalldata(index));
}

export function encodeBuilderAttestGetClaimerCalldata(assetHex32: string): Uint8Array {
  return concatWords([
    selectorWord(SELECTOR_BUILDER_ATTEST_GET_CLAIMER),
    hexToBytes(validateHex32(assetHex32)),
  ]);
}

export function encodeBuilderAttestGetClaimerCalldataHex(assetHex32: string): string {
  return bytesToHex(encodeBuilderAttestGetClaimerCalldata(assetHex32));
}

export function encodeBuilderAttestTransferClaimerCalldata(
  assetHex32: string,
  newClaimerHex32: string,
): Uint8Array {
  return concatWords([
    selectorWord(SELECTOR_BUILDER_ATTEST_TRANSFER_CLAIMER),
    hexToBytes(validateHex32(assetHex32)),
    hexToBytes(validateHex32(newClaimerHex32)),
  ]);
}

export function encodeBuilderAttestTransferClaimerCalldataHex(
  assetHex32: string,
  newClaimerHex32: string,
): string {
  return bytesToHex(
    encodeBuilderAttestTransferClaimerCalldata(assetHex32, newClaimerHex32),
  );
}

export function decodeBuilderAttestCountReturnData(returnDataHex: string): bigint {
  const w = decodeBoingStorageWordU128(ensureHex(returnDataHex));
  return w & 0xffff_ffff_ffff_ffffn;
}

export function decodeBuilderAttestGetAtReturnData(returnDataHex: string): {
  collectionHex: string;
  tokenHex: string;
  builderHex: string;
  noteHashHex: string;
  /** False when collection is zero (tombstone / empty slot). */
  active: boolean;
} {
  const raw = ensureHex(returnDataHex).slice(2).toLowerCase();
  if (!HEX_RE.test(raw)) throw new Error('get_attestation_at return data: invalid hex');
  if (raw.length < 256) {
    throw new Error('get_attestation_at return data: expected 128 bytes (256 hex chars)');
  }
  const collectionHex = validateHex32(`0x${raw.slice(0, 64)}`);
  const tokenHex = validateHex32(`0x${raw.slice(64, 128)}`);
  const builderHex = validateHex32(`0x${raw.slice(128, 192)}`);
  const noteHashHex = validateHex32(`0x${raw.slice(192, 256)}`);
  return {
    collectionHex,
    tokenHex,
    builderHex,
    noteHashHex,
    active: collectionHex !== ZERO32,
  };
}

function normalizeCalldataHex(calldataHex: string): string {
  const h = ensureHex(calldataHex.trim());
  const raw = h.slice(2);
  if (raw.length % 2 !== 0) throw new Error('calldata must be even-length hex');
  if (!HEX_RE.test(raw)) throw new Error('calldata: invalid hex');
  return `0x${raw.toLowerCase()}`;
}

export function resolveBuilderAttestationRegistryBytecodeHex(opts?: {
  explicitHex?: string;
}): `0x${string}` {
  if (opts?.explicitHex?.trim()) return ensure0xHex(opts.explicitHex);
  return ensure0xHex(DEFAULT_BUILDER_ATTESTATION_REGISTRY_BYTECODE_HEX);
}

export function buildBuilderAttestationRegistryDeployMetaTx(input?: {
  assetName?: string;
  assetSymbol?: string;
  purposeCategory?: string;
  descriptionHashHex?: string;
  create2SaltHex?: string;
  bytecodeHexOverride?: string;
}): ContractDeployMetaTxObject {
  const bytecodeHex = input?.bytecodeHexOverride?.trim()
    ? ensure0xHex(input.bytecodeHexOverride)
    : resolveBuilderAttestationRegistryBytecodeHex();
  return buildContractDeployMetaTx({
    bytecodeHex,
    assetName: input?.assetName?.trim() || 'Builder Attestation Registry',
    assetSymbol: input?.assetSymbol?.trim() || 'BATT',
    purposeCategory: input?.purposeCategory ?? 'dapp',
    descriptionHashHex: input?.descriptionHashHex,
    create2SaltHex: input?.create2SaltHex ?? BUILDER_ATTESTATION_REGISTRY_CREATE2_SALT_V1_HEX,
  });
}

export function buildBuilderAttestationRegistryAccessList(
  senderHex32: string,
  registryHex32: string,
): { read: string[]; write: string[] } {
  const s = validateHex32(senderHex32).toLowerCase();
  const r = validateHex32(registryHex32).toLowerCase();
  return { read: [s, r], write: [s, r] };
}

export function buildBuilderAttestationRegistryContractCallTx(
  senderHex32: string,
  registryHex32: string,
  calldataHex: string,
): {
  type: 'contract_call';
  contract: string;
  calldata: string;
  access_list: { read: string[]; write: string[] };
} {
  return {
    type: 'contract_call',
    contract: validateHex32(registryHex32).toLowerCase(),
    calldata: normalizeCalldataHex(calldataHex),
    access_list: buildBuilderAttestationRegistryAccessList(senderHex32, registryHex32),
  };
}

export function mergeBuilderAttestationRegistryAccessListWithSimulation(
  senderHex32: string,
  registryHex32: string,
  sim: SimulateResult,
): { read: string[]; write: string[] } {
  const base = buildBuilderAttestationRegistryAccessList(senderHex32, registryHex32);
  return mergeAccessListWithSimulation(base.read, base.write, sim);
}

/**
 * Build Express `contract_call` txs to claim both assets and attest once.
 * Caller must be (or become) claimer of both AccountIds — typically the joint deployer.
 * Companion links are app policy only (not checked on-chain).
 */
export function buildBuilderAttestationAttestFlowTxs(input: {
  senderHex32: string;
  registryHex32: string;
  collectionHex32: string;
  tokenHex32: string;
  noteHashHex32?: string;
}): Array<ReturnType<typeof buildBuilderAttestationRegistryContractCallTx>> {
  const {
    senderHex32,
    registryHex32,
    collectionHex32,
    tokenHex32,
    noteHashHex32 = ZERO32,
  } = input;
  return [
    buildBuilderAttestationRegistryContractCallTx(
      senderHex32,
      registryHex32,
      encodeBuilderAttestClaimAssetCalldataHex(collectionHex32),
    ),
    buildBuilderAttestationRegistryContractCallTx(
      senderHex32,
      registryHex32,
      encodeBuilderAttestClaimAssetCalldataHex(tokenHex32),
    ),
    buildBuilderAttestationRegistryContractCallTx(
      senderHex32,
      registryHex32,
      encodeBuilderAttestAttestCalldataHex(collectionHex32, tokenHex32, noteHashHex32),
    ),
  ];
}
