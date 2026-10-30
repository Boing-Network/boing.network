/**
 * Canonical quest IDs for POST /api/quests/submit.
 * Must stay aligned with website/schema.sql seeds and website/src/config/quests.ts.
 */
export const ALLOWED_QUEST_IDS = Object.freeze([
  'faucet',
  'first_tx',
  'validator_connect',
  'feedback',
  'social',
]);

export function isAllowedQuestId(questId) {
  return typeof questId === 'string' && ALLOWED_QUEST_IDS.includes(questId);
}
