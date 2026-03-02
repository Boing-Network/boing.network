/**
 * POST /api/quests/submit
 * Submit a quest completion. Body: { quest_id, account_id_hex, proof_type?, proof_value? }
 */
export async function onRequestPost(context) {
  const { env, request } = context;
  if (!env.DB) {
    return Response.json({ ok: false, message: 'Database not configured' }, { status: 503 });
  }
  try {
    const body = await request.json();
    const quest_id = body.quest_id?.trim();
    const account_id_hex = normalizeHex(body.account_id_hex || '');
    const proof_type = body.proof_type?.trim() || 'form';
    const proof_value = body.proof_value?.trim() || null;

    if (!quest_id) {
      return Response.json({ ok: false, message: 'Missing quest_id' }, { status: 400 });
    }
    if (!account_id_hex || account_id_hex.length !== 66) {
      return Response.json({ ok: false, message: 'Invalid account_id_hex (must be 32-byte hex with 0x)' }, { status: 400 });
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

function normalizeHex(s) {
  if (!s || typeof s !== 'string') return '';
  const t = s.trim().toLowerCase();
  return t.startsWith('0x') ? t : '0x' + t;
}
