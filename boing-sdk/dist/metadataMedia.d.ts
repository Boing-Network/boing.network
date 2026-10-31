/**
 * Off-chain metadata URI helpers for tokens / NFTs (Boing reference layouts).
 * See `docs/BOING-REFERENCE-NFT.md` and `docs/BOING-REFERENCE-TOKEN.md`.
 */
/** Rewrite `ipfs://…` to a public HTTPS gateway URL. */
export declare function ipfsUriToGatewayUrl(uri: string, gatewayBase?: string): string | null;
/** Pull the first `boing://blob/…` URI from free text (L1 media blobs). */
export declare function extractBoingBlobUri(...sources: (string | null | undefined)[]): string | null;
/** Pull the first http(s), ipfs://, or boing://blob/ URL from free text. */
export declare function extractHttpOrIpfsUrl(...sources: (string | null | undefined)[]): string | null;
/** Best-effort image URL from parsed off-chain JSON (OpenSea-style keys). */
export declare function resolveImageUrlFromMetadataJson(json: unknown): string | null;
/**
 * Resolve a display image from deploy metadata strings and optional fetched JSON.
 * Parses inline JSON blobs when `asset_name` / `asset_symbol` embed `{ "image": "…" }`.
 */
export declare function resolveImageUrlFromSources(...sources: (string | null | undefined)[]): string | null;
/**
 * Candidate fetch URLs for a 32-byte on-chain metadata commitment (reference NFT `metadata_hash` slot).
 * Order: embedded `boing://blob/` / http(s) / ipfs text, then **direct** `boing://blob/0x{word}`
 * (when the hash *is* a BlobId), then `ipfs.io/ipfs/{hex}` fallback.
 *
 * Resolve `boing://blob/…` with RPC `boing_getBlob` (see `mediaBlob.ts` / `BoingClient.getBlob`).
 */
export declare function metadataHashWordToFetchUrls(metadataHashHex32: string): string[];
/**
 * If `uri` is `boing://blob/0x…`, return the 32-byte id hex (with 0x); else null.
 * Use with `BoingClient.getBlob(id)`.
 */
export declare function blobIdHexFromBoingUri(uri: string): string | null;
/** Hostnames / literals that should not be fetched from server-side indexers (SSRF). */
export declare function isPrivateOrLinkLocalHostname(hostname: string): boolean;
/** True when `url` is http(s) targeting a private / link-local / loopback host. */
export declare function isPrivateOrLinkLocalUrl(url: string): boolean;
export type FetchMetadataJsonResult = {
    ok: true;
    url: string;
    json: unknown;
    imageUrl: string | null;
} | {
    ok: false;
    url: string;
    error: string;
};
/** Fetch JSON metadata from HTTPS (or IPFS gateway) with a short timeout. */
export declare function fetchMetadataJsonFromUrl(url: string, options?: {
    timeoutMs?: number;
    fetchImpl?: typeof fetch;
    blockPrivateUrls?: boolean;
}): Promise<FetchMetadataJsonResult>;
/** Try each URL until one returns parseable JSON metadata. */
export declare function fetchFirstMetadataJson(urls: readonly string[], options?: {
    timeoutMs?: number;
    fetchImpl?: typeof fetch;
}): Promise<FetchMetadataJsonResult | null>;
//# sourceMappingURL=metadataMedia.d.ts.map