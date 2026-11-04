/**
 * Canonical quest IDs for POST /api/quests/submit, read from the published Testnet Passport rules
 * (website/src/config/passport-rules-v1.json), so the page, the API and the D1 seed cannot drift.
 */
import rules from '../../../../src/config/passport-rules-v1.json' with { type: 'json' };

export const PASSPORT_RULES = rules;
export const QUESTS_BY_ID = Object.freeze(Object.fromEntries(rules.quests.map((q) => [q.id, q])));
export const ALLOWED_QUEST_IDS = Object.freeze(rules.quests.map((q) => q.id));
export const TRACK_IDS = Object.freeze(rules.tracks.map((t) => t.id));

export function isAllowedQuestId(questId) {
  return typeof questId === 'string' && Object.prototype.hasOwnProperty.call(QUESTS_BY_ID, questId);
}

export function isTrackId(t) {
  return typeof t === 'string' && TRACK_IDS.includes(t);
}
