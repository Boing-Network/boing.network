import { describe, expect, it } from 'vitest';
import {
  buildHostInterfaceBlobFlow,
  decodeInterfaceBlobWords,
  encodeInterfaceBlobPutCalldatasHex,
  IFACE_BLOB_ENCODING_DEFLATE_RAW,
  IFACE_BLOB_ENCODING_RAW,
  interfaceBlobId,
  interfaceBlobStorageKeyHex,
  interfaceBlobUri,
  interfaceBlobWords,
  interfaceJsonHashHex,
  parseInterfaceBlobUri,
  readInterfaceBlob,
  CANONICAL_BOING_TESTNET_INTERFACE_BLOB_STORE_HEX,
} from '../src/index.js';
import { hexToBytes } from '../src/hex.js';

const W = `0x${'ab'.repeat(32)}`;

describe('interface blob store', () => {
  it('storage key matches the Rust/contract layout', () => {
    // writer ^ "BOING_IFACE_BLOB_V1" ^ (id << 32) ^ index (also asserted in interface_blob_store.rs)
    expect(interfaceBlobStorageKeyHex(W, 0x0123456789abcdefn, 5)).toBe(
      '0xe9e4e2e5ecf4e2edeae8eef4e9e7e4e9f4fd9aabaa88eecc22006644abababae',
    );
  });

  it('uri, id, words and put calldata', () => {
    const h = interfaceJsonHashHex('{"functions":[]}');
    const id = interfaceBlobId(h);
    const uri = interfaceBlobUri(W, id);
    expect(uri.length).toBeLessThanOrEqual(192);
    expect(parseInterfaceBlobUri(uri)).toEqual({ writer: W, id });
    expect(parseInterfaceBlobUri('https://x/y.json')).toBeNull();
    const data = new Uint8Array(3000).map((_, i) => i % 251);
    const words = interfaceBlobWords(data, IFACE_BLOB_ENCODING_RAW);
    expect(words.length).toBe(1 + Math.ceil(3000 / 32));
    expect(decodeInterfaceBlobWords(words)?.stored).toEqual(data);
    const cds = encodeInterfaceBlobPutCalldatasHex(id, words);
    expect(cds.length).toBe(Math.ceil(words.length / 64));
    const first = hexToBytes(cds[0]);
    expect(first[31]).toBe(0xe0);
    expect(first.length).toBe(128 + 64 * 32);
    expect(() => interfaceBlobWords(new Uint8Array(0), 0)).toThrow();
  });

  it('host flow compresses and reads back through a fake chain', async () => {
    const json = new TextEncoder().encode(JSON.stringify({ functions: Array.from({ length: 30 }, (_, i) => ({ selector: i, name: `fn_${i}`, kind: 'write' })) }, null, 2));
    const h = interfaceJsonHashHex(json);
    const flow = await buildHostInterfaceBlobFlow({ senderHex32: W, interfaceHashHex32: h, jsonBytes: json });
    expect(flow.encoding).toBe(IFACE_BLOB_ENCODING_DEFLATE_RAW);
    expect(flow.storedBytes).toBeLessThan(json.length);
    expect(flow.txs.every((t) => t.contract === CANONICAL_BOING_TESTNET_INTERFACE_BLOB_STORE_HEX.toLowerCase())).toBe(true);
    // apply the puts to a fake storage map, as the contract would
    const storage = new Map<string, Uint8Array>();
    for (const t of flow.txs) {
      const cd = hexToBytes(t.calldata);
      const view = (o: number) => Number(BigInt('0x' + Buffer.from(cd.subarray(o + 24, o + 32)).toString('hex')));
      const start = view(64);
      const count = view(96);
      for (let i = 0; i < count; i++) {
        storage.set(interfaceBlobStorageKeyHex(W, flow.id, start + i), cd.slice(128 + 32 * i, 160 + 32 * i));
      }
    }
    const back = await readInterfaceBlob(flow.uri, async (_s, k) => storage.get(k) ?? new Uint8Array(32));
    expect(interfaceJsonHashHex(back)).toBe(h);
    await expect(readInterfaceBlob(interfaceBlobUri(W, 1n), async () => new Uint8Array(32))).rejects.toThrow(/not found/);
  });
});
