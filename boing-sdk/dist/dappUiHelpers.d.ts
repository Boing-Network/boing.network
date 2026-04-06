/**
 * Helpers for wallet / dApp UI copy and outbound links from **`boing_getNetworkInfo`**.
 * Explorer URL layout follows common block-explorer patterns (`/tx/{hash}`, `/address/{hex}`).
 */
import type { NetworkInfo } from './types.js';
/**
 * Human-facing chain title: **`end_user.chain_display_name`**, then **`chain_name`**, then **`chain_id`**, else generic.
 */
export declare function displayChainTitle(info: NetworkInfo): string;
/** Base explorer URL from **`end_user.explorer_url`**, or **`undefined`** when unset. */
export declare function explorerBaseUrl(info: NetworkInfo): string | undefined;
/** Faucet URL from **`end_user.faucet_url`**, or **`undefined`** when unset. */
export declare function faucetUrl(info: NetworkInfo): string | undefined;
/**
 * Transaction deep link: `{base}/tx/{txHash}`.
 * Returns **`undefined`** if no explorer base is configured.
 */
export declare function explorerTxUrl(info: NetworkInfo, txHashHex: string): string | undefined;
/**
 * Account deep link: `{base}/address/{accountIdHex}`.
 * Returns **`undefined`** if no explorer base is configured.
 */
export declare function explorerAccountUrl(info: NetworkInfo, accountIdHex: string): string | undefined;
/**
 * Optional “support” line combining **`explainBoingRpcError`** output with **`x-request-id`** when present.
 */
export declare function formatSupportHint(userMessage: string, requestId?: string | null): string;
//# sourceMappingURL=dappUiHelpers.d.ts.map