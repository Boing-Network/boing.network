/**
 * Helpers for wallet / dApp UI copy and outbound links from **`boing_getNetworkInfo`**.
 * Explorer URL layout follows common block-explorer patterns (`/tx/{hash}`, `/address/{hex}`).
 */
function trimSlash(s) {
    return s.replace(/\/+$/, '');
}
function ensure0x32(hex) {
    const t = hex.trim();
    if (t.startsWith('0x') || t.startsWith('0X'))
        return t.toLowerCase();
    return `0x${t.toLowerCase()}`;
}
/**
 * Human-facing chain title: **`end_user.chain_display_name`**, then **`chain_name`**, then **`chain_id`**, else generic.
 */
export function displayChainTitle(info) {
    const du = info.end_user;
    if (du?.chain_display_name != null && du.chain_display_name.trim().length > 0) {
        return du.chain_display_name.trim();
    }
    if (info.chain_name != null && info.chain_name.trim().length > 0) {
        return info.chain_name.trim();
    }
    if (info.chain_id != null) {
        return `Boing chain ${info.chain_id}`;
    }
    return 'Boing Network';
}
/** Base explorer URL from **`end_user.explorer_url`**, or **`undefined`** when unset. */
export function explorerBaseUrl(info) {
    const u = info.end_user?.explorer_url?.trim();
    return u && u.length > 0 ? trimSlash(u) : undefined;
}
/** Faucet URL from **`end_user.faucet_url`**, or **`undefined`** when unset. */
export function faucetUrl(info) {
    const u = info.end_user?.faucet_url?.trim();
    return u && u.length > 0 ? u : undefined;
}
/**
 * Transaction deep link: `{base}/tx/{txHash}`.
 * Returns **`undefined`** if no explorer base is configured.
 */
export function explorerTxUrl(info, txHashHex) {
    const base = explorerBaseUrl(info);
    if (base == null)
        return undefined;
    return `${base}/tx/${ensure0x32(txHashHex)}`;
}
/**
 * Account deep link: `{base}/address/{accountIdHex}`.
 * Returns **`undefined`** if no explorer base is configured.
 */
export function explorerAccountUrl(info, accountIdHex) {
    const base = explorerBaseUrl(info);
    if (base == null)
        return undefined;
    return `${base}/address/${ensure0x32(accountIdHex)}`;
}
/**
 * Optional “support” line combining **`explainBoingRpcError`** output with **`x-request-id`** when present.
 */
export function formatSupportHint(userMessage, requestId) {
    const rid = requestId?.trim();
    if (rid && rid.length > 0) {
        return `${userMessage}\n\nSupport ID: ${rid}`;
    }
    return userMessage;
}
