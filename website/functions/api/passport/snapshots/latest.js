/** GET /api/passport/snapshots/latest — latest weekly Passport snapshot (tiers + Merkle root). */
export async function onRequestGet({ env }) {
  if (!env.DB) return Response.json({ ok: false, message: 'Database not configured' }, { status: 503 });
  try {
    const row = await env.DB.prepare('SELECT week, merkle_root, rules_version, entries, json, created_at FROM passport_snapshots ORDER BY week DESC LIMIT 1').first();
    if (!row) return Response.json({ ok: true, snapshot: null, message: 'No snapshot yet' });
    return Response.json({ ok: true, snapshot: JSON.parse(row.json) }, { headers: { 'cache-control': 'public, max-age=300' } });
  } catch (e) {
    return Response.json({ ok: false, message: 'Server error' }, { status: 500 });
  }
}
