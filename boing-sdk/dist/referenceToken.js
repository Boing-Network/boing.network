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
