/**
 * Boing Express **web wallet** client: popup (preferred) or redirect signing via
 * `https://boing.express/connect` for browsers without the Boing extension (mobile, Safari, Firefox).
 *
 * Use it only as a fallback when `window.boing` is missing; the extension stays the primary path.
 * Protocol and security model: boing.express `docs/WEB-SIGNING-FLOW.md`. Dependency-free.
 */

export const BOING_WEB_WALLET_DEFAULT_URL = 'https://boing.express';
export const BOING_WEB_REQUEST_TYPE = 'boing_web_request';
export const BOING_WEB_RESPONSE_TYPE = 'boing_web_response';
export const BOING_WEB_READY_TYPE = 'boing_web_ready';
export const BOING_WEB_RESPONSE_FRAGMENT_KEY = 'boing_web_response';
const PENDING_KEY = 'boing_web_pending';
const ACCOUNTS_KEY = 'boing_web_accounts';

export type BoingWebMethod =
  | 'boing_requestAccounts'
  | 'boing_signMessage'
  | 'boing_signTransaction'
  | 'boing_sendTransaction';

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
  error?: { code: number; message: string; data?: { boingCode?: string } };
}

/** Error shaped like the extension provider's errors (`code`, `message`, `data.boingCode`). */
export class BoingWebWalletError extends Error {
  constructor(
    readonly code: number,
    message: string,
    readonly data: { boingCode: string }
  ) {
    super(message);
    this.name = 'BoingWebWalletError';
  }
}

function randomId(): string {
  const b = new Uint8Array(16);
  globalThis.crypto.getRandomValues(b);
  return Array.from(b, (x) => x.toString(16).padStart(2, '0')).join('');
}

export function walletOriginOf(walletUrl: string): string {
  return new URL(walletUrl).origin;
}

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

export function createBoingWebRequest(input: CreateBoingWebRequestInput): BoingWebRequest {
  const loc = typeof window !== 'undefined' ? window.location : undefined;
  const origin = input.origin ?? loc?.origin;
  if (!origin) throw new Error('origin is required outside a browser');
  const returnUrl = input.returnUrl ?? (loc ? loc.href.split('#')[0] : origin + '/');
  const now = input.now ?? Date.now();
  const ttl = Math.min(input.ttlMs ?? 5 * 60_000, 10 * 60_000);
  return {
    v: 1,
    type: BOING_WEB_REQUEST_TYPE,
    id: randomId(),
    method: input.method,
    params: input.params ?? [],
    origin,
    returnUrl,
    chainId: input.chainId.toLowerCase(),
    issuedAt: now,
    expiresAt: now + ttl,
  };
}

/** Checks a response against the pending request. Throws on mismatch; returns the result or throws the wallet error. */
export function validateBoingWebResponse(resp: unknown, req: Pick<BoingWebRequest, 'id' | 'origin' | 'chainId'>): unknown {
  if (!resp || typeof resp !== 'object') throw new Error('Invalid web wallet response');
  const r = resp as BoingWebResponse;
  if (r.type !== BOING_WEB_RESPONSE_TYPE || r.v !== 1) throw new Error('Invalid web wallet response type');
  if (r.id !== req.id) throw new Error('Web wallet response id mismatch');
  if (r.origin !== req.origin) throw new Error('Web wallet response origin mismatch');
  if (r.chainId !== req.chainId) throw new Error('Web wallet response chain mismatch');
  if (r.error) {
    throw new BoingWebWalletError(r.error.code, r.error.message, { boingCode: r.error.data?.boingCode ?? 'BOING_WEB_ERROR' });
  }
  return r.result;
}

function b64url(s: string): string {
  const bytes = new TextEncoder().encode(s);
  let bin = '';
  for (const b of bytes) bin += String.fromCharCode(b);
  return btoa(bin).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}
