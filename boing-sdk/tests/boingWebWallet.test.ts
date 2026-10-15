import { describe, expect, it } from 'vitest';
import {
  BoingWebWalletError,
  consumeBoingWebRedirectResponse,
  createBoingWebRequest,
  validateBoingWebResponse,
} from '../src/boingWebWallet.js';

const ORIGIN = 'https://boing.finance';

function resp(over: Record<string, unknown> = {}) {
  return { v: 1, type: 'boing_web_response', id: 'a'.repeat(32), origin: ORIGIN, chainId: '0x1b01', result: ['0x' + '1'.repeat(64)], ...over };
}

describe('boingWebWallet', () => {
  it('creates bounded requests with random ids', () => {
    const a = createBoingWebRequest({ method: 'boing_requestAccounts', chainId: '0x1B01', origin: ORIGIN, returnUrl: ORIGIN + '/', now: 1000, ttlMs: 60 * 60_000 });
    const b = createBoingWebRequest({ method: 'boing_requestAccounts', chainId: '0x1b01', origin: ORIGIN, returnUrl: ORIGIN + '/', now: 1000 });
    expect(a.id).toMatch(/^[0-9a-f]{32}$/);
    expect(a.id).not.toBe(b.id);
    expect(a.chainId).toBe('0x1b01');
    expect(a.expiresAt - a.issuedAt).toBe(10 * 60_000);
  });

  it('validates id, origin and chain of responses', () => {
    const req = { id: 'a'.repeat(32), origin: ORIGIN, chainId: '0x1b01' };
    expect(validateBoingWebResponse(resp(), req)).toEqual(['0x' + '1'.repeat(64)]);
    expect(() => validateBoingWebResponse(resp({ id: 'b'.repeat(32) }), req)).toThrow(/id mismatch/);
    expect(() => validateBoingWebResponse(resp({ origin: 'https://evil.example' }), req)).toThrow(/origin mismatch/);
    expect(() => validateBoingWebResponse(resp({ chainId: '0x1b02' }), req)).toThrow(/chain mismatch/);
    try {
      validateBoingWebResponse(resp({ result: undefined, error: { code: 4001, message: 'no', data: { boingCode: 'BOING_USER_REJECTED' } } }), req);
    } catch (e) {
      expect(e).toBeInstanceOf(BoingWebWalletError);
      expect((e as BoingWebWalletError).code).toBe(4001);
    }
  });

  it('consumes a redirect response once', () => {
    const store = new Map<string, string>();
    const r = resp();
    const frag = Buffer.from(JSON.stringify(r)).toString('base64url');
    let hash = `#boing_web_response=${frag}`;
    const win = {
      location: { get hash() { return hash; }, pathname: '/swap', search: '' },
      history: { replaceState: () => { hash = ''; } },
      sessionStorage: {
        getItem: (k: string) => store.get(k) ?? null,
        setItem: (k: string, v: string) => void store.set(k, v),
        removeItem: (k: string) => void store.delete(k),
      },
    } as unknown as Window;
    store.set('boing_web_pending', JSON.stringify({ id: r.id, origin: ORIGIN, chainId: '0x1b01', method: 'boing_requestAccounts', expiresAt: 0 }));
    const out = consumeBoingWebRedirectResponse(win);
    expect(out?.result).toEqual(r.result);
    expect(hash).toBe('');
    // Replayed fragment: no pending request any more.
    hash = `#boing_web_response=${frag}`;
    expect(consumeBoingWebRedirectResponse(win)?.error).toBeInstanceOf(Error);
  });
});
