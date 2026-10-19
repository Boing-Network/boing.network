/**
 * Thin **project pack** helper: joint NFT + fungible CREATE2 deploys, then registry
 * claim×2 + `register_link`. Composes existing helpers — no new contracts.
 *
 * See `docs/BOING-LINKED-NFT-TOKEN.md`.
 */
import { type BuildLinkedNftTokenPairDeploysInput, type LinkedNftTokenPairDeploys } from './linkedNftToken.js';
import { buildLinkedNftTokenRegisterFlowTxs } from './linkedNftTokenRegistry.js';
/** Recommended Express submit order for a project pack (deploy pair, then claim×2, then register). */
export declare const LINKED_NFT_TOKEN_PROJECT_PACK_SUBMIT_ORDER: readonly ["collectionDeploy", "tokenDeploy", "claimCollection", "claimToken", "registerLink"];
/**
 * Submit order when both deploys claim themselves in the v2 registry from their init code
 * (`claimAtDeploy: true`): no separate claim transactions.
 */
export declare const LINKED_NFT_TOKEN_PROJECT_PACK_SUBMIT_ORDER_CLAIM_AT_DEPLOY: readonly ["collectionDeploy", "tokenDeploy", "registerLink"];
export type LinkedNftTokenProjectPackSubmitStep = (typeof LINKED_NFT_TOKEN_PROJECT_PACK_SUBMIT_ORDER)[number];
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
    /**
     * Both bytecode overrides already claim themselves in the v2 registry from their init code
     * ({@link buildLinkedNftTokenClaimSelfInitSnippetHex}); only `register_link` is left to send.
     */
    claimAtDeploy?: boolean;
};
export type LinkedNftTokenProjectPack = LinkedNftTokenPairDeploys & {
    registryHex32: `0x${string}`;
    senderHex32: `0x${string}`;
    /** claim(collection), claim(token), register_link — same as {@link buildLinkedNftTokenRegisterFlowTxs}. */
    registerFlowTxs: ReturnType<typeof buildLinkedNftTokenRegisterFlowTxs>;
    /** Stable hint for wizard progress UI. */
    submitOrder: typeof LINKED_NFT_TOKEN_PROJECT_PACK_SUBMIT_ORDER | typeof LINKED_NFT_TOKEN_PROJECT_PACK_SUBMIT_ORDER_CLAIM_AT_DEPLOY;
};
/**
 * Build a full **project pack**: two `contract_deploy_meta` txs (NFT + token) plus three
 * registry `contract_call` txs (claim×2 + register_link) against predicted CREATE2 addresses.
 *
 * Thin composition of {@link buildLinkedNftTokenPairDeploys} and
 * {@link buildLinkedNftTokenRegisterFlowTxs}. Submit deploys first (either order), wait for
 * inclusion, then submit register-flow txs in order. On the v2 registry pass bytecode overrides whose
 * init code claims the asset (and `claimAtDeploy: true`): nobody else can claim your AccountIds.
 */
export declare function buildLinkedNftTokenProjectPack(input: BuildLinkedNftTokenProjectPackInput): LinkedNftTokenProjectPack;
//# sourceMappingURL=linkedNftTokenProjectPack.d.ts.map