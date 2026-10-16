/**
 * Boing name registry helpers (`label.boing`) — mirrors `boing_primitives::names` and the `Name*`
 * transaction payloads (variants 13-19).
 *
 * Flow to claim a name: `NameCommit` (hides the label) → wait until it is included → `NameRegister`
 * revealing the label + salt (at least `commit_min_age` blocks later) → optionally `NameSetPrimary`.
 * Rent is burned: 3 chars 10, 4 chars 5, 5 chars 2, 6+ chars 1 BOING per year (testnet).
 */

import { blake3 } from '@noble/hashes/blake3';
import type { BoingClient } from './client.js';
import type { TransactionInput, TransactionPayloadInput } from './bincode.js';
import { bytesToHex, hexToAccountId, validateHex32 } from './hex.js';
import {
  fetchNextNonce,
  senderHexFromSecretKey,
  signTransactionInput,
  type Ed25519SecretKey32,
} from './transactionBuilder.js';

export const BOING_NAME_SUFFIX = '.boing';
export const BOING_NAME_MIN_LEN = 3;
export const BOING_NAME_MAX_LEN = 32;
export const BOING_NAME_MAX_YEARS = 10;
export const BOING_NAME_COMMIT_DOMAIN = 'BOING_NAME_COMMIT_V1';

export const BOING_RESERVED_NAMES: readonly string[] = [
  'boing', 'admin', 'administrator', 'root', 'system', 'validator', 'validators', 'faucet',
  'observer', 'wallet', 'support', 'help', 'official', 'team', 'dex', 'finance', 'express',
  'network', 'treasury', 'governance', 'security', 'staking', 'bridge', 'testnet', 'mainnet',
  'null', 'undefined', 'anchor', 'registry', 'names',
];

export type BoingNameValidation = { ok: true; label: string } | { ok: false; error: string };

/** Same rules as the node: trim, lowercase, drop `.boing`, 3-32 of a-z 0-9 '-', no edge/double hyphen, not address-like. */
export function normalizeBoingName(input: string): BoingNameValidation {
  let s = (input ?? '').trim().toLowerCase();
  if (s.endsWith(BOING_NAME_SUFFIX)) s = s.slice(0, -BOING_NAME_SUFFIX.length);
  if (s.length < BOING_NAME_MIN_LEN || s.length > BOING_NAME_MAX_LEN) {
    return { ok: false, error: `name must be ${BOING_NAME_MIN_LEN}-${BOING_NAME_MAX_LEN} characters` };
  }
  if (!/^[a-z0-9-]+$/.test(s)) return { ok: false, error: "name may only contain a-z, 0-9 and '-'" };
  if (s.startsWith('-') || s.endsWith('-') || s.includes('--')) {
    return { ok: false, error: "name cannot start or end with '-' or contain '--'" };
  }
  if (s.startsWith('0x') || /^[0-9]+$/.test(s) || (s.length >= 32 && /^[0-9a-f]+$/.test(s))) {
    return { ok: false, error: 'name cannot look like an address or be purely numeric' };
  }
  return { ok: true, label: s };
}

export function isReservedBoingName(label: string): boolean {
  return BOING_RESERVED_NAMES.includes(label);
}

/**
 * True when user input should be treated as a name (contains a letter / ends with `.boing`) rather than
 * a hex address. 64-hex (with or without 0x) is always an address.
 */
export function looksLikeBoingName(input: string): boolean {
  const s = (input ?? '').trim();
  if (!s) return false;
  if (/^(0x)?[0-9a-fA-F]{64}$/.test(s)) return false;
  return normalizeBoingName(s).ok;
}

/** Yearly rent in whole BOING (testnet). */
export function boingNameRentPerYear(label: string): bigint {
  const n = label.length;
  if (n <= 3) return 10n;
  if (n === 4) return 5n;
  if (n === 5) return 2n;
  return 1n;
}

export function boingNameRent(label: string, years: number): bigint {
  return boingNameRentPerYear(label) * BigInt(years);
}

export function displayBoingName(label: string): string {
  return `${label}${BOING_NAME_SUFFIX}`;
}

/** `blake3(domain ‖ u32le(len) ‖ label ‖ owner ‖ salt)` (matches Rust `name_commitment`). */
export function boingNameCommitment(label: string, ownerHex: string, salt: Uint8Array): Uint8Array {
  if (salt.length !== 32) throw new Error('salt must be 32 bytes');
  const dom = new TextEncoder().encode(BOING_NAME_COMMIT_DOMAIN);
  const l = new TextEncoder().encode(label);
  const owner = hexToAccountId(validateHex32(ownerHex));
  const len = new Uint8Array(4);
  new DataView(len.buffer).setUint32(0, l.length, true);
  const pre = new Uint8Array(dom.length + 4 + l.length + 32 + 32);
  let o = 0;
  for (const part of [dom, len, l, owner, salt]) {
    pre.set(part, o);
    o += part.length;
  }
  return blake3(pre);
}

/** 32 random bytes (WebCrypto). */
export function randomBoingNameSalt(): Uint8Array {
  const s = new Uint8Array(32);
  globalThis.crypto.getRandomValues(s);
  return s;
}

function selfTx(senderHex: string, nonce: bigint, payload: TransactionPayloadInput): TransactionInput {
  const id = hexToAccountId(validateHex32(senderHex));
  return { nonce, sender: id, payload, accessList: { read: [id], write: [id] } };
}

function labelOrThrow(name: string): string {
  const v = normalizeBoingName(name);
  if (!v.ok) throw new Error(v.error);
  return v.label;
}

export function buildNameCommitTransaction(input: { nonce: bigint; senderHex: string; name: string; salt: Uint8Array }): TransactionInput {
  const label = labelOrThrow(input.name);
  return selfTx(input.senderHex, input.nonce, {
    kind: 'nameCommit',
    commitment: boingNameCommitment(label, input.senderHex, input.salt),
  });
}

