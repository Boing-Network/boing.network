import { describe, expect, it } from 'vitest';
import { predictCreate2ContractAddress } from '../src/create2.js';
import { hexToBytes, validateHex32 } from '../src/hex.js';
import {
  LINKED_NFT_TOKEN_SCHEMA,
  COMPANION_TOKENS_OFFCHAIN_KEY,
  COMPANION_COLLECTIONS_OFFCHAIN_KEY,
  LINKED_NFT_TOKEN_OFFCHAIN_KEY,
  attachLinkedNftTokenPeer,
  applyLinkedNftTokenOffchainKeys,
  buildLinkedNftTokenPairDeploys,
  decodeLinkedNftTokenJson,
  deriveLinkedNftTokenFungibleSaltHex,
  descriptionHashHexFromLinkedNftToken,
  encodeLinkedNftTokenJson,
  linkedNftTokenPairSalts,
  normalizeLinkedNftToken,
  readLinkedNftTokenFromOffchainMetadata,
  softGateLinkedNftTokenPeers,
  unlinkLinkedNftTokenPeer,
  updateLinkedNftTokenPeers,
} from '../src/linkedNftToken.js';

const DEPLOYER = `0x${'11'.repeat(32)}`;
const PEER_A = `0x${'22'.repeat(32)}`;
const PEER_B = `0x${'33'.repeat(32)}`;
const OTHER = `0x${'44'.repeat(32)}`;

describe('linkedNftToken schema', () => {
  it('normalizes peers: lowercase, dedupe, sort', () => {
    const n = normalizeLinkedNftToken({
      role: 'nft_collection',
      peers: [PEER_B, PEER_A, PEER_A.toUpperCase()],
      self: DEPLOYER,
      attester: DEPLOYER,
    });
    expect(n.schema).toBe(LINKED_NFT_TOKEN_SCHEMA);
    expect(n.peers).toEqual([PEER_A, PEER_B]);
    expect(n.self).toBe(DEPLOYER);
  });

  it('encode/decode round-trip and stable description_hash', () => {
    const input = {
      role: 'fungible_token' as const,
      peers: [PEER_B, PEER_A],
      self: DEPLOYER,
      attester: DEPLOYER,
      revision: 2,
      note: 'demo',
    };
    const json = encodeLinkedNftTokenJson(input);
    const decoded = decodeLinkedNftTokenJson(json);
    expect(decoded).toEqual(normalizeLinkedNftToken(input));
    const h1 = descriptionHashHexFromLinkedNftToken(input);
    const h2 = descriptionHashHexFromLinkedNftToken({
      ...input,
      peers: [PEER_A, PEER_B],
    });
    expect(h1).toMatch(/^0x[0-9a-f]{64}$/);
    expect(h2).toBe(h1);
  });

  it('rejects wrong schema on decode', () => {
    expect(() =>
      decodeLinkedNftTokenJson(JSON.stringify({ schema: 'other.v1', role: 'nft_collection' })),
    ).toThrow(/expected schema/);
  });
});

describe('linkedNftToken peer mutation', () => {
  it('attach / update / unlink with revision bumps', () => {
    let doc = normalizeLinkedNftToken({ role: 'nft_collection', peers: [PEER_A] });
    expect(doc.revision).toBe(0);
    doc = attachLinkedNftTokenPeer(doc, PEER_B);
    expect(doc.peers).toEqual([PEER_A, PEER_B]);
    expect(doc.revision).toBe(1);
    doc = updateLinkedNftTokenPeers(doc, [PEER_B]);
    expect(doc.peers).toEqual([PEER_B]);
    expect(doc.revision).toBe(2);
    doc = unlinkLinkedNftTokenPeer(doc, PEER_B);
    expect(doc.peers).toEqual([]);
    expect(doc.revision).toBe(3);
  });

  it('off-chain keys + read back', () => {
    const doc = normalizeLinkedNftToken({
      role: 'nft_collection',
      peers: [PEER_A, PEER_B],
      attester: DEPLOYER,
    });
    const meta = applyLinkedNftTokenOffchainKeys({ name: 'C' }, doc);
    expect(meta[LINKED_NFT_TOKEN_OFFCHAIN_KEY]).toEqual(doc);
    expect(meta[COMPANION_TOKENS_OFFCHAIN_KEY]).toEqual([PEER_A, PEER_B]);
    expect(readLinkedNftTokenFromOffchainMetadata(meta)?.peers).toEqual([PEER_A, PEER_B]);

    const tokenMeta = applyLinkedNftTokenOffchainKeys(
      {},
      normalizeLinkedNftToken({ role: 'fungible_token', peers: [PEER_A] }),
    );
    expect(tokenMeta[COMPANION_COLLECTIONS_OFFCHAIN_KEY]).toEqual([PEER_A]);
  });
});

