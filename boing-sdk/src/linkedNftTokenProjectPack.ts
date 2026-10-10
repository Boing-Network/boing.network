/**
 * Thin **project pack** helper: joint NFT + fungible CREATE2 deploys, then registry
 * claim×2 + `register_link`. Composes existing helpers — no new contracts.
 *
 * See `docs/BOING-LINKED-NFT-TOKEN.md`.
 */

import { validateHex32 } from './hex.js';
import {
  buildLinkedNftTokenPairDeploys,
  type BuildLinkedNftTokenPairDeploysInput,
  type LinkedNftTokenPairDeploys,
} from './linkedNftToken.js';
import {
  buildLinkedNftTokenRegisterFlowTxs,
  CANONICAL_BOING_TESTNET_LINKED_NFT_TOKEN_REGISTRY_HEX,
} from './linkedNftTokenRegistry.js';

/** Recommended Express submit order for a project pack (deploy pair, then claim×2, then register). */
export const LINKED_NFT_TOKEN_PROJECT_PACK_SUBMIT_ORDER = [
  'collectionDeploy',
  'tokenDeploy',
  'claimCollection',
  'claimToken',
  'registerLink',
] as const;

export type LinkedNftTokenProjectPackSubmitStep =
  (typeof LINKED_NFT_TOKEN_PROJECT_PACK_SUBMIT_ORDER)[number];

export type BuildLinkedNftTokenProjectPackInput = BuildLinkedNftTokenPairDeploysInput & {
  /**
   * Linked NFT↔token registry AccountId.
   * Defaults to {@link CANONICAL_BOING_TESTNET_LINKED_NFT_TOKEN_REGISTRY_HEX} (public testnet).
   */
  registryHex32?: string;
  /**
   * Account that will claim both assets and register the link.
   * Defaults to {@link BuildLinkedNftTokenPairDeploysInput.deployerHex} (joint deployer).
   */
  senderHex32?: string;
};

export type LinkedNftTokenProjectPack = LinkedNftTokenPairDeploys & {
  registryHex32: `0x${string}`;
  senderHex32: `0x${string}`;
  /** claim(collection), claim(token), register_link — same as {@link buildLinkedNftTokenRegisterFlowTxs}. */
  registerFlowTxs: ReturnType<typeof buildLinkedNftTokenRegisterFlowTxs>;
  /** Stable hint for wizard progress UI. */
  submitOrder: typeof LINKED_NFT_TOKEN_PROJECT_PACK_SUBMIT_ORDER;
};

/**
 * Build a full **project pack**: two `contract_deploy_meta` txs (NFT + token) plus three
 * registry `contract_call` txs (claim×2 + register_link) against predicted CREATE2 addresses.
 *
 * Thin composition of {@link buildLinkedNftTokenPairDeploys} and
 * {@link buildLinkedNftTokenRegisterFlowTxs}. Submit deploys first (either order), wait for
 * inclusion, then submit register-flow txs in order — claim soon after deploy so a third party
 * cannot front-run `claim_asset` on your AccountIds.
 */
export function buildLinkedNftTokenProjectPack(
  input: BuildLinkedNftTokenProjectPackInput,
): LinkedNftTokenProjectPack {
  const pair = buildLinkedNftTokenPairDeploys(input);
  const registryHex32 = validateHex32(
    input.registryHex32?.trim()
      ? input.registryHex32
      : CANONICAL_BOING_TESTNET_LINKED_NFT_TOKEN_REGISTRY_HEX,
  ).toLowerCase() as `0x${string}`;
  const senderHex32 = validateHex32(input.senderHex32?.trim() ? input.senderHex32 : input.deployerHex)
    .toLowerCase() as `0x${string}`;

  const registerFlowTxs = buildLinkedNftTokenRegisterFlowTxs({
    senderHex32,
    registryHex32,
    collectionHex32: pair.predictedCollectionAddress,
    tokenHex32: pair.predictedTokenAddress,
  });

  return {
    ...pair,
    registryHex32,
    senderHex32,
    registerFlowTxs,
    submitOrder: LINKED_NFT_TOKEN_PROJECT_PACK_SUBMIT_ORDER,
  };
}
