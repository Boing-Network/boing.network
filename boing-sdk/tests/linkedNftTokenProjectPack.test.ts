import { describe, expect, it } from 'vitest';
import { predictCreate2ContractAddress } from '../src/create2.js';
import { hexToBytes, validateHex32 } from '../src/hex.js';
import {
  CANONICAL_BOING_TESTNET_LINKED_NFT_TOKEN_REGISTRY_HEX,
  SELECTOR_LINKED_NFT_TOKEN_CLAIM_ASSET,
  SELECTOR_LINKED_NFT_TOKEN_REGISTER_LINK,
  encodeLinkedNftTokenClaimAssetCalldataHex,
  encodeLinkedNftTokenRegisterLinkCalldataHex,
} from '../src/linkedNftTokenRegistry.js';
import {
  LINKED_NFT_TOKEN_PROJECT_PACK_SUBMIT_ORDER,
  buildLinkedNftTokenProjectPack,
} from '../src/linkedNftTokenProjectPack.js';

const DEPLOYER = `0x${'11'.repeat(32)}`;
const REG = `0x${'cc'.repeat(32)}`;
const COLLECTION_SALT = `0x${'55'.repeat(32)}`;

describe('buildLinkedNftTokenProjectPack', () => {
  it('composes pair deploys + claim×2 + register against predicted addresses', () => {
    const pack = buildLinkedNftTokenProjectPack({
      deployerHex: DEPLOYER,
      collectionName: 'Pack NFT',
      collectionSymbol: 'PNFT',
      tokenName: 'Pack Token',
      tokenSymbol: 'PTKN',
      collectionSaltHex: COLLECTION_SALT,
      registryHex32: REG,
    });

    expect(pack.collectionSaltHex).toBe(COLLECTION_SALT);
    expect(pack.predictedCollectionAddress).toMatch(/^0x[0-9a-f]{64}$/);
    expect(pack.predictedTokenAddress).toMatch(/^0x[0-9a-f]{64}$/);
    expect(pack.collectionDeployTx.type).toBe('contract_deploy_meta');
    expect(pack.tokenDeployTx.type).toBe('contract_deploy_meta');
    expect(pack.collectionDeployTx.create2_salt).toBe(pack.collectionSaltHex);
    expect(pack.tokenDeployTx.create2_salt).toBe(pack.tokenSaltHex);

    const predictedNft = validateHex32(
      predictCreate2ContractAddress(
        DEPLOYER,
        hexToBytes(pack.collectionSaltHex),
        hexToBytes(pack.collectionDeployTx.bytecode),
      ),
    ).toLowerCase();
    const predictedTok = validateHex32(
      predictCreate2ContractAddress(
        DEPLOYER,
        hexToBytes(pack.tokenSaltHex),
        hexToBytes(pack.tokenDeployTx.bytecode),
      ),
    ).toLowerCase();
    expect(pack.predictedCollectionAddress).toBe(predictedNft);
    expect(pack.predictedTokenAddress).toBe(predictedTok);

    expect(pack.registryHex32).toBe(REG);
    expect(pack.senderHex32).toBe(DEPLOYER.toLowerCase());
    expect(pack.registerFlowTxs).toHaveLength(3);
    expect(pack.registerFlowTxs.every((t) => t.type === 'contract_call')).toBe(true);
    expect(pack.registerFlowTxs.every((t) => t.contract === REG)).toBe(true);

    const claimCol = encodeLinkedNftTokenClaimAssetCalldataHex(pack.predictedCollectionAddress);
    const claimTok = encodeLinkedNftTokenClaimAssetCalldataHex(pack.predictedTokenAddress);
    const register = encodeLinkedNftTokenRegisterLinkCalldataHex(
      pack.predictedCollectionAddress,
      pack.predictedTokenAddress,
    );
    expect(pack.registerFlowTxs[0]!.calldata).toBe(claimCol);
    expect(pack.registerFlowTxs[1]!.calldata).toBe(claimTok);
    expect(pack.registerFlowTxs[2]!.calldata).toBe(register);

    const claimBytes = hexToBytes(pack.registerFlowTxs[0]!.calldata);
    expect(claimBytes[31]).toBe(SELECTOR_LINKED_NFT_TOKEN_CLAIM_ASSET);
    const regBytes = hexToBytes(pack.registerFlowTxs[2]!.calldata);
    expect(regBytes[31]).toBe(SELECTOR_LINKED_NFT_TOKEN_REGISTER_LINK);

    expect(pack.submitOrder).toEqual([...LINKED_NFT_TOKEN_PROJECT_PACK_SUBMIT_ORDER]);
  });

  it('defaults registry to canonical public testnet when omitted', () => {
    const pack = buildLinkedNftTokenProjectPack({
      deployerHex: DEPLOYER,
      collectionName: 'A',
      collectionSymbol: 'A',
      tokenName: 'B',
      tokenSymbol: 'B',
      collectionSaltHex: COLLECTION_SALT,
    });
    expect(pack.registryHex32).toBe(CANONICAL_BOING_TESTNET_LINKED_NFT_TOKEN_REGISTRY_HEX);
  });

  it('allows a distinct sender for claim/register', () => {
    const sender = `0x${'dd'.repeat(32)}`;
    const pack = buildLinkedNftTokenProjectPack({
      deployerHex: DEPLOYER,
      senderHex32: sender,
      collectionName: 'A',
      collectionSymbol: 'A',
      tokenName: 'B',
      tokenSymbol: 'B',
      collectionSaltHex: COLLECTION_SALT,
      registryHex32: REG,
    });
    expect(pack.senderHex32).toBe(sender);
    expect(pack.registerFlowTxs[0]!.access_list.read).toContain(sender);
  });
});
