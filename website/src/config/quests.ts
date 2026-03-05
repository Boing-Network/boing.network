/**
 * Community quest definitions for the incentivized testnet.
 * Used by /network/quests page. See docs/TESTNET.md Part 2 §2.7 (Community Quests).
 */

export type VerificationType = 'on_chain' | 'manual';
export type RewardTier = 'base' | 'validator' | 'bonus';

export interface Quest {
  id: string;
  name: string;
  description: string;
  howToComplete: string;
  verificationType: VerificationType;
  rewardTier: RewardTier;
}

export const QUESTS: Quest[] = [
  {
    id: 'faucet',
    name: 'First drip',
    description: 'Request testnet BOING from the faucet.',
    howToComplete: 'Go to the Faucet page, enter your account ID (32-byte hex), and request 1,000 testnet BOING.',
    verificationType: 'on_chain',
    rewardTier: 'base',
  },
  {
    id: 'first_tx',
    name: 'First transaction',
    description: 'Send any transaction on testnet.',
    howToComplete: 'Use your wallet or the RPC to send a transaction from your account. We verify via on-chain data.',
    verificationType: 'on_chain',
    rewardTier: 'base',
  },
  {
    id: 'validator_connect',
    name: 'Join the network',
    description: 'Run a node connected to testnet bootnodes.',
    howToComplete: 'Run boing-node with --p2p_listen and --bootnodes, or use VibeMiner. Submit your node ID or a screenshot.',
    verificationType: 'manual',
    rewardTier: 'validator',
  },
  {
    id: 'feedback',
    name: 'Share feedback',
    description: 'Answer a few short questions about UX, docs, or bugs.',
    howToComplete: 'Submit the form below (or the linked form) with your account ID and your feedback or link to feedback.',
    verificationType: 'manual',
    rewardTier: 'bonus',
  },
  {
    id: 'social',
    name: 'Join community',
    description: 'Join Discord and post in #testnet-intros.',
    howToComplete: 'Join our Discord, post in #testnet-intros with your testnet account ID. Submit your Discord handle below.',
    verificationType: 'manual',
    rewardTier: 'bonus',
  },
];

/** Optional: URL for external form if API is not ready (Phase 0). Set to empty to use in-page form only. */
export const EXTERNAL_QUEST_FORM_URL = '';