export function buildNameRegisterTransaction(input: {
  nonce: bigint;
  senderHex: string;
  name: string;
  salt: Uint8Array;
  targetHex?: string;
  years?: number;
}): TransactionInput {
  return selfTx(input.senderHex, input.nonce, {
    kind: 'nameRegister',
    name: labelOrThrow(input.name),
    salt: input.salt,
    target: hexToAccountId(validateHex32(input.targetHex ?? input.senderHex)),
    years: input.years ?? 1,
  });
}

export function buildNameRenewTransaction(input: { nonce: bigint; senderHex: string; name: string; years?: number }): TransactionInput {
  return selfTx(input.senderHex, input.nonce, { kind: 'nameRenew', name: labelOrThrow(input.name), years: input.years ?? 1 });
}

export function buildNameTransferTransaction(input: { nonce: bigint; senderHex: string; name: string; newOwnerHex: string }): TransactionInput {
  return selfTx(input.senderHex, input.nonce, {
    kind: 'nameTransfer',
    name: labelOrThrow(input.name),
    newOwner: hexToAccountId(validateHex32(input.newOwnerHex)),
  });
}

export function buildNameSetTargetTransaction(input: { nonce: bigint; senderHex: string; name: string; targetHex: string }): TransactionInput {
  return selfTx(input.senderHex, input.nonce, {
    kind: 'nameSetTarget',
    name: labelOrThrow(input.name),
    target: hexToAccountId(validateHex32(input.targetHex)),
  });
}

export function buildNameSetPrimaryTransaction(input: { nonce: bigint; senderHex: string; name: string | null }): TransactionInput {
  return selfTx(input.senderHex, input.nonce, {
    kind: 'nameSetPrimary',
    name: input.name == null ? null : labelOrThrow(input.name),
  });
}

export function buildNameReleaseTransaction(input: { nonce: bigint; senderHex: string; name: string }): TransactionInput {
  return selfTx(input.senderHex, input.nonce, { kind: 'nameRelease', name: labelOrThrow(input.name) });
}

// --- RPC types ---

export type BoingNameStatus = 'active' | 'grace' | 'expired';

export interface BoingNameRecord {
  name: string;
  display: string;
  owner: string;
  target: string;
  registered_at: number;
  expires_at: number;
  status: BoingNameStatus;
}

export interface BoingReverseName {
  name: string;
  display: string;
  address: string;
  verified: true;
}

export interface BoingNameRegistryInfo {
  enabled: boolean;
  active?: boolean;
  activation_height?: number;
  blocks_per_year?: number;
  grace_blocks?: number;
  commit_min_age?: number;
  commit_max_age?: number;
  rent_per_year?: Record<string, string>;
  max_years?: number;
}

export interface BoingNameCheck {
  input: string;
  name?: string;
  display?: string;
  valid: boolean;
  error?: string;
  reserved?: boolean;
  available: boolean;
  status?: BoingNameStatus | null;
  rent_per_year?: string;
}

export interface BoingNameInfo extends BoingNameCheck {
  record?: BoingNameRecord;
  registry: BoingNameRegistryInfo;
  head_height: number;
}

export interface BoingCheckNamesResult {
  registry: BoingNameRegistryInfo;
  results: BoingNameCheck[];
}

/**
 * Resolve user input to an address: 64-hex passes through (`0x` + lowercase); a name resolves via RPC.
 * Returns null when a name is not registered / not active.
 */
export async function resolveBoingRecipient(
  client: BoingClient,
  input: string,
): Promise<{ address: string; name: string | null } | null> {
  const s = (input ?? '').trim();
  const hex = s.replace(/^0x/i, '');
  if (/^[0-9a-fA-F]{64}$/.test(hex)) return { address: `0x${hex.toLowerCase()}`, name: null };
  const v = normalizeBoingName(s);
  if (!v.ok) return null;
  const rec = await client.resolveName(v.label);
  return rec ? { address: rec.target.toLowerCase(), name: v.label } : null;
}

async function signAndSubmit(client: BoingClient, tx: TransactionInput, secret: Ed25519SecretKey32) {
  const hex = await signTransactionInput(tx, secret);
  return client.submitTransaction(hex);
}

/**
 * Commit to a name (step 1). Keep `salt` (and the label) until `registerBoingName` succeeds; without
 * them the commitment cannot be revealed.
 */
export async function commitBoingName(client: BoingClient, secret: Ed25519SecretKey32, name: string, salt = randomBoingNameSalt()) {
  const senderHex = await senderHexFromSecretKey(secret);
  const nonce = await fetchNextNonce(client, senderHex);
  const tx = buildNameCommitTransaction({ nonce, senderHex, name, salt });
  return { salt, saltHex: bytesToHex(salt), result: await signAndSubmit(client, tx, secret) };
}

/** Reveal + register (step 2), after the commit is included and matured. */
export async function registerBoingName(
  client: BoingClient,
  secret: Ed25519SecretKey32,
  input: { name: string; salt: Uint8Array; targetHex?: string; years?: number },
) {
  const senderHex = await senderHexFromSecretKey(secret);
  const nonce = await fetchNextNonce(client, senderHex);
  return signAndSubmit(client, buildNameRegisterTransaction({ nonce, senderHex, ...input }), secret);
}

export async function setBoingPrimaryName(client: BoingClient, secret: Ed25519SecretKey32, name: string | null) {
  const senderHex = await senderHexFromSecretKey(secret);
  const nonce = await fetchNextNonce(client, senderHex);
  return signAndSubmit(client, buildNameSetPrimaryTransaction({ nonce, senderHex, name }), secret);
}