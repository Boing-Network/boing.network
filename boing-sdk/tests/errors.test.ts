import { describe, expect, it } from 'vitest';
import { BoingRpcError, explainBoingRpcError, isBoingRpcMethodNotFound } from '../src/errors.js';

describe('isBoingRpcMethodNotFound', () => {
  it('is true for BoingRpcError -32601', () => {
    expect(
      isBoingRpcMethodNotFound(new BoingRpcError(-32601, 'Method not found: boing_getSyncState', undefined, 'boing_getSyncState'))
    ).toBe(true);
  });

  it('is false for other errors', () => {
    expect(isBoingRpcMethodNotFound(new BoingRpcError(-32000, 'fail'))).toBe(false);
    expect(isBoingRpcMethodNotFound(new Error('x'))).toBe(false);
  });
});

describe('BoingRpcError helpers', () => {
  it('isRateLimited for -32016', () => {
    const e = new BoingRpcError(-32016, 'Rate limit exceeded. Try again later.');
    expect(e.isRateLimited).toBe(true);
    expect(explainBoingRpcError(e)).toContain('Rate limited');
  });

  it('explainBoingRpcError mentions Retry-After for rate limits', () => {
    const e = new BoingRpcError(-32016, 'Too many requests', undefined, undefined, 1000);
    const s = explainBoingRpcError(e);
    expect(s).toContain('Rate limited');
    expect(s).toContain('Retry-After');
    expect(s).toContain('1s');
  });

  it('explainBoingRpcError mentions Retry-After for -32000 with retryAfterMs', () => {
    const e = new BoingRpcError(-32000, 'HTTP 503: Service unavailable', undefined, undefined, 2000);
    expect(explainBoingRpcError(e)).toContain('Retry-After');
    expect(explainBoingRpcError(e)).toContain('2s');
  });

  it('explainBoingRpcError mentions oversize body for HTTP 413', () => {
    const e = new BoingRpcError(
      -32000,
      'HTTP 413: JSON-RPC body exceeds this node limit (raise BOING_RPC_MAX_BODY_MB on the operator if legitimate).'
    );
    expect(explainBoingRpcError(e)).toContain('too large');
  });

  it('explainBoingRpcError covers method not found and invalid params', () => {
    expect(
      explainBoingRpcError(new BoingRpcError(-32601, 'Method not found: boing_foo'))
    ).toContain('not implemented');
    expect(explainBoingRpcError(new BoingRpcError(-32602, 'Invalid params'))).toContain('Invalid RPC params');
  });

  it('explainBoingRpcError covers parse error and invalid request', () => {
    expect(explainBoingRpcError(new BoingRpcError(-32700, 'Parse error:'))).toContain('Invalid JSON');
    expect(explainBoingRpcError(new BoingRpcError(-32600, 'batch exceeds'))).toContain('Invalid JSON-RPC');
  });

  it('explainBoingRpcError covers duplicate mempool submit', () => {
    expect(explainBoingRpcError(new BoingRpcError(-32000, 'Duplicate transaction'))).toContain(
      'already in the mempool',
    );
  });

  it('explainBoingRpcError covers insufficient execution fee', () => {
    expect(
      explainBoingRpcError(new BoingRpcError(-32000, 'insufficient balance for fee: have 50000, need 381612')),
    ).toContain('Not enough native BOING');
  });
});
