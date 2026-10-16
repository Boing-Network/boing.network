/**
 * Built-in hosting for interface JSON ("Host on Boing"): the interface blob store contract.
 *
 * The (optionally deflate-raw compressed) JSON is written into the caller's own namespace of the
 * blob store and the registry records `boing://iface-blob/0x<writer>/<id>` as the URI. Readers
 * fetch the words with `boing_getContractStorage`, decode, and verify BLAKE3 against the registry
 * record exactly as for an https URI. Mirrors `boing_execution::interface_blob_store`.
 * See docs/INTERFACE-REGISTRY.md (Built-in hosting).
 */
import { bytesToHex, hexToBytes, validateHex32 } from './hex.js';

/** Public testnet interface blob store (CREATE2 from the canonical DEX deployer, salt `BOING_IFACE_BLOB_STORE_V1`). */
export const CANONICAL_BOING_TESTNET_INTERFACE_BLOB_STORE_HEX = validateHex32(
  '0xa524837577e67228bf676257e861d29e510a3eddd1a4dbd607f613bbcd37904d',
);
export const SELECTOR_IFACE_BLOB_PUT = 0xe0;
export const IFACE_BLOB_MAX_WORDS_PER_PUT = 64;
/** Client-side cap on stored bytes (after compression). */
export const IFACE_BLOB_MAX_STORED_BYTES = 24 * 1024;
export const IFACE_BLOB_ENCODING_RAW = 0;
export const IFACE_BLOB_ENCODING_DEFLATE_RAW = 1;
export const IFACE_BLOB_URI_PREFIX = 'boing://iface-blob/';
const MAGIC = [0x42, 0x49, 0x46, 0x31]; // "BIF1"

function asciiWord(s: string): Uint8Array {
  const w = new Uint8Array(32);
  w.set(new TextEncoder().encode(s));
  return w;
}
export const IFACE_BLOB_TAG = asciiWord('BOING_IFACE_BLOB_V1');
export const INTERFACE_BLOB_STORE_CREATE2_SALT_V1_HEX = bytesToHex(asciiWord('BOING_IFACE_BLOB_STORE_V1'));

function u64Word(n: bigint | number): Uint8Array {
  const w = new Uint8Array(32);
  let v = BigInt(n);
  for (let i = 31; i >= 24; i--) {
    w[i] = Number(v & 0xffn);
    v >>= 8n;
  }
  return w;
}

/** Storage key of word `index` of blob `id` written by `writer`: `writer ^ TAG ^ (id << 32) ^ index`. */
export function interfaceBlobStorageKeyHex(writerHex32: string, id: bigint, index: number): string {
  const k = hexToBytes(validateHex32(writerHex32));
  for (let i = 0; i < 32; i++) k[i] ^= IFACE_BLOB_TAG[i];
  let v = BigInt.asUintN(64, id);
  for (let i = 27; i >= 20; i--) {
    k[i] ^= Number(v & 0xffn);
    v >>= 8n;
  }
  let x = index >>> 0;
  for (let i = 31; i >= 28; i--) {
    k[i] ^= x & 0xff;
    x >>>= 8;
  }
  return bytesToHex(k);
}

/** Blob id for an interface hash: its first 8 bytes, big endian. */
export function interfaceBlobId(interfaceHashHex32: string): bigint {
  const h = hexToBytes(validateHex32(interfaceHashHex32));
  let v = 0n;
  for (let i = 0; i < 8; i++) v = (v << 8n) | BigInt(h[i]);
  return v;
}

export function interfaceBlobUri(writerHex32: string, id: bigint): string {
  return `${IFACE_BLOB_URI_PREFIX}${validateHex32(writerHex32).toLowerCase()}/${BigInt.asUintN(64, id).toString(16).padStart(16, '0')}`;
}

export function isInterfaceBlobUri(uri: string): boolean {
  return typeof uri === 'string' && uri.startsWith(IFACE_BLOB_URI_PREFIX);
}

