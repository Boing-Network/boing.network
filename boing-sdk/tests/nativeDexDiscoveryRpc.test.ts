import { describe, expect, it, vi } from 'vitest';
import { BoingClient } from '../src/client.js';

describe('DEX discovery RPC helpers', () => {
  it('listDexPoolsPage posts boing_listDexPools with object params', async () => {
    const fetchImpl = vi.fn(async () => {
      return new Response(
        JSON.stringify({
          jsonrpc: '2.0',
          id: 1,
          result: {
            pools: [
              {
                poolHex: '0x' + '11'.repeat(32),
                tokenAHex: '0x' + 'aa'.repeat(32),
                tokenBHex: '0x' + 'bb'.repeat(32),
                tokenADecimals: 18,
                tokenBDecimals: 18,
                feeBps: 30,
                reserveA: '1',
                reserveB: '2',
                createdAtHeight: null,
              },
            ],
            nextCursor: null,
          },
        }),
        { status: 200, headers: { 'Content-Type': 'application/json' } },
      );
    });
    const client = new BoingClient({ baseUrl: 'https://example.invalid', fetch: fetchImpl as typeof fetch });
    const out = await client.listDexPoolsPage({ cursor: 'i0', limit: 5, includeDiagnostics: true });
    expect(out.pools).toHaveLength(1);
    expect(out.pools[0]!.feeBps).toBe(30);
    expect(fetchImpl).toHaveBeenCalledTimes(1);
    const body = JSON.parse((fetchImpl.mock.calls[0]![1] as RequestInit).body as string);
    expect(body.method).toBe('boing_listDexPools');
    expect(body.params).toEqual([{ cursor: 'i0', limit: 5, includeDiagnostics: true }]);
  });

  it('listDexTokensPage and getDexToken forward methods', async () => {
    const fetchImpl = vi.fn(async (_input, init) => {
      const body = JSON.parse((init as RequestInit).body as string) as { method: string };
      if (body.method === 'boing_listDexTokens') {
        return new Response(
          JSON.stringify({
            jsonrpc: '2.0',
            id: 2,
            result: { tokens: [], nextCursor: null },
          }),
          { status: 200, headers: { 'Content-Type': 'application/json' } },
        );
      }
      return new Response(
        JSON.stringify({
          jsonrpc: '2.0',
          id: 3,
          result: null,
        }),
        { status: 200, headers: { 'Content-Type': 'application/json' } },
      );
    });
    const client = new BoingClient({ baseUrl: 'https://example.invalid', fetch: fetchImpl as typeof fetch });
    await client.listDexTokensPage({ minLiquidityWei: '1000' });
    await client.getDexToken('0x' + 'cc'.repeat(32));
    const methods = fetchImpl.mock.calls.map((c) => JSON.parse((c[1] as RequestInit).body as string).method);
    expect(methods).toEqual(['boing_listDexTokens', 'boing_getDexToken']);
  });

  it('getTokenMetadata and listTokensPage forward methods and params', async () => {
    const fetchImpl = vi.fn(async (_input, init) => {
      const body = JSON.parse((init as RequestInit).body as string) as { method: string };
      const result =
        body.method === 'boing_listTokens'
          ? { tokens: [], nextCursor: null, total: 0, indexedThroughHeight: 7 }
          : { id: '0x' + 'cc'.repeat(32), name: null, symbol: null, decimals: 0, totalSupply: null };
      return new Response(JSON.stringify({ jsonrpc: '2.0', id: 1, result }), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      });
    });
    const client = new BoingClient({ baseUrl: 'https://example.invalid', fetch: fetchImpl as typeof fetch });
    const meta = await client.getTokenMetadata('0x' + 'cc'.repeat(32));
    expect(meta?.decimals).toBe(0);
    const page = await client.listTokensPage({ limit: 10 });
    expect(page.indexedThroughHeight).toBe(7);
    const calls = fetchImpl.mock.calls.map((c) => JSON.parse((c[1] as RequestInit).body as string));
    expect(calls[0].method).toBe('boing_getTokenMetadata');
    expect(calls[0].params).toEqual([{ id: '0x' + 'cc'.repeat(32) }]);
    expect(calls[1].method).toBe('boing_listTokens');
    expect(calls[1].params).toEqual([{ limit: 10 }]);
  });
});
