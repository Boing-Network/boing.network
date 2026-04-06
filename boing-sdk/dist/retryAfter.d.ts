/**
 * Parse HTTP **`Retry-After`** (RFC 7231): delay in seconds as integer, or an HTTP-date.
 * Returns milliseconds to wait, or **`undefined`** if missing / unparseable.
 */
export declare function parseRetryAfterMs(value: string | null): number | undefined;
//# sourceMappingURL=retryAfter.d.ts.map