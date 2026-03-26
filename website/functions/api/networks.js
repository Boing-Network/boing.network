/**
 * GET /api/networks
 * Static network definitions merged with D1 rows from network_listings (same id overlays node_* fields).
 * Used by VibeMiner and other clients; id "boing-devnet" must match @vibeminer/shared.
 */

const CORS = {
  'Access-Control-Allow-Origin': '*',
  'Access-Control-Allow-Methods': 'GET, OPTIONS',
  'Access-Control-Max-Age': '86400',
};

/** Hosts allowed for node_download_url (HTTPS only). */
const ALLOWED_DOWNLOAD_HOSTS = new Set(['github.com', 'www.github.com']);

function isAllowedDownloadHost(hostname) {
  const h = String(hostname || '').toLowerCase();
  if (ALLOWED_DOWNLOAD_HOSTS.has(h)) return true;
  return h.endsWith('.githubusercontent.com');
}

function isAllowedDownloadUrl(urlString) {
  if (!urlString || typeof urlString !== 'string') return false;
  try {
    const u = new URL(urlString.trim());
    if (u.protocol !== 'https:') return false;
    return isAllowedDownloadHost(u.hostname);
  } catch {
    return false;
  }
}

/** Default bootnodes (keep in sync with website/src/config/testnet.ts fallbacks). */
const DEFAULT_BOOTNODES = ['/ip4/73.84.106.121/tcp/4001', '/ip4/73.84.106.121/tcp/4001'];

function staticNetworks() {
  return [
    {
      id: 'boing-devnet',
      name: 'Boing Devnet',
      rpc_url: 'https://testnet-rpc.boing.network/',
      bootnodes: DEFAULT_BOOTNODES,
      chain_id_hex: '0x1b01',
    },
  ];
}

function mergeListing(base, row) {
  const out = { ...base };
  if (!row) return out;

  const url = row.node_download_url?.trim() || '';
  if (url && isAllowedDownloadUrl(url)) {
    out.node_download_url = url;
  }

  const tpl = row.node_command_template?.trim() || '';
  if (tpl) {
    out.node_command_template = tpl;
  }

  const sha = row.node_binary_sha256?.trim() || '';
  if (sha) {
    out.node_binary_sha256 = sha;
  }

  return out;
}

export async function onRequestOptions() {
  return new Response(null, { status: 204, headers: CORS });
}

export async function onRequestGet(context) {
  const { env } = context;
  const headers = { 'Content-Type': 'application/json', ...CORS };

  const bases = staticNetworks();
  const ids = bases.map((n) => n.id);

  if (!env.DB) {
    const networks = bases.map((b) => ({ ...b }));
    return Response.json({ ok: true, networks, warning: 'Database not configured; D1 overrides skipped' }, { headers });
  }

  try {
    const placeholders = ids.map(() => '?').join(',');
    const res = await env.DB.prepare(
      `SELECT id, node_download_url, node_command_template, node_binary_sha256 FROM network_listings WHERE id IN (${placeholders})`
    )
      .bind(...ids)
      .all();

    const byId = new Map();
    for (const r of res.results || []) {
      byId.set(r.id, r);
    }

    const networks = bases.map((b) => mergeListing(b, byId.get(b.id)));
    return Response.json({ ok: true, networks }, { headers });
  } catch (e) {
    return Response.json(
      { ok: false, message: e.message || 'Server error' },
      { status: 500, headers }
    );
  }
}
