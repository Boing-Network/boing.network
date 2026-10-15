/**
 * Boing Express **web wallet** client: popup (preferred) or redirect signing via
 * `https://boing.express/connect` for browsers without the Boing extension (mobile, Safari, Firefox).
 *
 * Use it only as a fallback when `window.boing` is missing; the extension stays the primary path.
 * Protocol and security model: boing.express `docs/WEB-SIGNING-FLOW.md`. Dependency-free.
 */
export declare const BOING_WEB_WALLET_DEFAULT_URL = "https://boing.express";
export declare const BOING_WEB_REQUEST_TYPE = "boing_web_request";
export declare const BOING_WEB_RESPONSE_TYPE = "boing_web_response";
export declare const BOING_WEB_READY_TYPE = "boing_web_ready";
export declare const BOING_WEB_RESPONSE_FRAGMENT_KEY = "boing_web_response";
export type BoingWebMethod = 'boing_requestAccounts' | 'boing_signMessage' | 'boing_signTransaction' | 'boing_sendTransaction';
export interface BoingWebRequest {
    v: 1;
    type: typeof BOING_WEB_REQUEST_TYPE;
    id: string;
    method: BoingWebMethod;
    params: unknown[];
    origin: string;
    returnUrl: string;
    chainId: string;
    issuedAt: number;
    expiresAt: number;
}
export interface BoingWebResponse {
    v: 1;
    type: typeof BOING_WEB_RESPONSE_TYPE;
    id: string;
    origin: string;
    chainId: string;
    result?: unknown;
    error?: {
        code: number;
        message: string;
        data?: {
            boingCode?: string;
        };
    };
}
/** Error shaped like the extension provider's errors (`code`, `message`, `data.boingCode`). */
export declare class BoingWebWalletError extends Error {
    readonly code: number;
    readonly data: {
        boingCode: string;
    };
    constructor(code: number, message: string, data: {
        boingCode: string;
    });
}
export declare function walletOriginOf(walletUrl: string): string;
export interface CreateBoingWebRequestInput {
    method: BoingWebMethod;
    params?: unknown[];
    chainId: string;
    /** Defaults to `window.location.origin`. */
    origin?: string;
    /** Defaults to the current page URL without fragment. Must be on `origin`. */
    returnUrl?: string;
    ttlMs?: number;
    now?: number;
}
export declare function createBoingWebRequest(input: CreateBoingWebRequestInput): BoingWebRequest;
/** Checks a response against the pending request. Throws on mismatch; returns the result or throws the wallet error. */
export declare function validateBoingWebResponse(resp: unknown, req: Pick<BoingWebRequest, 'id' | 'origin' | 'chainId'>): unknown;
export interface PopupOptions {
    walletUrl?: string;
    /** Overall timeout (defaults to the request lifetime). */
    timeoutMs?: number;
    /** A window you already opened synchronously in the click handler (recommended on mobile). */
    popup?: Window | null;
}
/**
 * Opens `/connect?mode=popup`, hands over the request after the wallet's ready ping, and resolves with
 * the result. Call it **synchronously** from a user gesture (before any `await`) so the popup isn't blocked.
 */
export declare function requestViaBoingWebPopup(req: BoingWebRequest, opts?: PopupOptions): Promise<unknown>;
/** Redirect mode: stores the pending request and navigates to the wallet. The result arrives on reload. */
export declare function startBoingWebRedirect(req: BoingWebRequest, walletUrl?: string): void;
/**
 * Call on page load in redirect mode. Returns null when there is no response in the fragment, otherwise
 * `{ method, result }` or `{ method, error }`. The pending request is consumed once and the fragment removed.
 */
export declare function consumeBoingWebRedirectResponse(win?: Window): {
    method: string;
    result?: unknown;
    error?: unknown;
} | null;
export interface BoingWebWalletProviderOptions {
    walletUrl?: string;
    chainId: string;
    mode?: 'popup' | 'redirect';
}
export interface BoingWebWalletProvider {
    isBoingWeb: true;
    request(args: {
        method: string;
        params?: unknown[];
    }): Promise<unknown>;
    on(event: string, cb: (...a: unknown[]) => void): void;
    removeListener(event: string, cb: (...a: unknown[]) => void): void;
    disconnect(): void;
}
/**
 * EIP-1193-style provider backed by the boing.express web wallet. `request()` for wallet methods
 * opens the popup, so call it directly in a click handler.
 */
export declare function createBoingWebWalletProvider(opts: BoingWebWalletProviderOptions): BoingWebWalletProvider;
//# sourceMappingURL=boingWebWallet.d.ts.map