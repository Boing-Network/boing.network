/**
 * Testnet Passport quest definitions for the testnet portal.
 * Single source of truth: ./passport-rules-v1.json (also published at /testnet/passport/rules-v1.json
 * and served by GET /api/passport/rules). The Pages Functions allowlist reads the same file.
 * Activity may be considered in any future program; quests are not a promise of tokens or a mainnet claim.
 */
import rules from './passport-rules-v1.json';

export type VerificationType = 'on_chain' | 'manual';
export type RewardTier = 'base' | 'validator' | 'bonus' | 'ecosystem';
export type Track = 'user' | 'developer' | 'node_operator';

export interface Quest {
  id: string;
  name: string;
  description: string;
  howToComplete: string;
  verificationType: VerificationType;
  rewardTier: RewardTier;
  tracks: Track[];
  check: string;
}

export const PASSPORT_RULES_VERSION: string = rules.rules_version;
export const PASSPORT_DISCLAIMER: string = rules.disclaimer;
export const PASSPORT_TRACKS = rules.tracks as { id: Track; label: string; description: string }[];
export const QUESTS: Quest[] = rules.quests as Quest[];

/** Optional: URL for an external form. Empty = use the in-page form. */
export const EXTERNAL_QUEST_FORM_URL = '';
