/**
 * GET /api/passport/check?account_id_hex=0x...[&quest_id=faucet]
 * Read-only dry run of Testnet Passport on-chain quest checks. Writes nothing.
 */
import { isAccountHex, normalizeHex } from '../portal/_lib/hex.js';
import { isAllowedQuestId, PASSPORT_RULES } from '../portal/_lib/quests.js';
import { checkRateLimit, clientIp, rateLimitedResponse } from '../portal/_lib/rateLimit.js';
import { DEFAULT_RPC_URL, verifyAccount } from '../portal/_lib/passportVerify.js';

export async function onRequestGet(context) {
  const { env, request } = context;
  const url = new URL(request.url);
  const account = normalizeHex(url.searchParams.get('account_id_hex') || '');
  if (!isAccountHex(account)) {
    return Response.json({ ok: false, message: 'Missing or invalid account_id_hex' }, { status: 400 });
  }
  const questId = url.searchParams.get('quest_id');
  if (questId && !isAllowedQuestId(questId)) {
    return Response.json({ ok: false, message: 'Unknown quest_id' }, { status: 400 });
  }
  if (env.DB) {
    const rl = await checkRateLimit(env.DB, `passport_check:${clientIp(request)}`, 10, 60);
    if (!rl.ok) return rateLimitedResponse(rl.retryAfterSecs);
  }
  try {
    const out = await verifyAccount(account, questId ? [questId] : null, { rpcUrl: env.TESTNET_RPC_URL || DEFAULT_RPC_URL });
    return Response.json({ ok: true, dry_run: true, rules_version: PASSPORT_RULES.rules_version, ...out });
  } catch (e) {
    return Response.json({ ok: false, message: 'Testnet RPC unavailable', error_code: 'rpc_unavailable' }, { status: 502 });
  }
}
