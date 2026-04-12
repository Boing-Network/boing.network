import { describe, expect, it } from 'vitest';
import {
  buildContractDeployMetaTx,
  buildReferenceFungibleDeployMetaTx,
  buildReferenceFungibleSecuredDeployMetaTx,
  buildReferenceNftCollectionDeployMetaTx,
  ensure0xHex,
  REFERENCE_FUNGIBLE_SECURED_TEMPLATE_VERSION,
  REFERENCE_FUNGIBLE_TEMPLATE_VERSION,
  REFERENCE_NFT_COLLECTION_TEMPLATE_VERSION,
  resolveReferenceFungibleSecuredTemplateBytecodeHex,
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

  it('resolveReferenceFungibleSecuredTemplateBytecodeHex uses explicitHex', () => {
    expect(resolveReferenceFungibleSecuredTemplateBytecodeHex({ explicitHex: 'cc' })).toBe('0xcc');
  });

  it('REFERENCE_FUNGIBLE_SECURED_TEMPLATE_VERSION matches pinned default', () => {
    expect(REFERENCE_FUNGIBLE_SECURED_TEMPLATE_VERSION).toBe('1');
  });

  it('resolveReferenceFungibleSecuredTemplateBytecodeHex returns embedded default (starts with 0xfd)', () => {
    const h = resolveReferenceFungibleSecuredTemplateBytecodeHex();
    expect(h.startsWith('0xfd')).toBe(true);
    expect(h.length).toBeGreaterThan(resolveReferenceFungibleTemplateBytecodeHex().length);
  });

  it('buildReferenceFungibleSecuredDeployMetaTx matches resolve + buildContractDeployMetaTx', () => {
    const a = buildReferenceFungibleSecuredDeployMetaTx({ assetName: 'S', assetSymbol: 's' });
    const b = buildContractDeployMetaTx({
      bytecodeHex: resolveReferenceFungibleSecuredTemplateBytecodeHex(),
      assetName: 'S',
      assetSymbol: 's',
    });
    expect(a).toEqual(b);
  });

  it('resolveReferenceNftCollectionTemplateBytecodeHex uses explicitHex', () => {
    expect(resolveReferenceNftCollectionTemplateBytecodeHex({ explicitHex: '0xab' })).toBe('0xab');
  });

  it('resolveReferenceNftCollectionTemplateBytecodeHex returns embedded default without env', () => {
    const h = resolveReferenceNftCollectionTemplateBytecodeHex();
    expect(h.startsWith('0x')).toBe(true);
    expect(h.length).toBeGreaterThan(200);
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

  it('buildReferenceFungibleDeployMetaTx commits nativeTokenSecurity to description_hash', () => {
    const tx = buildReferenceFungibleDeployMetaTx({
      assetName: 'T',
      assetSymbol: 't',
      nativeTokenSecurity: { antiBot: true, enableBlacklist: true },
    });
    expect(tx.description_hash).toMatch(/^0x[0-9a-f]{64}$/i);
    const tx2 = buildReferenceFungibleDeployMetaTx({
      assetName: 'T',
      assetSymbol: 't',
      nativeTokenSecurity: { antiBot: true, enableBlacklist: true },
    });
    expect(tx2.description_hash).toBe(tx.description_hash);
  });

  it('buildReferenceFungibleDeployMetaTx prefers explicit descriptionHashHex over nativeTokenSecurity', () => {
    const explicit = `0x${'11'.repeat(32)}`;
    const tx = buildReferenceFungibleDeployMetaTx({
      assetName: 'T',
      assetSymbol: 't',
      descriptionHashHex: explicit,
      nativeTokenSecurity: { antiBot: true },
    });
    expect(tx.description_hash).toBe(explicit);
  });

  it('buildReferenceNftCollectionDeployMetaTx uses embedded default bytecode without override', () => {
    const tx = buildReferenceNftCollectionDeployMetaTx({ collectionName: 'C', collectionSymbol: 'c' });
    expect(tx.type).toBe('contract_deploy_meta');
    expect(tx.bytecode.startsWith('0x')).toBe(true);
    expect(tx.bytecode.length).toBeGreaterThan(200);
    expect(tx.asset_name).toBe('C');
    expect(tx.asset_symbol).toBe('C');
    expect(tx.purpose_category).toBe('nft');
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
