import { describe, expect, it } from 'vitest';
import {
  buildContractDeployMetaTx,
  buildReferenceFungibleDeployMetaTx,
  buildReferenceNftCollectionDeployMetaTx,
  ensure0xHex,
  REFERENCE_FUNGIBLE_TEMPLATE_VERSION,
  REFERENCE_NFT_COLLECTION_TEMPLATE_VERSION,
  resolveReferenceFungibleTemplateBytecodeHex,
  resolveReferenceNftCollectionTemplateBytecodeHex,
} from '../src/canonicalDeployArtifacts.js';

describe('canonicalDeployArtifacts', () => {
  it('ensure0xHex adds 0x', () => {
    expect(ensure0xHex('abc')).toBe('0xabc');
    expect(ensure0xHex('0xDEAD')).toBe('0xDEAD');
  });

  it('resolveReferenceFungibleTemplateBytecodeHex uses explicitHex', () => {
    expect(resolveReferenceFungibleTemplateBytecodeHex({ explicitHex: 'aa' })).toBe('0xaa');
    expect(resolveReferenceFungibleTemplateBytecodeHex({ explicitHex: '0xbb' })).toBe('0xbb');
  });

  it('buildContractDeployMetaTx builds wallet object', () => {
    const tx = buildContractDeployMetaTx({
      bytecodeHex: '0x6001',
      assetName: 'My Token',
      assetSymbol: 'mtk',
      purposeCategory: 'token',
      descriptionHashHex: '0x' + 'ab'.repeat(32),
    });
    expect(tx.type).toBe('contract_deploy_meta');
    expect(tx.bytecode).toBe('0x6001');
    expect(tx.asset_name).toBe('My Token');
    expect(tx.asset_symbol).toBe('MTK');
    expect(tx.purpose_category).toBe('token');
    expect(tx.description_hash).toBe('0x' + 'ab'.repeat(32));
  });

  it('REFERENCE_FUNGIBLE_TEMPLATE_VERSION matches pinned default bytecode', () => {
    expect(REFERENCE_FUNGIBLE_TEMPLATE_VERSION).toBe('1');
  });

  it('resolveReferenceFungibleTemplateBytecodeHex returns embedded default without env', () => {
    const h = resolveReferenceFungibleTemplateBytecodeHex();
    expect(h.startsWith('0x')).toBe(true);
    expect(h.length).toBeGreaterThan(200);
  });

  it('resolveReferenceNftCollectionTemplateBytecodeHex uses explicitHex', () => {
    expect(resolveReferenceNftCollectionTemplateBytecodeHex({ explicitHex: '0xab' })).toBe('0xab');
  });

  it('REFERENCE_NFT_COLLECTION_TEMPLATE_VERSION is 1', () => {
    expect(REFERENCE_NFT_COLLECTION_TEMPLATE_VERSION).toBe('1');
  });

  it('buildReferenceFungibleDeployMetaTx matches resolve + buildContractDeployMetaTx', () => {
    const a = buildReferenceFungibleDeployMetaTx({ assetName: 'T', assetSymbol: 't' });
    const b = buildContractDeployMetaTx({
      bytecodeHex: resolveReferenceFungibleTemplateBytecodeHex(),
      assetName: 'T',
      assetSymbol: 't',
    });
    expect(a).toEqual(b);
  });

  it('buildReferenceNftCollectionDeployMetaTx throws without bytecode', () => {
    expect(() =>
      buildReferenceNftCollectionDeployMetaTx({ collectionName: 'C', collectionSymbol: 'c' }),
    ).toThrow(/no collection bytecode/);
  });

  it('buildReferenceNftCollectionDeployMetaTx works with bytecodeHexOverride', () => {
    const tx = buildReferenceNftCollectionDeployMetaTx({
      collectionName: 'My Coll',
      collectionSymbol: 'c',
      bytecodeHexOverride: '0x6001',
    });
    expect(tx.type).toBe('contract_deploy_meta');
    expect(tx.bytecode).toBe('0x6001');
    expect(tx.asset_name).toBe('My Coll');
    expect(tx.asset_symbol).toBe('C');
    expect(tx.purpose_category).toBe('nft');
  });
});
