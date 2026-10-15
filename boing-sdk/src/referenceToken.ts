/**
 * Reference fungible calldata layout (Boing-defined). See `docs/BOING-REFERENCE-TOKEN.md`.
 */

import { bytesToHex } from './hex.js';
import {
  calldataAccountIdWord,
  calldataSelectorLastByte,
  calldataU128BeWord,
} from './calldata.js';

/** Selector low byte for reference `transfer`. */
export const SELECTOR_TRANSFER = 0x01;
/** Selector low byte for reference first-mint style hook. */
export const SELECTOR_MINT_FIRST = 0x02;

/** Build 96-byte reference `transfer(to, amount)` calldata. */
export function encodeReferenceTransferCalldata(toHexAccount32: string, amount: bigint): Uint8Array {
  const out = new Uint8Array(96);
  out.set(calldataSelectorLastByte(SELECTOR_TRANSFER), 0);
  out.set(calldataAccountIdWord(toHexAccount32), 32);
  out.set(calldataU128BeWord(amount), 64);
  return out;
}

/** Build 96-byte reference `mint_first` calldata. */
export function encodeReferenceMintFirstCalldata(toHexAccount32: string, amount: bigint): Uint8Array {
  const out = new Uint8Array(96);
  out.set(calldataSelectorLastByte(SELECTOR_MINT_FIRST), 0);
  out.set(calldataAccountIdWord(toHexAccount32), 32);
  out.set(calldataU128BeWord(amount), 64);
  return out;
}

/** Hex `0x` + 96-byte reference transfer calldata. */
export function encodeReferenceTransferCalldataHex(toHexAccount32: string, amount: bigint): string {
  return bytesToHex(encodeReferenceTransferCalldata(toHexAccount32, amount));
}

/** Reference fungible **v2** selectors (`approve`, `transfer_from`, `allowance`). */
export const SELECTOR_APPROVE = 0x08;
export const SELECTOR_TRANSFER_FROM = 0x09;
export const SELECTOR_ALLOWANCE = 0x0a;

/** v2: 96-byte `approve(spender, amount)` calldata (overwrites the allowance). */
export function encodeReferenceApproveCalldata(spenderHexAccount32: string, amount: bigint): Uint8Array {
  const out = new Uint8Array(96);
  out.set(calldataSelectorLastByte(SELECTOR_APPROVE), 0);
  out.set(calldataAccountIdWord(spenderHexAccount32), 32);
  out.set(calldataU128BeWord(amount), 64);
  return out;
}

/** v2: 128-byte `transfer_from(from, to, amount)` calldata. */
export function encodeReferenceTransferFromCalldata(fromHex32: string, toHex32: string, amount: bigint): Uint8Array {
  const out = new Uint8Array(128);
  out.set(calldataSelectorLastByte(SELECTOR_TRANSFER_FROM), 0);
  out.set(calldataAccountIdWord(fromHex32), 32);
  out.set(calldataAccountIdWord(toHex32), 64);
  out.set(calldataU128BeWord(amount), 96);
  return out;
}

/** v2: 96-byte `allowance(owner, spender)` calldata (returns the allowance word). */
export function encodeReferenceAllowanceCalldata(ownerHex32: string, spenderHex32: string): Uint8Array {
  const out = new Uint8Array(96);
  out.set(calldataSelectorLastByte(SELECTOR_ALLOWANCE), 0);
  out.set(calldataAccountIdWord(ownerHex32), 32);
  out.set(calldataAccountIdWord(spenderHex32), 64);
  return out;
}
