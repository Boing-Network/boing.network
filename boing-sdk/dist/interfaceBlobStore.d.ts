/** Public testnet interface blob store (CREATE2 from the canonical DEX deployer, salt `BOING_IFACE_BLOB_STORE_V1`). */
export declare const CANONICAL_BOING_TESTNET_INTERFACE_BLOB_STORE_HEX: string;
export declare const SELECTOR_IFACE_BLOB_PUT = 224;
export declare const IFACE_BLOB_MAX_WORDS_PER_PUT = 64;
/** Client-side cap on stored bytes (after compression). */
export declare const IFACE_BLOB_MAX_STORED_BYTES: number;
export declare const IFACE_BLOB_ENCODING_RAW = 0;
export declare const IFACE_BLOB_ENCODING_DEFLATE_RAW = 1;
export declare const IFACE_BLOB_URI_PREFIX = "boing://iface-blob/";
export declare const IFACE_BLOB_TAG: Uint8Array<ArrayBufferLike>;
export declare const INTERFACE_BLOB_STORE_CREATE2_SALT_V1_HEX: string;
/** Storage key of word `index` of blob `id` written by `writer`: `writer ^ TAG ^ (id << 32) ^ index`. */
export declare function interfaceBlobStorageKeyHex(writerHex32: string, id: bigint, index: number): string;
/** Blob id for an interface hash: its first 8 bytes, big endian. */
export declare function interfaceBlobId(interfaceHashHex32: string): bigint;
export declare function interfaceBlobUri(writerHex32: string, id: bigint): string;
export declare function isInterfaceBlobUri(uri: string): boolean;
export declare function parseInterfaceBlobUri(uri: string): {
    writer: string;
    id: bigint;
} | null;
/** Header word + data words for `stored` bytes (already encoded as `encoding`). */
export declare function interfaceBlobWords(stored: Uint8Array, encoding: number): Uint8Array[];
/** Parse the header word: `{ encoding, length, words }` (words = data words to read), or null. */
export declare function parseInterfaceBlobHeader(header: Uint8Array): {
    encoding: number;
    length: number;
    words: number;
} | null;
export declare function decodeInterfaceBlobWords(words: Uint8Array[]): {
    encoding: number;
    stored: Uint8Array;
} | null;
/** `put(id, start, count, words)` calldatas, at most 64 words each. */
export declare function encodeInterfaceBlobPutCalldatasHex(id: bigint, words: Uint8Array[]): string[];
/** deflate-raw via the platform CompressionStream (browsers, Node 20.12+). */
export declare function deflateRawBytes(input: Uint8Array): Promise<Uint8Array>;
export declare function inflateRawBytes(input: Uint8Array): Promise<Uint8Array>;
export type InterfaceBlobContractCallTx = {
    type: 'contract_call';
    contract: string;
    calldata: string;
    access_list: {
        read: string[];
        write: string[];
    };
};
/**
 * Plan built-in hosting for the exact interface JSON bytes: compresses when that is smaller,
 * returns the `boing://` URI to record in the registry and the blob store `put` calls.
 */
export declare function buildHostInterfaceBlobFlow(input: {
    senderHex32: string;
    interfaceHashHex32: string;
    jsonBytes: Uint8Array;
    storeHex32?: string;
    compress?: boolean;
}): Promise<{
    uri: string;
    id: bigint;
    encoding: number;
    storedBytes: number;
    words: number;
    txs: InterfaceBlobContractCallTx[];
}>;
/** Reads one storage word of `store` (hex in, 32 bytes out). */
export type StorageWordReader = (storeHex32: string, keyHex32: string) => Promise<Uint8Array>;
/** Storage reader over a JSON-RPC call function (`boing_getContractStorage`). */
export declare function storageWordReaderFromRpc(call: (method: string, params: unknown[]) => Promise<unknown>): StorageWordReader;
/**
 * Read and decode a `boing://iface-blob/...` URI into the exact JSON bytes.
 * The caller still verifies BLAKE3(bytes) against the registry record.
 */
export declare function readInterfaceBlob(uri: string, read: StorageWordReader, opts?: {
    storeHex32?: string;
    inflate?: (b: Uint8Array) => Promise<Uint8Array>;
}): Promise<Uint8Array>;
//# sourceMappingURL=interfaceBlobStore.d.ts.map