function unb64url(s: string): string {
  const bin = atob(s.replace(/-/g, '+').replace(/_/g, '/') + '='.repeat((4 - (s.length % 4)) % 4));
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return new TextDecoder().decode(out);
}

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
export function requestViaBoingWebPopup(req: BoingWebRequest, opts: PopupOptions = {}): Promise<unknown> {
  const walletUrl = opts.walletUrl ?? BOING_WEB_WALLET_DEFAULT_URL;
  const walletOrigin = walletOriginOf(walletUrl);
  const popup =
    opts.popup ?? window.open(`${walletUrl.replace(/\/$/, '')}/connect?mode=popup`, `boing_web_${req.id.slice(0, 8)}`, 'popup,width=420,height=720');
  if (!popup) {
    return Promise.reject(new BoingWebWalletError(4900, 'Popup blocked. Allow popups for this site, or use redirect mode.', { boingCode: 'BOING_WEB_POPUP_BLOCKED' }));
  }
  if (opts.popup) popup.location.href = `${walletUrl.replace(/\/$/, '')}/connect?mode=popup`;
  const timeoutMs = opts.timeoutMs ?? req.expiresAt - Date.now();
  return new Promise((resolve, reject) => {
    let settled = false;
    const finish = (fn: () => void) => {
      if (settled) return;
      settled = true;
      window.removeEventListener('message', onMsg);
      clearInterval(poll);
      clearTimeout(timer);
      fn();
    };
    const onMsg = (ev: MessageEvent) => {
      if (ev.origin !== walletOrigin || ev.source !== popup) return;
      const d = ev.data as { type?: string } | null;
      if (!d || typeof d !== 'object') return;
      if (d.type === BOING_WEB_READY_TYPE) {
        popup.postMessage(req, walletOrigin);
        return;
      }
      if (d.type !== BOING_WEB_RESPONSE_TYPE) return;
      if ((d as BoingWebResponse).id !== req.id) return;
      finish(() => {
        try {
          resolve(validateBoingWebResponse(d, req));
        } catch (e) {
          reject(e);
        }
      });
    };
    window.addEventListener('message', onMsg);
    const poll = setInterval(() => {
      if (popup.closed) finish(() => reject(new BoingWebWalletError(4001, 'Boing Express window was closed.', { boingCode: 'BOING_USER_REJECTED' })));
    }, 400);
    const timer = setTimeout(() => {
      finish(() => reject(new BoingWebWalletError(4001, 'Boing Express request timed out.', { boingCode: 'BOING_WEB_TIMEOUT' })));
      try {
        popup.close();
      } catch {
        /* ignore */
      }
    }, Math.max(1000, timeoutMs));
  });
}

/** Redirect mode: stores the pending request and navigates to the wallet. The result arrives on reload. */
export function startBoingWebRedirect(req: BoingWebRequest, walletUrl = BOING_WEB_WALLET_DEFAULT_URL): void {
  window.sessionStorage.setItem(PENDING_KEY, JSON.stringify({ id: req.id, origin: req.origin, chainId: req.chainId, method: req.method, expiresAt: req.expiresAt }));
  window.location.assign(`${walletUrl.replace(/\/$/, '')}/connect#req=${b64url(JSON.stringify(req))}`);
}

/**
 * Call on page load in redirect mode. Returns null when there is no response in the fragment, otherwise
 * `{ method, result }` or `{ method, error }`. The pending request is consumed once and the fragment removed.
 */
