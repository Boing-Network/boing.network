/**
 * POST /api/quests/submit
 * Submit a quest completion. Requires the account to be a registered user and
 * ownership proof (portal password or wallet signature with action quest_submit).
 * Body: { quest_id, account_id_hex, proof_type?, proof_value?, password? | message+signature }
 * quest_id must be in the allowlist (see portal/_lib/quests.js).
 */
import { isAccountHex, normalizeHex } from '../portal/_lib/hex.js';
import { verifyAccountOwnership } from '../portal/_lib/ownership.js';
import { isAllowedQuestId } from '../portal/_lib/quests.js';

const MAX_PROOF_VALUE_LEN = 512;

export async function onRequestPost(context) {
  const { env, request } = context;
  if (!env.DB) {
    return Response.json({ ok: false, message: 'Database not configured' }, { status: 503 });
  }
  try {
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
    if (reg.role !== 'user') {
      return Response.json({ ok: false, message: 'Only registered users can submit quests', error_code: 'wrong_role' }, { status: 403 });
    }

    const auth = await verifyAccountOwnership(env, account_id_hex, body, 'quest_submit', reg);
    if (!auth.ok) {
      return Response.json(
        { ok: false, message: auth.message, error_code: auth.error_code },
        { status: auth.status || 401 }
      );
    }

    // Ensure the quest row exists (FK). Seeded in schema.sql; reject if missing.
    const questRow = await env.DB.prepare('SELECT id FROM quests WHERE id = ? AND active = 1')
      .bind(quest_id)
      .first();
    if (!questRow) {
      return Response.json({ ok: false, message: 'Quest is not active', error_code: 'quest_inactive' }, { status: 400 });
    }

    const now = new Date().toISOString();
    await env.DB.prepare(
      `INSERT INTO quest_completions (quest_id, account_id_hex, proof_type, proof_value, submitted_at)
       VALUES (?, ?, ?, ?, ?)
       ON CONFLICT(quest_id, account_id_hex) DO UPDATE SET proof_type = excluded.proof_type, proof_value = excluded.proof_value, submitted_at = excluded.submitted_at`
    )
      .bind(quest_id, account_id_hex, proof_type, proof_value, now)
      .run();

    return Response.json({ ok: true, message: 'Submitted' });
  } catch (e) {
    return Response.json({ ok: false, message: e.message || 'Server error' }, { status: 500 });
  }
}

function sanitizeOptionalText(value, maxLen) {
  if (value == null) return null;
  if (typeof value !== 'string') return null;
  const t = value.trim();
  if (!t) return null;
  return t.replace(/[<>]/g, '').slice(0, maxLen) || null;
}
