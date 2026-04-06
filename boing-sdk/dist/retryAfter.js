/** Cap parsed delay so a bad header cannot stall the client for hours. */
const MAX_RETRY_AFTER_MS = 86400000; // 24h
/**
 * Parse HTTP **`Retry-After`** (RFC 7231): delay in seconds as integer, or an HTTP-date.
 * Returns milliseconds to wait, or **`undefined`** if missing / unparseable.
 */
export function parseRetryAfterMs(value) {
    if (value == null)
        return undefined;
    const trimmed = value.trim();
    if (trimmed === '')
        return undefined;
    const asInt = Number.parseInt(trimmed, 10);
    if (!Number.isNaN(asInt) && asInt >= 0 && String(asInt) === trimmed) {
        return Math.min(asInt * 1000, MAX_RETRY_AFTER_MS);
    }
    const when = Date.parse(trimmed);
    if (!Number.isNaN(when)) {
        const delta = when - Date.now();
        if (delta > 0)
            return Math.min(delta, MAX_RETRY_AFTER_MS);
    }
    return undefined;
}
