/**
 * Community quest definitions for the testnet portal.
 * No rewards are offered while the incentivized testnet is not running.
 * Completing quests does not create a mainnet BOING claim.
 * Used by testnet portal user hub (dashboard, community, quests). See docs/TESTNET.md Part 2 §2.7.
 */

export type VerificationType = 'on_chain' | 'manual';
export type RewardTier = 'base' | 'validator' | 'bonus' | 'ecosystem';

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
    howToComplete: 'Go to the Faucet page, paste your Boing address (or connect Boing Express), and request testnet BOING.',
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
    id: 'express_wallet',
    name: 'Open Express',
    description: 'Create or unlock a Boing Express wallet and copy your testnet address.',
    howToComplete: 'Install or open https://boing.express, create/unlock a wallet on chain 6913, then submit your address on the Quests form.',
    verificationType: 'manual',
    rewardTier: 'ecosystem',
  },
  {
    id: 'finance_swap',
    name: 'Try Finance',
    description: 'Connect Express to Boing Finance and complete a small testnet swap (or open the Swap tab if liquidity is thin).',
    howToComplete: 'Visit https://boing.finance, connect Express, attempt a native swap or note why it could not complete. Submit a short note + tx hash if any.',
    verificationType: 'manual',
    rewardTier: 'ecosystem',
  },
  {
    id: 'observer_lookup',
    name: 'Look up on Observer',
    description: 'Find your account or a recent block on Boing Observer.',
    howToComplete: 'Open https://boing.observer, search your address or a block height, and submit the explorer URL you used.',
    verificationType: 'manual',
    rewardTier: 'ecosystem',
  },
  {
    id: 'validator_connect',
    name: 'Join the network',
    description: 'Run a node connected to testnet bootnodes.',
    howToComplete: 'Run boing-node with --p2p-listen and --bootnodes (see the validator guide). Submit your node ID or a screenshot. Note: public day-to-day traffic still rides a single hosted validator path.',
    verificationType: 'manual',
    rewardTier: 'validator',
  },
  {
    id: 'feedback',
    name: 'Share feedback',
    description: 'Answer a few short questions about UX, docs, or bugs.',
    howToComplete: 'Submit the form below (or the linked form) with your account address and your feedback or link to feedback.',
    verificationType: 'manual',
    rewardTier: 'bonus',
  },
  {
    id: 'social',
    name: 'Join community',
    description: 'Join Discord and post in #testnet-intros.',
    howToComplete: 'Join our Discord, post in #testnet-intros with your testnet account address. Submit your Discord handle below.',
    verificationType: 'manual',
    rewardTier: 'bonus',
  },
];

/** Optional: URL for external form if API is not ready (Phase 0). Set to empty to use in-page form only. */
export const EXTERNAL_QUEST_FORM_URL = '';
