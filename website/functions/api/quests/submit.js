/**
 * POST /api/quests/submit
 * Submit a Testnet Passport quest. Any registered portal account (any track) may submit.
 * Requires ownership proof (portal password or wallet signature with action quest_submit).
 * Body: { quest_id, account_id_hex, proof_type?, proof_value?, password? | message+signature }
 *
 * On-chain quests are verified immediately against the public testnet RPC (read-only) and
 * stored as verified; if the chain shows no matching activity nothing is stored.
 * Manual quests are stored as pending until reviewed.
 */
import { isAccountHex, normalizeHex } from '../portal/_lib/hex.js';
import { verifyAccountOwnership } from '../portal/_lib/ownership.js';
import { isAllowedQuestId, QUESTS_BY_ID } from '../portal/_lib/quests.js';
import { checkRateLimit, clientIp, rateLimitedResponse } from '../portal/_lib/rateLimit.js';
import { DEFAULT_RPC_URL, isAutoQuest, verifyAccount } from '../portal/_lib/passportVerify.js';

const MAX_PROOF_VALUE_LEN = 512;

export async function onRequestPost(context) {
  const { env, request } = context;
  if (!env.DB) {
    return Response.json({ ok: false, message: 'Database not configured' }, { status: 503 });
  }
  try {
    const rl = await checkRateLimit(env.DB, `quest_submit:${clientIp(request)}`, 20, 600);
    if (!rl.ok) return rateLimitedResponse(rl.retryAfterSecs);

    const body = await request.json();
    const quest_id = typeof body.quest_id === 'string' ? body.quest_id.trim() : '';
    const account_id_hex = normalizeHex(body.account_id_hex || '');
    const proof_type = sanitizeOptionalText(body.proof_type, 32) || 'form';
    const proof_value = sanitizeOptionalText(body.proof_value, MAX_PROOF_VALUE_LEN);

    if (!quest_id) {
      return Response.json({ ok: false, message: 'Missing quest_id' }, { status: 400 });
    }
    if (!isAllowedQuestId(quest_id)) {
      return Response.json({ ok: false, message: 'Unknown quest_id', error_code: 'invalid_quest' }, { status: 400 });
    }
    if (!isAccountHex(account_id_hex)) {
      return Response.json({ ok: false, message: 'Invalid account_id_hex (must be 32-byte hex with 0x)' }, { status: 400 });
    }

    const reg = await env.DB.prepare(
      'SELECT account_id_hex, role, password_salt, password_hash FROM portal_registrations WHERE account_id_hex = ?'
    )
      .bind(account_id_hex)
      .first();
    if (!reg) {
      return Response.json({ ok: false, message: 'Account not registered', error_code: 'not_registered' }, { status: 403 });
    }

    const auth = await verifyAccountOwnership(env, account_id_hex, body, 'quest_submit', reg);
    if (!auth.ok) {
      return Response.json(
        { ok: false, message: auth.message, error_code: auth.error_code },
        { status: auth.status || 401 }
      );
    }

    const questRow = await env.DB.prepare('SELECT id FROM quests WHERE id = ? AND active = 1')
      .bind(quest_id)
      .first();
    if (!questRow) {
      return Response.json({ ok: false, message: 'Quest is not active', error_code: 'quest_inactive' }, { status: 400 });
    }

    const now = new Date().toISOString();

    if (isAutoQuest(quest_id)) {
      let result;
      try {
        const out = await verifyAccount(account_id_hex, [quest_id], { rpcUrl: env.TESTNET_RPC_URL || DEFAULT_RPC_URL });
        result = out.results[quest_id];
      } catch (e) {
        return Response.json(
          { ok: false, message: 'Could not reach the testnet RPC to verify. Try again shortly.', error_code: 'rpc_unavailable' },
          { status: 502 }
        );
      }
      if (!result || !result.verified) {
        return Response.json(
          {
            ok: false,
            verified: false,
            error_code: 'not_verified',
            message: `No matching on-chain activity yet: ${QUESTS_BY_ID[quest_id].check}`,
          },
          { status: 422 }
        );
      }
      await env.DB.prepare(
        `INSERT INTO quest_completions (quest_id, account_id_hex, proof_type, proof_value, submitted_at, verified_at, verification_status, verified_by)
         VALUES (?, ?, 'on_chain', ?, ?, ?, 'verified', 'rpc')
         ON CONFLICT(quest_id, account_id_hex) DO UPDATE SET proof_type = 'on_chain', proof_value = excluded.proof_value,
           verified_at = COALESCE(quest_completions.verified_at, excluded.verified_at),
           verification_status = 'verified', verified_by = 'rpc', rejected_reason = NULL`
      )
        .bind(quest_id, account_id_hex, result.evidence || null, now, now)
        .run();
      return Response.json({ ok: true, verified: true, evidence: result.evidence || null, message: 'Verified on-chain' });
    }

    if (!proof_value) {
      return Response.json(
        { ok: false, message: 'This quest is reviewed manually; add proof (link, handle, or note).', error_code: 'proof_required' },
        { status: 400 }
      );
    }
    await env.DB.prepare(
      `INSERT INTO quest_completions (quest_id, account_id_hex, proof_type, proof_value, submitted_at, verification_status)
       VALUES (?, ?, ?, ?, ?, 'pending')
       ON CONFLICT(quest_id, account_id_hex) DO UPDATE SET proof_type = excluded.proof_type, proof_value = excluded.proof_value,
         submitted_at = excluded.submitted_at, verification_status = 'pending', rejected_reason = NULL
       WHERE quest_completions.verified_at IS NULL`
    )
      .bind(quest_id, account_id_hex, proof_type, proof_value, now)
      .run();

    return Response.json({ ok: true, verified: false, message: 'Submitted for review' });
  } catch (e) {
    return Response.json({ ok: false, message: 'Server error' }, { status: 500 });
  }
}

function sanitizeOptionalText(value, maxLen) {
  if (value == null) return null;
  if (typeof value !== 'string') return null;
  const t = value.trim();
  if (!t) return null;
  return t.replace(/[<>]/g, '').slice(0, maxLen) || null;
}