export function consumeBoingWebRedirectResponse(
  win: Window = window
): { method: string; result?: unknown; error?: unknown } | null {
  const h = win.location.hash.replace(/^#/, '');
  const raw = new URLSearchParams(h).get(BOING_WEB_RESPONSE_FRAGMENT_KEY);
  if (!raw) return null;
  win.history.replaceState(null, '', win.location.pathname + win.location.search);
  const pendingRaw = win.sessionStorage.getItem(PENDING_KEY);
  win.sessionStorage.removeItem(PENDING_KEY);
  if (!pendingRaw) return { method: '', error: new Error('No pending Boing Express request (already used or expired).') };
  const pending = JSON.parse(pendingRaw) as { id: string; origin: string; chainId: string; method: string; expiresAt: number };
  try {
    const result = validateBoingWebResponse(JSON.parse(unb64url(raw)), pending);
    if (pending.method === 'boing_requestAccounts' && Array.isArray(result)) {
      win.sessionStorage.setItem(ACCOUNTS_KEY, JSON.stringify(result));
    }
    return { method: pending.method, result };
  } catch (error) {
    return { method: pending.method, error };
  }
}

export interface BoingWebWalletProviderOptions {
  walletUrl?: string;
  chainId: string;
  mode?: 'popup' | 'redirect';
}

export interface BoingWebWalletProvider {
  isBoingWeb: true;
  request(args: { method: string; params?: unknown[] }): Promise<unknown>;
  on(event: string, cb: (...a: unknown[]) => void): void;
  removeListener(event: string, cb: (...a: unknown[]) => void): void;
  disconnect(): void;
}

const METHOD_ALIASES: Record<string, string> = {
  eth_requestAccounts: 'boing_requestAccounts',
  eth_accounts: 'boing_accounts',
  eth_chainId: 'boing_chainId',
  personal_sign: 'boing_signMessage',
  eth_sendTransaction: 'boing_sendTransaction',
  eth_signTransaction: 'boing_signTransaction',
};

/**
 * EIP-1193-style provider backed by the boing.express web wallet. `request()` for wallet methods
 * opens the popup, so call it directly in a click handler.
 */
export function createBoingWebWalletProvider(opts: BoingWebWalletProviderOptions): BoingWebWalletProvider {
  let chainId = opts.chainId.toLowerCase();
  const listeners = new Map<string, Set<(...a: unknown[]) => void>>();
  const emit = (ev: string, ...a: unknown[]) => listeners.get(ev)?.forEach((cb) => cb(...a));
  const storedAccounts = (): string[] => {
    try {
      const v = JSON.parse(window.sessionStorage.getItem(ACCOUNTS_KEY) || '[]');
      return Array.isArray(v) ? v : [];
    } catch {
      return [];
    }
  };
  const run = (method: BoingWebMethod, params: unknown[]): Promise<unknown> => {
    const req = createBoingWebRequest({ method, params, chainId });
    if (opts.mode === 'redirect') {
      startBoingWebRedirect(req, opts.walletUrl);
      return new Promise(() => undefined); // page navigates away
    }
    return requestViaBoingWebPopup(req, { walletUrl: opts.walletUrl });
  };
  return {
    isBoingWeb: true,
    async request({ method: m, params = [] }) {
      const method = METHOD_ALIASES[m] ?? m;
      switch (method) {
        case 'boing_chainId':
          return chainId;
        case 'boing_accounts':
          return storedAccounts();
        case 'boing_switchChain': {
          const p = params[0] as { chainId?: string } | string | undefined;
          const next = (typeof p === 'string' ? p : p?.chainId ?? '').toLowerCase();
          if (!/^0x[0-9a-f]+$/.test(next)) throw new BoingWebWalletError(-32602, 'Invalid chainId', { boingCode: 'BOING_INVALID_PARAMS' });
          if (next !== chainId) {
            chainId = next;
            emit('chainChanged', chainId);
          }
          return null;
        }
        case 'boing_requestAccounts': {
          const accounts = (await run('boing_requestAccounts', [])) as string[];
          window.sessionStorage.setItem(ACCOUNTS_KEY, JSON.stringify(accounts));
          emit('accountsChanged', accounts);
          return accounts;
        }
        case 'boing_signMessage':
        case 'boing_signTransaction':
        case 'boing_sendTransaction':
          return run(method, params.slice(0, 1));
        default:
          throw new BoingWebWalletError(4200, `Method not supported by Boing Express web wallet: ${m}`, { boingCode: 'BOING_WEB_UNSUPPORTED_METHOD' });
      }
    },
    on(ev, cb) {
      if (!listeners.has(ev)) listeners.set(ev, new Set());
      listeners.get(ev)!.add(cb);
    },
    removeListener(ev, cb) {
      listeners.get(ev)?.delete(cb);
    },
    disconnect() {
      window.sessionStorage.removeItem(ACCOUNTS_KEY);
      emit('accountsChanged', []);
    },
  };
}
