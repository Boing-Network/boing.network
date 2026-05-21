/**
 * Off-chain metadata URI helpers for tokens / NFTs (Boing reference layouts).
 * See `docs/BOING-REFERENCE-NFT.md` and `docs/BOING-REFERENCE-TOKEN.md`.
 */
/** Rewrite `ipfs://…` to a public HTTPS gateway URL. */
export declare function ipfsUriToGatewayUrl(uri: string, gatewayBase?: string): string | null;
/** Pull the first http(s) or ipfs:// URL from free text. */
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
 * Tries: embedded URI text, then `ipfs.io/ipfs/{hex}`.
 */
export declare function metadataHashWordToFetchUrls(metadataHashHex32: string): string[];
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
}): Promise<FetchMetadataJsonResult>;
/** Try each URL until one returns parseable JSON metadata. */
export declare function fetchFirstMetadataJson(urls: readonly string[], options?: {
    timeoutMs?: number;
    fetchImpl?: typeof fetch;
}): Promise<FetchMetadataJsonResult | null>;
//# sourceMappingURL=metadataMedia.d.ts.map