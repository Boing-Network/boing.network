/**
 * GET /api/networks
 * Static network definitions merged with D1 rows from network_listings (same id overlays node_* fields).
 *
 * **IDs:** `boing-devnet` (Windows zip), `boing-devnet-linux`, `boing-devnet-macos` — clients pick by `platform`.
 * This matches three D1 rows (see `website/migrations/insert-boing-devnet-listing.sql` and
 * `scripts/network-listings-release-sql.mjs`). `@vibeminer/shared` instead exposes one static `boing-devnet`
 * row with `nodePresets` for Windows / Linux / macOS — equivalent coverage, different shape.
 */

const CORS = {
  'Access-Control-Allow-Origin': '*',
  'Access-Control-Allow-Methods': 'GET, HEAD, OPTIONS',
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

const DEVNET_BASE = {
  rpc_url: 'https://testnet-rpc.boing.network/',
  bootnodes: DEFAULT_BOOTNODES,
  chain_id_hex: '0x1b01',
  website: 'https://boing.network',
};

/** Same bootnodes/RPC/chain; `platform` helps clients choose a listing without parsing zip names. */
function staticNetworks() {
  return [
    {
      id: 'boing-devnet',
      name: 'Boing (Testnet) — Windows x86_64',
      platform: 'windows',
      ...DEVNET_BASE,
    },
    {
      id: 'boing-devnet-linux',
      name: 'Boing (Testnet) — Linux x86_64',
      platform: 'linux',
      ...DEVNET_BASE,
    },
    {
      id: 'boing-devnet-macos',
      name: 'Boing (Testnet) — macOS Apple Silicon',
      platform: 'macos',
      ...DEVNET_BASE,
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

/**
 * Build JSON body and status. Shared by GET and HEAD — HEAD must not 404 on Pages
 * (otherwise clients that probe with HEAD see a failed request).
 */
async function networksJsonResponse(context) {
  const { env } = context;
  const headers = { 'Content-Type': 'application/json', ...CORS };

  const bases = staticNetworks();
  const ids = bases.map((n) => n.id);

  if (!env.DB) {
    const networks = bases.map((b) => ({ ...b }));
    return {
      status: 200,
      headers,
      body: JSON.stringify({
        ok: true,
        networks,
        warning: 'Database not configured; D1 overrides skipped',
      }),
    };
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
    return {
      status: 200,
      headers,
      body: JSON.stringify({ ok: true, networks }),
    };
  } catch (e) {
    return {
      status: 500,
      headers,
      body: JSON.stringify({ ok: false, message: e.message || 'Server error' }),
    };
  }
}

export async function onRequestGet(context) {
  const { status, headers, body } = await networksJsonResponse(context);
  return new Response(body, { status, headers });
}

export async function onRequestHead(context) {
  const { status, headers, body } = await networksJsonResponse(context);
  const h = new Headers(headers);
  h.set('Content-Length', String(new TextEncoder().encode(body).length));
  return new Response(null, { status, headers: h });
}
