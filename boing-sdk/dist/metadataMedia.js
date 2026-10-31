/**
 * Off-chain metadata URI helpers for tokens / NFTs (Boing reference layouts).
 * See `docs/BOING-REFERENCE-NFT.md` and `docs/BOING-REFERENCE-TOKEN.md`.
 */
const MAX_URL = 2048;
/** Rewrite `ipfs://…` to a public HTTPS gateway URL. */
export function ipfsUriToGatewayUrl(uri, gatewayBase = 'https://ipfs.io/ipfs/') {
    const trimmed = uri.trim();
    const m = /^ipfs:\/\/(.+)$/i.exec(trimmed);
    if (!m)
        return null;
    let path = m[1].replace(/^\/+/, '');
    path = path.replace(/^ipfs\//i, '');
    if (!path || path.length > MAX_URL)
        return null;
    const base = gatewayBase.endsWith('/') ? gatewayBase : `${gatewayBase}/`;
    return `${base}${path}`;
}
/** Pull the first `boing://blob/…` URI from free text (L1 media blobs). */
export function extractBoingBlobUri(...sources) {
    for (const raw of sources) {
        if (raw == null)
            continue;
        const text = raw.trim();
        if (!text)
            continue;
        const m = /\bboing:\/\/blob\/0x[0-9a-fA-F]{64}\b/.exec(text);
        if (m && m[0].length <= MAX_URL)
            return m[0];
    }
    return null;
}
/** Pull the first http(s), ipfs://, or boing://blob/ URL from free text. */
export function extractHttpOrIpfsUrl(...sources) {
    for (const raw of sources) {
        if (raw == null)
            continue;
        const text = raw.trim();
        if (!text)
            continue;
        const blob = extractBoingBlobUri(text);
        if (blob)
            return blob;
        const ipfsWord = /\bipfs:\/\/[^\s"'<>]+/i.exec(text);
        if (ipfsWord) {
            const g = ipfsUriToGatewayUrl(ipfsWord[0]);
            if (g && g.length <= MAX_URL)
                return g;
        }
        const httpsWord = /\bhttps:\/\/[^\s"'<>]{4,2048}/i.exec(text);
        if (httpsWord && httpsWord[0].length <= MAX_URL)
            return httpsWord[0];
        const httpWord = /\bhttp:\/\/[^\s"'<>]{4,2048}/i.exec(text);
        if (httpWord && httpWord[0].length <= MAX_URL)
            return httpWord[0];
    }
    return null;
}
function readMetadataImageField(value) {
    if (typeof value !== 'string' || !value.trim())
        return null;
    return extractHttpOrIpfsUrl(value.trim()) ?? value.trim();
}
/** Best-effort image URL from parsed off-chain JSON (OpenSea-style keys). */
export function resolveImageUrlFromMetadataJson(json) {
    if (json == null || typeof json !== 'object')
        return null;
    const o = json;
    for (const key of ['image', 'image_url', 'logo', 'logoURI', 'animation_url', 'content_hash']) {
        const u = readMetadataImageField(o[key]);
        if (u) {
            const blob = extractBoingBlobUri(u);
            if (blob)
                return blob;
            const g = extractHttpOrIpfsUrl(u);
            if (g)
                return g;
            if (/^https?:\/\//i.test(u) && u.length <= MAX_URL)
                return u;
            if (/^boing:\/\/blob\//i.test(u) && u.length <= MAX_URL)
                return u;
        }
    }
    return null;
}
/**
 * Resolve a display image from deploy metadata strings and optional fetched JSON.
 * Parses inline JSON blobs when `asset_name` / `asset_symbol` embed `{ "image": "…" }`.
 */
export function resolveImageUrlFromSources(...sources) {
    for (const raw of sources) {
        if (raw == null)
            continue;
        const text = raw.trim();
        if (!text)
            continue;
        if (text.startsWith('{') || text.startsWith('[')) {
            try {
                const parsed = JSON.parse(text);
                const fromJson = resolveImageUrlFromMetadataJson(parsed);
                if (fromJson)
                    return fromJson;
            }
            catch {
                /* not JSON */
            }
        }
        const direct = extractHttpOrIpfsUrl(text);
        if (direct)
            return direct;
    }
    return null;
}
function isZeroWordHex(hex32) {
    const raw = hex32.replace(/^0x/i, '');
    return /^0+$/.test(raw);
}
/**
 * Candidate fetch URLs for a 32-byte on-chain metadata commitment (reference NFT `metadata_hash` slot).
 * Order: embedded `boing://blob/` / http(s) / ipfs text, then **direct** `boing://blob/0x{word}`
 * (when the hash *is* a BlobId), then `ipfs.io/ipfs/{hex}` fallback.
 *
 * Resolve `boing://blob/…` with RPC `boing_getBlob` (see `mediaBlob.ts` / `BoingClient.getBlob`).
 */
export function metadataHashWordToFetchUrls(metadataHashHex32) {
    const hex = metadataHashHex32.replace(/^0x/i, '');
    if (hex.length !== 64 || isZeroWordHex(hex))
        return [];
    const out = [];
    try {
        const bytes = Uint8Array.from(hex.match(/.{1,2}/g).map((b) => parseInt(b, 16)));
        const ascii = new TextDecoder('utf-8', { fatal: false }).decode(bytes).replace(/\0+$/, '').trim();
        const fromAscii = extractHttpOrIpfsUrl(ascii);
        if (fromAscii)
            out.push(fromAscii);
    }
    catch {
        /* ignore */
    }
    const boingBlob = `boing://blob/0x${hex.toLowerCase()}`;
    if (!out.includes(boingBlob))
        out.push(boingBlob);
    const ipfsPath = `https://ipfs.io/ipfs/${hex}`;
    if (!out.includes(ipfsPath))
        out.push(ipfsPath);
    return out.slice(0, 4);
}
/**
 * If `uri` is `boing://blob/0x…`, return the 32-byte id hex (with 0x); else null.
 * Use with `BoingClient.getBlob(id)`.
 */
export function blobIdHexFromBoingUri(uri) {
    const m = /^boing:\/\/blob\/(0x[0-9a-fA-F]{64})$/i.exec(uri.trim());
    return m ? m[1].toLowerCase() : null;
}
/** Hostnames / literals that should not be fetched from server-side indexers (SSRF). */
export function isPrivateOrLinkLocalHostname(hostname) {
    const h = hostname.trim().toLowerCase().replace(/^\[|\]$/g, '');
    if (!h)
        return true;
    if (h === 'localhost' || h.endsWith('.localhost') || h === '0.0.0.0')
        return true;
    if (h === '::1' || h === '0:0:0:0:0:0:0:1')
        return true;
    // IPv4 dotted
    const m = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/.exec(h);
    if (m) {
        const a = m.slice(1).map((x) => Number(x));
        if (a.some((n) => n > 255))
            return true;
        if (a[0] === 10)
            return true;
        if (a[0] === 127)
            return true;
        if (a[0] === 0)
            return true;
        if (a[0] === 169 && a[1] === 254)
            return true;
        if (a[0] === 172 && a[1] >= 16 && a[1] <= 31)
            return true;
        if (a[0] === 192 && a[1] === 168)
            return true;
        if (a[0] === 100 && a[1] >= 64 && a[1] <= 127)
            return true; // CGNAT
    }
    if (h.startsWith('fc') || h.startsWith('fd') || h.startsWith('fe80:'))
        return true;
    return false;
}
/** True when `url` is http(s) targeting a private / link-local / loopback host. */
export function isPrivateOrLinkLocalUrl(url) {
    try {
        const u = new URL(url);
        if (u.protocol !== 'http:' && u.protocol !== 'https:')
            return false;
        return isPrivateOrLinkLocalHostname(u.hostname);
    }
    catch {
        return true;
    }
}
/** Fetch JSON metadata from HTTPS (or IPFS gateway) with a short timeout. */
export async function fetchMetadataJsonFromUrl(url, options) {
    const timeoutMs = Math.min(30000, Math.max(500, options?.timeoutMs ?? 8000));
    const fetchFn = options?.fetchImpl ?? fetch;
    // Server-side consumers should set blockPrivateUrls: true (audit #61 item 57).
    if (options?.blockPrivateUrls && isPrivateOrLinkLocalUrl(url)) {
        return { ok: false, url, error: 'Blocked private or link-local URL' };
    }
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), timeoutMs);
    try {
        const res = await fetchFn(url, {
            signal: controller.signal,
            headers: { Accept: 'application/json, text/plain, */*' },
        });
        if (!res.ok) {
            return { ok: false, url, error: `HTTP ${res.status}` };
        }
        const json = (await res.json());
        return { ok: true, url, json, imageUrl: resolveImageUrlFromMetadataJson(json) };
    }
    catch (e) {
        const message = e instanceof Error ? e.message : String(e);
        return { ok: false, url, error: message };
    }
    finally {
        clearTimeout(timer);
    }
}
/** Try each URL until one returns parseable JSON metadata. */
export async function fetchFirstMetadataJson(urls, options) {
    for (const url of urls) {
        const result = await fetchMetadataJsonFromUrl(url, options);
        if (result.ok)
            return result;
    }
    return null;
}
