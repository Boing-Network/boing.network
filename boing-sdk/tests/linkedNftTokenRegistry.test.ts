import { describe, expect, it } from 'vitest';
import { hexToBytes, validateHex32 } from '../src/hex.js';
import { predictLinkedNftTokenRegistryCreate2Address } from '../src/create2.js';
import {
  LINKED_NFT_TOKEN_REGISTRY_CREATE2_SALT_V1_HEX,
  LINKED_NFT_TOKEN_REGISTRY_COUNT_KEY_HEX,
  LINKED_NFT_TOKEN_TOPIC_REGISTER_HEX,
  SELECTOR_LINKED_NFT_TOKEN_CLAIM_ASSET,
  SELECTOR_LINKED_NFT_TOKEN_REGISTER_LINK,
  buildLinkedNftTokenRegisterFlowTxs,
  buildLinkedNftTokenRegistryDeployMetaTx,
  decodeLinkedNftTokenGetLinkAtReturnData,
  decodeLinkedNftTokenLinksCountReturnData,
  encodeLinkedNftTokenClaimAssetCalldata,
  encodeLinkedNftTokenGetLinkAtCalldata,
  encodeLinkedNftTokenRegisterLinkCalldata,
  encodeLinkedNftTokenUnlinkAtCalldata,
  resolveLinkedNftTokenRegistryBytecodeHex,
} from '../src/linkedNftTokenRegistry.js';

const A = `0x${'11'.repeat(32)}`;
const B = `0x${'22'.repeat(32)}`;
const SENDER = `0x${'aa'.repeat(32)}`;
const REG = `0x${'bb'.repeat(32)}`;

describe('linkedNftTokenRegistry', () => {
  it('encodes claim / register / unlink lengths and selectors', () => {
    const claim = encodeLinkedNftTokenClaimAssetCalldata(A);
    expect(claim.length).toBe(64);
    expect(claim[31]).toBe(SELECTOR_LINKED_NFT_TOKEN_CLAIM_ASSET);

    const reg = encodeLinkedNftTokenRegisterLinkCalldata(A, B);
    expect(reg.length).toBe(96);
    expect(reg[31]).toBe(SELECTOR_LINKED_NFT_TOKEN_REGISTER_LINK);

    const un = encodeLinkedNftTokenUnlinkAtCalldata(3);
    expect(un.length).toBe(64);
    expect(encodeLinkedNftTokenGetLinkAtCalldata(0).length).toBe(64);
  });

  it('builds registry deploy meta with CREATE2 salt and dapp purpose', () => {
    const tx = buildLinkedNftTokenRegistryDeployMetaTx();
    expect(tx.type).toBe('contract_deploy_meta');
    expect(tx.purpose_category).toBe('dapp');
    expect(tx.create2_salt).toBe(LINKED_NFT_TOKEN_REGISTRY_CREATE2_SALT_V1_HEX);
    expect(tx.bytecode.startsWith('0x')).toBe(true);
    expect(tx.bytecode.length).toBeGreaterThan(200);
    expect(resolveLinkedNftTokenRegistryBytecodeHex()).toBe(tx.bytecode);
  });

  it('predicts CREATE2 address from pinned bytecode + salt', () => {
    const code = hexToBytes(resolveLinkedNftTokenRegistryBytecodeHex());
    const addr = predictLinkedNftTokenRegistryCreate2Address(SENDER, code);
    expect(validateHex32(addr)).toMatch(/^0x[0-9a-f]{64}$/i);
  });

  it('builds claim×2 + register flow txs', () => {
    const txs = buildLinkedNftTokenRegisterFlowTxs({
      senderHex32: SENDER,
      registryHex32: REG,
      collectionHex32: A,
      tokenHex32: B,
    });
    expect(txs).toHaveLength(3);
    expect(txs.every((t) => t.type === 'contract_call')).toBe(true);
    expect(txs[0]!.contract).toBe(REG);
    expect(txs[0]!.calldata.length).toBe(2 + 64 * 2); // 0x + 64 bytes
    expect(txs[2]!.calldata.length).toBe(2 + 96 * 2);
  });

  it('decodes links_count and get_link_at return data', () => {
    const countWord = '0x' + '00'.repeat(31) + '03';
    expect(decodeLinkedNftTokenLinksCountReturnData(countWord)).toBe(3n);

    const pair = A.slice(2) + B.slice(2);
    const decoded = decodeLinkedNftTokenGetLinkAtReturnData(`0x${pair}`);
    expect(decoded.collectionHex.toLowerCase()).toBe(A);
    expect(decoded.tokenHex.toLowerCase()).toBe(B);
    expect(decoded.active).toBe(true);

    const tomb = decodeLinkedNftTokenGetLinkAtReturnData('0x' + '0'.repeat(128));
    expect(tomb.active).toBe(false);
  });

  it('exposes stable topic0 and count key hex', () => {
    expect(LINKED_NFT_TOKEN_TOPIC_REGISTER_HEX).toMatch(/^0x[0-9a-f]{64}$/i);
    expect(LINKED_NFT_TOKEN_REGISTRY_COUNT_KEY_HEX).toMatch(/424f494e474c4e4b/i);
  });
});