describe('linkedNftToken salts + soft-gate', () => {
  it('derives fungible salt deterministically from collection salt', () => {
    const collectionSalt = `0x${'ab'.repeat(32)}`;
    const a = deriveLinkedNftTokenFungibleSaltHex(collectionSalt);
    const b = deriveLinkedNftTokenFungibleSaltHex(collectionSalt);
    expect(a).toBe(b);
    expect(a).not.toBe(collectionSalt);
    const pair = linkedNftTokenPairSalts(collectionSalt);
    expect(pair.collectionSaltHex).toBe(validateHex32(collectionSalt).toLowerCase());
    expect(pair.tokenSaltHex).toBe(a);
  });

  it('soft-gate prefers matching deployers; flags spoof risk on mismatch', () => {
    const ok = softGateLinkedNftTokenPeers({
      subjectDeployerHex: DEPLOYER,
      peerDeployerHex: DEPLOYER,
      attesterHex: DEPLOYER,
    });
    expect(ok.ok).toBe(true);
    expect(ok.spoofRisk).toBe(false);
    expect(ok.matched).toBe(true);

    const soft = softGateLinkedNftTokenPeers({
      subjectDeployerHex: DEPLOYER,
      peerDeployerHex: OTHER,
    });
    expect(soft.ok).toBe(true);
    expect(soft.spoofRisk).toBe(true);

    const hard = softGateLinkedNftTokenPeers({
      subjectDeployerHex: DEPLOYER,
      peerDeployerHex: OTHER,
      requireMatch: true,
    });
    expect(hard.ok).toBe(false);
    expect(hard.spoofRisk).toBe(true);
  });
});

describe('buildLinkedNftTokenPairDeploys', () => {
  it('builds two CREATE2 deploys with mutual description_hash and predictable addresses', () => {
    const collectionSalt = `0x${'cd'.repeat(32)}`;
    const pair = buildLinkedNftTokenPairDeploys({
      deployerHex: DEPLOYER,
      collectionName: 'Art',
      collectionSymbol: 'art',
      tokenName: 'Art Coin',
      tokenSymbol: 'artc',
      collectionSaltHex: collectionSalt,
      note: 'pair',
    });

    expect(pair.collectionSaltHex).toBe(validateHex32(collectionSalt).toLowerCase());
    expect(pair.tokenSaltHex).toBe(deriveLinkedNftTokenFungibleSaltHex(collectionSalt));
    expect(pair.collectionDeployTx.type).toBe('contract_deploy_meta');
    expect(pair.tokenDeployTx.type).toBe('contract_deploy_meta');
    expect(pair.collectionDeployTx.purpose_category).toBe('nft');
    expect(pair.tokenDeployTx.purpose_category).toBe('token');
    expect(pair.collectionDeployTx.create2_salt).toBe(pair.collectionSaltHex);
    expect(pair.tokenDeployTx.create2_salt).toBe(pair.tokenSaltHex);
    expect(pair.collectionDeployTx.description_hash).toBe(
      descriptionHashHexFromLinkedNftToken(pair.collectionLink),
    );
    expect(pair.tokenDeployTx.description_hash).toBe(
      descriptionHashHexFromLinkedNftToken(pair.tokenLink),
    );
    expect(pair.collectionLink.peers).toEqual([pair.predictedTokenAddress]);
    expect(pair.tokenLink.peers).toEqual([pair.predictedCollectionAddress]);
    expect(pair.collectionLink.attester).toBe(DEPLOYER);

    const predColl = validateHex32(
      predictCreate2ContractAddress(
        DEPLOYER,
        hexToBytes(pair.collectionSaltHex),
        hexToBytes(pair.collectionDeployTx.bytecode),
      ),
    ).toLowerCase();
    const predTok = validateHex32(
      predictCreate2ContractAddress(
        DEPLOYER,
        hexToBytes(pair.tokenSaltHex),
        hexToBytes(pair.tokenDeployTx.bytecode),
      ),
    ).toLowerCase();
    expect(pair.predictedCollectionAddress).toBe(predColl);
    expect(pair.predictedTokenAddress).toBe(predTok);
  });

  it('many-to-many: attach second peer after joint deploy doc', () => {
    const pair = buildLinkedNftTokenPairDeploys({
      deployerHex: DEPLOYER,
      collectionName: 'C',
      collectionSymbol: 'c',
      tokenName: 'T',
      tokenSymbol: 't',
      collectionSaltHex: `0x${'ee'.repeat(32)}`,
    });
    const expanded = attachLinkedNftTokenPeer(pair.collectionLink, PEER_B);
    expect(expanded.peers).toContain(pair.predictedTokenAddress);
    expect(expanded.peers).toContain(PEER_B);
    expect(expanded.peers.length).toBe(2);
  });
});
