/**
 * Reference fungible calldata layout (Boing-defined). See `docs/BOING-REFERENCE-TOKEN.md`.
 */
/** Selector low byte for reference `transfer`. */
export declare const SELECTOR_TRANSFER = 1;
/** Selector low byte for reference first-mint style hook. */
export declare const SELECTOR_MINT_FIRST = 2;
/** Build 96-byte reference `transfer(to, amount)` calldata. */
export declare function encodeReferenceTransferCalldata(toHexAccount32: string, amount: bigint): Uint8Array;
/** Build 96-byte reference `mint_first` calldata. */
export declare function encodeReferenceMintFirstCalldata(toHexAccount32: string, amount: bigint): Uint8Array;
/** Hex `0x` + 96-byte reference transfer calldata. */
export declare function encodeReferenceTransferCalldataHex(toHexAccount32: string, amount: bigint): string;
/** Reference fungible **v2** selectors (`approve`, `transfer_from`, `allowance`). */
export declare const SELECTOR_APPROVE = 8;
export declare const SELECTOR_TRANSFER_FROM = 9;
export declare const SELECTOR_ALLOWANCE = 10;
/** v2: 96-byte `approve(spender, amount)` calldata (overwrites the allowance). */
export declare function encodeReferenceApproveCalldata(spenderHexAccount32: string, amount: bigint): Uint8Array;
/** v2: 128-byte `transfer_from(from, to, amount)` calldata. */
export declare function encodeReferenceTransferFromCalldata(fromHex32: string, toHex32: string, amount: bigint): Uint8Array;
/** v2: 96-byte `allowance(owner, spender)` calldata (returns the allowance word). */
export declare function encodeReferenceAllowanceCalldata(ownerHex32: string, spenderHex32: string): Uint8Array;
//# sourceMappingURL=referenceToken.d.ts.map