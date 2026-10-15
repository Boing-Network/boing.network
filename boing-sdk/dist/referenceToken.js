/**
 * Reference fungible calldata layout (Boing-defined). See `docs/BOING-REFERENCE-TOKEN.md`.
 */
import { bytesToHex } from './hex.js';
import { calldataAccountIdWord, calldataSelectorLastByte, calldataU128BeWord, } from './calldata.js';
/** Selector low byte for reference `transfer`. */
export const SELECTOR_TRANSFER = 0x01;
/** Selector low byte for reference first-mint style hook. */
export const SELECTOR_MINT_FIRST = 0x02;
/** Build 96-byte reference `transfer(to, amount)` calldata. */
export function encodeReferenceTransferCalldata(toHexAccount32, amount) {
    const out = new Uint8Array(96);
    out.set(calldataSelectorLastByte(SELECTOR_TRANSFER), 0);
    out.set(calldataAccountIdWord(toHexAccount32), 32);
    out.set(calldataU128BeWord(amount), 64);
    return out;
}
/** Build 96-byte reference `mint_first` calldata. */
export function encodeReferenceMintFirstCalldata(toHexAccount32, amount) {
    const out = new Uint8Array(96);
    out.set(calldataSelectorLastByte(SELECTOR_MINT_FIRST), 0);
    out.set(calldataAccountIdWord(toHexAccount32), 32);
    out.set(calldataU128BeWord(amount), 64);
    return out;
}
/** Hex `0x` + 96-byte reference transfer calldata. */
export function encodeReferenceTransferCalldataHex(toHexAccount32, amount) {
    return bytesToHex(encodeReferenceTransferCalldata(toHexAccount32, amount));
}
/** Reference fungible **v2** selectors (`approve`, `transfer_from`, `allowance`). */
export const SELECTOR_APPROVE = 0x08;
export const SELECTOR_TRANSFER_FROM = 0x09;
export const SELECTOR_ALLOWANCE = 0x0a;
/** v2: 96-byte `approve(spender, amount)` calldata (overwrites the allowance). */
export function encodeReferenceApproveCalldata(spenderHexAccount32, amount) {
    const out = new Uint8Array(96);
    out.set(calldataSelectorLastByte(SELECTOR_APPROVE), 0);
    out.set(calldataAccountIdWord(spenderHexAccount32), 32);
    out.set(calldataU128BeWord(amount), 64);
    return out;
}
/** v2: 128-byte `transfer_from(from, to, amount)` calldata. */
export function encodeReferenceTransferFromCalldata(fromHex32, toHex32, amount) {
    const out = new Uint8Array(128);
    out.set(calldataSelectorLastByte(SELECTOR_TRANSFER_FROM), 0);
    out.set(calldataAccountIdWord(fromHex32), 32);
    out.set(calldataAccountIdWord(toHex32), 64);
    out.set(calldataU128BeWord(amount), 96);
    return out;
}
/** v2: 96-byte `allowance(owner, spender)` calldata (returns the allowance word). */
export function encodeReferenceAllowanceCalldata(ownerHex32, spenderHex32) {
    const out = new Uint8Array(96);
    out.set(calldataSelectorLastByte(SELECTOR_ALLOWANCE), 0);
    out.set(calldataAccountIdWord(ownerHex32), 32);
    out.set(calldataAccountIdWord(spenderHex32), 64);
    return out;
}
/**
 * `true` when `calldata` is a reference-style `transfer(to, amount)` (selector `0x01`, 96 bytes; also the
 * secured template and LP share token) whose `to` is `senderHex`. On v1 of those templates a
 * self-transfer credits without debiting (balance doubling, known bug); on v2 it is a no-op.
 * Wallets should refuse or warn before signing such a call.
 */
export function isReferenceSelfTransferCalldata(senderHex32, calldata) {
    const b = typeof calldata === 'string' ? hexToBytesLoose(calldata) : calldata;
    if (b == null || b.length !== 96)
        return false;
    for (let i = 0; i < 31; i++)
        if (b[i] !== 0)
            return false;
    if (b[31] !== SELECTOR_TRANSFER)
        return false;
    const to = bytesToHex(b.subarray(32, 64)).toLowerCase();
    return to === senderHex32.trim().toLowerCase();
}
function hexToBytesLoose(h) {
    const s = h.trim().replace(/^0x/i, '');
    if (s.length % 2 !== 0 || /[^0-9a-f]/i.test(s))
        return null;
    const out = new Uint8Array(s.length / 2);
    for (let i = 0; i < out.length; i++)
        out[i] = parseInt(s.slice(i * 2, i * 2 + 2), 16);
    return out;
}
