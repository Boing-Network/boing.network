import { describe, expect, it } from 'vitest';
import { firstServedReceiptHeight, storageTreeV2ActiveAt } from '../src/chainParameters.js';
import { BoingClient } from '../src/client.js';
import type { BoingHealth, NetworkInfo } from '../src/types.js';

describe('storageTreeV2ActiveAt', () => {
  it('is unknown when the node does not publish the field', () => {
    expect(storageTreeV2ActiveAt({}, 10)).toBeUndefined();
  });
  it('is false when not configured (null)', () => {
    expect(storageTreeV2ActiveAt({ storage_tree_v2_activation_height: null }, 10_000_000)).toBe(false);
  });
  it('switches exactly at the activation height', () => {
    const info = { storage_tree_v2_activation_height: 500 };
    expect(storageTreeV2ActiveAt(info, 499)).toBe(false);
    expect(storageTreeV2ActiveAt(info, 500)).toBe(true);
    expect(storageTreeV2ActiveAt(info, 501)).toBe(true);
    expect(storageTreeV2ActiveAt({ storage_tree_v2_activation_height: 0 }, 0)).toBe(true);
  });
});

describe('firstServedReceiptHeight', () => {
  it('is unknown when the node does not publish the field', () => {
    expect(firstServedReceiptHeight({ head_height: 10 })).toBeUndefined();
  });
  it('is 0 when unbounded (null) or 0', () => {
    expect(firstServedReceiptHeight({ receipt_retention_blocks: null, head_height: 5_000_000 })).toBe(0);
    expect(firstServedReceiptHeight({ receipt_retention_blocks: 0, head_height: 5 })).toBe(0);
  });
  it('is tip - window + 1, floored at 0', () => {
    expect(firstServedReceiptHeight({ receipt_retention_blocks: 1_000_000, head_height: 3_000_000 })).toBe(2_000_001);
    expect(firstServedReceiptHeight({ receipt_retention_blocks: 1_000_000, head_height: 42 })).toBe(0);
    expect(firstServedReceiptHeight({ receipt_retention_blocks: 100 }, 1_000)).toBe(901);
  });
  it('is unknown for a bounded window without a tip', () => {
    expect(firstServedReceiptHeight({ receipt_retention_blocks: 100 })).toBeUndefined();
  });
});

describe('client types round-trip the published shapes', () => {
  it('getNetworkInfo / health expose the fields typed', async () => {
    const info = {
      storage_tree_v2_activation_height: null,
      receipt_retention_blocks: 1_000_000,
      head_height: 42,
    } as unknown as NetworkInfo;
    const health = {
      ok: true,
      head_height: 42,
      storage_tree_v2_activation_height: 7,
      receipt_retention_blocks: null,
    } as unknown as BoingHealth;
    const fetchImpl = (async (_url: unknown, init?: { body?: string }) => {
      const m = JSON.parse(init?.body ?? '{}') as { id: number; method: string };
      const result = m.method === 'boing_health' ? health : info;
      return new Response(JSON.stringify({ jsonrpc: '2.0', id: m.id, result }), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      });
    }) as unknown as typeof fetch;
    const client = new BoingClient({ baseUrl: 'http://127.0.0.1:1', fetch: fetchImpl } as never);
    const n = await client.getNetworkInfo();
    expect(n.storage_tree_v2_activation_height).toBeNull();
    expect(n.receipt_retention_blocks).toBe(1_000_000);
    const h = await client.health();
    expect(h.storage_tree_v2_activation_height).toBe(7);
    expect(h.receipt_retention_blocks).toBeNull();
  });
});