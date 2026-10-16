import { describe, expect, it } from 'vitest';
import {
  CANONICAL_BOING_TESTNET_INTERFACE_REGISTRY_HEX,
  decodeIfaceGetInterfaceReturn,
  encodeIfaceClaimAssetCalldataHex,
  encodeIfaceSetInterfaceCalldataHex,
  fetchInterfaceRecord,
  interfaceJsonHashHex,
  interfaceRegistryClaimerStorageKeyHex,
  interfaceRegistryFieldStorageKeyHex,
  predictInterfaceRegistryCreate2Address,
  buildPublishInterfaceFlowTxs,
  IFACE_FIELD_HASH,
  IFACE_FIELD_PUBLISHER,
  IFACE_FIELD_URI_LEN,
  IFACE_FIELD_URI0,
} from '../src/index.js';

const DEX_DEPLOYER = '0x8079f30559c02c5fe087c9ad1bc3b751a29de35992f3f8431734243e825f696e';
const C = '0x' + '11'.repeat(32);

describe('interface registry', () => {
  it('canonical testnet address is the CREATE2 address from the DEX deployer', () => {
    expect(predictInterfaceRegistryCreate2Address(DEX_DEPLOYER)).toBe(CANONICAL_BOING_TESTNET_INTERFACE_REGISTRY_HEX);
  });

  it('encodes calldata with the right lengths and selectors', () => {
    const claim = encodeIfaceClaimAssetCalldataHex(C);
    expect((claim.length - 2) / 2).toBe(64);
    expect(claim.slice(2 + 62, 2 + 64)).toBe('d0');
    const set = encodeIfaceSetInterfaceCalldataHex(C, '0x' + 'ab'.repeat(32), 'https://x/y.json');
    expect((set.length - 2) / 2).toBe(320);
    expect(set.slice(2 + 62, 2 + 64)).toBe('d1');
    expect(() => encodeIfaceSetInterfaceCalldataHex(C, '0x' + 'ab'.repeat(32), 'x'.repeat(193))).toThrow();
  });

  it('decodes get_interface return data', () => {
    const uri = 'https://boing.network/interfaces/testnet/a.json';
    const u = new TextEncoder().encode(uri);
    const b = new Uint8Array(320);
    b.fill(0xaa, 0, 32);
    b.fill(0xbb, 32, 64);
    b.fill(0xcc, 64, 96);
    b[127] = u.length;
    b.set(u, 128);
    const hex = '0x' + Array.from(b, (x) => x.toString(16).padStart(2, '0')).join('');
    expect(decodeIfaceGetInterfaceReturn(hex)).toEqual({
      claimer: '0x' + 'aa'.repeat(32),
      publisher: '0x' + 'bb'.repeat(32),
      interfaceHash: '0x' + 'cc'.repeat(32),
      uri,
    });
    expect(decodeIfaceGetInterfaceReturn('0x' + '00'.repeat(320))).toBeNull();
  });

  it('reads a record from storage words', async () => {
    const uri = 'https://boing.network/interfaces/testnet/0x' + '11'.repeat(32) + '.json';
    const u = new TextEncoder().encode(uri);
    const hash = interfaceJsonHashHex('{"functions":[]}');
    const store = new Map<string, string>();
    const w = (bytes: Uint8Array) => '0x' + Array.from(bytes, (x) => x.toString(16).padStart(2, '0')).join('');
    store.set(interfaceRegistryClaimerStorageKeyHex(C), DEX_DEPLOYER);
    store.set(interfaceRegistryFieldStorageKeyHex(C, IFACE_FIELD_PUBLISHER), DEX_DEPLOYER);
    store.set(interfaceRegistryFieldStorageKeyHex(C, IFACE_FIELD_HASH), hash);
    const len = new Uint8Array(32);
    len[31] = u.length;
    store.set(interfaceRegistryFieldStorageKeyHex(C, IFACE_FIELD_URI_LEN), w(len));
    for (let i = 0; i * 32 < u.length; i++) {
      const word = new Uint8Array(32);
      word.set(u.slice(i * 32, i * 32 + 32));
      store.set(interfaceRegistryFieldStorageKeyHex(C, IFACE_FIELD_URI0 + i), w(word));
    }
    const reader = async (_reg: string, key: string) => ({ value: store.get(key) ?? '0x' + '00'.repeat(32) });
    const r = await fetchInterfaceRecord(reader, C);
    expect(r.claimer).toBe(DEX_DEPLOYER);
    expect(r.record).toEqual({ claimer: DEX_DEPLOYER, publisher: DEX_DEPLOYER, interfaceHash: hash, uri });
    const none = await fetchInterfaceRecord(async () => ({ value: '0x' + '00'.repeat(32) }), C);
    expect(none).toEqual({ claimer: null, record: null });
  });

  it('builds claim + set flow', () => {
    const txs = buildPublishInterfaceFlowTxs({
      senderHex32: DEX_DEPLOYER,
      contractHex32: C,
      interfaceHashHex32: '0x' + 'ab'.repeat(32),
      uri: 'https://x',
    });
    expect(txs).toHaveLength(2);
    expect(txs[0]!.contract).toBe(CANONICAL_BOING_TESTNET_INTERFACE_REGISTRY_HEX);
    expect(buildPublishInterfaceFlowTxs({ senderHex32: DEX_DEPLOYER, contractHex32: C, interfaceHashHex32: '0x' + 'ab'.repeat(32), uri: 'x', alreadyClaimed: true })).toHaveLength(1);
  });
});