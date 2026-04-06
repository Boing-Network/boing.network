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
//# sourceMappingURL=referenceToken.d.ts.map