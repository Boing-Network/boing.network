/** GET /api/passport/rules — the published Testnet Passport rules (versioned JSON). */
import { PASSPORT_RULES } from '../portal/_lib/quests.js';

export async function onRequestGet() {
  return Response.json(PASSPORT_RULES, { headers: { 'cache-control': 'public, max-age=300' } });
}