describe('linkedNftTokenRegistry v2 (creator-only claims)', () => {
  it('canonical testnet registry is v2; v1 kept for history', async () => {
    const m = await import('../src/linkedNftTokenRegistry.js');
    expect(m.CANONICAL_BOING_TESTNET_LINKED_NFT_TOKEN_REGISTRY_V1_HEX).toBe(
      '0xebf9f0190f415852f90d0e60343126201248ab96273fdbf8acc5fe5fa03c3dd8',
    );
    expect(m.CANONICAL_BOING_TESTNET_LINKED_NFT_TOKEN_REGISTRY_HEX).not.toBe(
      m.CANONICAL_BOING_TESTNET_LINKED_NFT_TOKEN_REGISTRY_V1_HEX,
    );
  });

  it('encodes claim_self / get_asset_deployer and the storage keys', async () => {
    const m = await import('../src/linkedNftTokenRegistry.js');
    const cs = m.encodeLinkedNftTokenClaimSelfCalldata(A);
    expect(cs.length).toBe(64);
    expect(cs[31]).toBe(0xe7);
    expect(m.encodeLinkedNftTokenGetAssetDeployerCalldata(A)[31]).toBe(0xe8);
    const ascii = (t: string) => {
      const u = new Uint8Array(32);
      u.set(new TextEncoder().encode(t));
      return u;
    };
    const a = hexToBytes(A);
    const xor = (mask: Uint8Array) => `0x${Array.from(a, (b, i) => (b ^ mask[i]!).toString(16).padStart(2, '0')).join('')}`;
    expect(m.linkedNftTokenClaimerStorageKeyHex(A)).toBe(xor(ascii('BOING_LNK_CLAIMER_V1')));
    expect(m.linkedNftTokenDeployerStorageKeyHex(A)).toBe(xor(ascii('BOING_LNK_DEPLOYER_V2')));
    expect(m.LINKED_NFT_TOKEN_REGISTRY_CREATE2_SALT_V2_HEX.startsWith('0x424f494e475f4e46545f544f4b454e5f4c494e4b5f5245475f5632')).toBe(true);
  });

  it('claim_self init snippet: selector word, CALLER, CALL registry with 64 bytes of args', async () => {
    const m = await import('../src/linkedNftTokenRegistry.js');
    const h = m.buildLinkedNftTokenClaimSelfInitSnippetHex(REG).slice(2);
    const w = (n: number) => n.toString(16).padStart(64, '0');
    const expected =
      `7f${w(0xe7)}7f${w(0)}52` + // MSTORE(0, selector)
      `337f${w(32)}52` + // MSTORE(32, CALLER)
      `7f${REG.slice(2)}7f${w(0)}7f${w(64)}7f${w(0)}7f${w(0)}f1`; // CALL(registry, 0, 64, 0, 0)
    expect(h).toBe(expected);
  });

  it('register flow without claims, v2 deploy tx uses the v2 salt and runtime', async () => {
    const m = await import('../src/linkedNftTokenRegistry.js');
    const txs = m.buildLinkedNftTokenRegisterFlowTxs({
      senderHex32: SENDER,
      registryHex32: REG,
      collectionHex32: A,
      tokenHex32: B,
      includeClaims: false,
    });
    expect(txs).toHaveLength(1);
    expect(txs[0]!.calldata.slice(2, 66)).toBe(`${'00'.repeat(31)}e1`);
    const d = m.buildLinkedNftTokenRegistryV2DeployMetaTx();
    expect(d.create2_salt).toBe(m.LINKED_NFT_TOKEN_REGISTRY_CREATE2_SALT_V2_HEX);
    expect(d.bytecode).toBe(m.resolveLinkedNftTokenRegistryV2BytecodeHex());
  });
});