export function parseInterfaceBlobUri(uri: string): { writer: string; id: bigint } | null {
  if (!isInterfaceBlobUri(uri)) return null;
  const m = /^0x([0-9a-fA-F]{64})\/([0-9a-fA-F]{16})$/.exec(uri.slice(IFACE_BLOB_URI_PREFIX.length));
  if (!m) return null;
  return { writer: `0x${m[1].toLowerCase()}`, id: BigInt(`0x${m[2]}`) };
}

/** Header word + data words for `stored` bytes (already encoded as `encoding`). */
export function interfaceBlobWords(stored: Uint8Array, encoding: number): Uint8Array[] {
  if (stored.length === 0 || stored.length > IFACE_BLOB_MAX_STORED_BYTES) {
    throw new Error(`blob is ${stored.length} bytes; must be 1..${IFACE_BLOB_MAX_STORED_BYTES}`);
  }
  const header = u64Word(stored.length);
  header.set(MAGIC, 0);
  header[4] = encoding;
  const out = [header];
  for (let i = 0; i < stored.length; i += 32) {
    const w = new Uint8Array(32);
    w.set(stored.subarray(i, i + 32));
    out.push(w);
  }
  return out;
}

/** Parse the header word: `{ encoding, length, words }` (words = data words to read), or null. */
export function parseInterfaceBlobHeader(header: Uint8Array): { encoding: number; length: number; words: number } | null {
  if (header.length !== 32 || MAGIC.some((b, i) => header[i] !== b)) return null;
  let len = 0;
  for (let i = 24; i < 32; i++) len = len * 256 + header[i];
  if (len === 0 || len > IFACE_BLOB_MAX_STORED_BYTES) return null;
  return { encoding: header[4], length: len, words: Math.ceil(len / 32) };
}

export function decodeInterfaceBlobWords(words: Uint8Array[]): { encoding: number; stored: Uint8Array } | null {
  const h = words[0] && parseInterfaceBlobHeader(words[0]);
  if (!h || words.length < 1 + h.words) return null;
  const b = new Uint8Array(h.words * 32);
  for (let i = 0; i < h.words; i++) b.set(words[1 + i], i * 32);
  return { encoding: h.encoding, stored: b.subarray(0, h.length) };
}

/** `put(id, start, count, words)` calldatas, at most 64 words each. */
export function encodeInterfaceBlobPutCalldatasHex(id: bigint, words: Uint8Array[]): string[] {
  const out: string[] = [];
  for (let start = 0; start < words.length; start += IFACE_BLOB_MAX_WORDS_PER_PUT) {
    const chunk = words.slice(start, start + IFACE_BLOB_MAX_WORDS_PER_PUT);
    const v = new Uint8Array(128 + 32 * chunk.length);
    v[31] = SELECTOR_IFACE_BLOB_PUT;
    v.set(u64Word(BigInt.asUintN(64, id)), 32);
    v.set(u64Word(start), 64);
    v.set(u64Word(chunk.length), 96);
    chunk.forEach((w, i) => v.set(w, 128 + 32 * i));
    out.push(bytesToHex(v));
  }
  return out;
}

async function streamBytes(input: Uint8Array, stream: { readable: ReadableStream; writable: WritableStream }): Promise<Uint8Array> {
  const w = stream.writable.getWriter();
  w.write(input).then(() => w.close()).catch(() => undefined);
  const buf = await new Response(stream.readable).arrayBuffer();
  return new Uint8Array(buf);
}

/** deflate-raw via the platform CompressionStream (browsers, Node 20.12+). */
export async function deflateRawBytes(input: Uint8Array): Promise<Uint8Array> {
  const CS = (globalThis as { CompressionStream?: new (f: string) => { readable: ReadableStream; writable: WritableStream } }).CompressionStream;
  if (!CS) throw new Error('CompressionStream is not available');
  return streamBytes(input, new CS('deflate-raw'));
}

export async function inflateRawBytes(input: Uint8Array): Promise<Uint8Array> {
  const DS = (globalThis as { DecompressionStream?: new (f: string) => { readable: ReadableStream; writable: WritableStream } }).DecompressionStream;
  if (!DS) throw new Error('DecompressionStream is not available');
  return streamBytes(input, new DS('deflate-raw'));
}

export type InterfaceBlobContractCallTx = {
  type: 'contract_call';
  contract: string;
  calldata: string;
  access_list: { read: string[]; write: string[] };
};

/**
 * Plan built-in hosting for the exact interface JSON bytes: compresses when that is smaller,
 * returns the `boing://` URI to record in the registry and the blob store `put` calls.
 */
export async function buildHostInterfaceBlobFlow(input: {
  senderHex32: string;
  interfaceHashHex32: string;
  jsonBytes: Uint8Array;
  storeHex32?: string;
  compress?: boolean;
}): Promise<{ uri: string; id: bigint; encoding: number; storedBytes: number; words: number; txs: InterfaceBlobContractCallTx[] }> {
  const store = validateHex32(input.storeHex32 ?? CANONICAL_BOING_TESTNET_INTERFACE_BLOB_STORE_HEX).toLowerCase();
  const sender = validateHex32(input.senderHex32).toLowerCase();
  let stored = input.jsonBytes;
  let encoding = IFACE_BLOB_ENCODING_RAW;
  if (input.compress !== false) {
    try {
      const z = await deflateRawBytes(input.jsonBytes);
      if (z.length < stored.length) {
        stored = z;
        encoding = IFACE_BLOB_ENCODING_DEFLATE_RAW;
      }
    } catch {
      /* no CompressionStream: store raw */
    }
  }
  const id = interfaceBlobId(input.interfaceHashHex32);
  const words = interfaceBlobWords(stored, encoding);
  const txs = encodeInterfaceBlobPutCalldatasHex(id, words).map((calldata) => ({
    type: 'contract_call' as const,
    contract: store,
    calldata,
    access_list: { read: [sender, store], write: [sender, store] },
  }));
  return { uri: interfaceBlobUri(sender, id), id, encoding, storedBytes: stored.length, words: words.length, txs };
}

/** Reads one storage word of `store` (hex in, 32 bytes out). */
export type StorageWordReader = (storeHex32: string, keyHex32: string) => Promise<Uint8Array>;

/** Storage reader over a JSON-RPC call function (`boing_getContractStorage`). */
export function storageWordReaderFromRpc(call: (method: string, params: unknown[]) => Promise<unknown>): StorageWordReader {
  return async (store, key) => {
    const r = (await call('boing_getContractStorage', [store, key])) as { value?: string } | null;
    const b = r?.value ? hexToBytes(r.value) : new Uint8Array(0);
    const w = new Uint8Array(32);
    if (b.length === 32) w.set(b);
    return w;
  };
}

/**
 * Read and decode a `boing://iface-blob/...` URI into the exact JSON bytes.
 * The caller still verifies BLAKE3(bytes) against the registry record.
 */
export async function readInterfaceBlob(
  uri: string,
  read: StorageWordReader,
  opts: { storeHex32?: string; inflate?: (b: Uint8Array) => Promise<Uint8Array> } = {},
): Promise<Uint8Array> {
  const p = parseInterfaceBlobUri(uri);
  if (!p) throw new Error('not a boing://iface-blob/ URI');
  const store = validateHex32(opts.storeHex32 ?? CANONICAL_BOING_TESTNET_INTERFACE_BLOB_STORE_HEX);
  const header = await read(store, interfaceBlobStorageKeyHex(p.writer, p.id, 0));
  const h = parseInterfaceBlobHeader(header);
  if (!h) throw new Error('interface blob not found (empty or bad header)');
  const words: Uint8Array[] = [];
  for (let i = 0; i < h.words; i += 16) {
    const n = Math.min(16, h.words - i);
    words.push(
      ...(await Promise.all(
        Array.from({ length: n }, (_, j) => read(store, interfaceBlobStorageKeyHex(p.writer, p.id, i + j + 1))),
      )),
    );
  }
  const decoded = decodeInterfaceBlobWords([header, ...words]);
  if (!decoded) throw new Error('interface blob is incomplete');
  if (decoded.encoding === IFACE_BLOB_ENCODING_RAW) return decoded.stored;
  if (decoded.encoding === IFACE_BLOB_ENCODING_DEFLATE_RAW) return (opts.inflate ?? inflateRawBytes)(decoded.stored);
  throw new Error(`unknown interface blob encoding ${decoded.encoding}`);
}
