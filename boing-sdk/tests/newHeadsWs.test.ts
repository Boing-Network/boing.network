import { afterEach, describe, expect, it, vi } from 'vitest';
import { boingRpcWebSocketUrl, BoingNewHeadsWs } from '../src/newHeadsWs.js';

describe('boingRpcWebSocketUrl', () => {
  it('maps http to ws and https to wss', () => {
    expect(boingRpcWebSocketUrl('http://rpc.example:8545', '/ws')).toBe('ws://rpc.example:8545/ws');
    expect(boingRpcWebSocketUrl('https://rpc.example/', '/ws')).toBe('wss://rpc.example/ws');
  });

  it('defaults path to /ws and accepts host without scheme', () => {
    expect(boingRpcWebSocketUrl('127.0.0.1:8545')).toBe('ws://127.0.0.1:8545/ws');
  });

  it('normalizes custom path', () => {
    expect(boingRpcWebSocketUrl('http://h', 'stream')).toBe('ws://h/stream');
  });
});

/** Flush a few microtask rounds (mock **WebSocket** uses **queueMicrotask** for **onopen** / **onmessage**). */
async function flushMicrotasks(n = 8): Promise<void> {
  for (let i = 0; i < n; i += 1) {
    await Promise.resolve();
  }
}

describe('BoingNewHeadsWs (mock WebSocket)', () => {
  const OriginalWs = globalThis.WebSocket;

  afterEach(() => {
    globalThis.WebSocket = OriginalWs;
    vi.useRealTimers();
  });

  it('sends subscribe, reaches subscribed, and delivers newHead', async () => {
    const sockets: MockSocket[] = [];

    class MockSocket {
      url: string;
      sent: string[] = [];
      onopen: (() => void) | null = null;
      onmessage: ((ev: MessageEvent) => void) | null = null;
      onclose: ((ev: CloseEvent) => void) | null = null;
      onerror: (() => void) | null = null;

      constructor(url: string) {
        this.url = url;
        sockets.push(this);
        queueMicrotask(() => {
          this.onopen?.();
        });
      }

      send(data: string): void {
        this.sent.push(data);
        const msg = JSON.parse(data) as { type?: string; channel?: string };
        if (msg.type === 'subscribe' && msg.channel === 'newHeads') {
          queueMicrotask(() => {
            this.onmessage?.({ data: JSON.stringify({ type: 'subscribed', channel: 'newHeads' }) } as MessageEvent);
            this.onmessage?.({
              data: JSON.stringify({ type: 'newHead', height: 42 }),
            } as MessageEvent);
          });
        }
      }

      close(): void {
        queueMicrotask(() => {
          this.onclose?.({} as CloseEvent);
        });
      }
    }

    globalThis.WebSocket = MockSocket as unknown as typeof WebSocket;

    const heads: unknown[] = [];
    let subscribedCalls = 0;
    const sub = new BoingNewHeadsWs('http://127.0.0.1:8545', {
      onHead: (p) => heads.push(p),
      onSubscribed: () => {
        subscribedCalls += 1;
      },
    });

    sub.connect();
    await flushMicrotasks();

    expect(sockets).toHaveLength(1);
    expect(sockets[0].url).toBe('ws://127.0.0.1:8545/ws');
    expect(sockets[0].sent.some((s) => s.includes('subscribe'))).toBe(true);
    expect(sub.getState()).toBe('subscribed');
    expect(subscribedCalls).toBe(1);
    expect(heads).toHaveLength(1);
    expect((heads[0] as { height?: number }).height).toBe(42);

    sub.close();
    await flushMicrotasks();
    expect(sub.getState()).toBe('closed');
  });

  it('opens a second socket after remote close (reconnect)', async () => {
    vi.useFakeTimers();
    const sockets: MockSocket[] = [];

    class MockSocket {
      url: string;
      sent: string[] = [];
      onopen: (() => void) | null = null;
      onmessage: ((ev: MessageEvent) => void) | null = null;
      onclose: ((ev: CloseEvent) => void) | null = null;
      onerror: (() => void) | null = null;

      constructor(url: string) {
        this.url = url;
        sockets.push(this);
        queueMicrotask(() => {
          this.onopen?.();
        });
      }

      send(data: string): void {
        this.sent.push(data);
        const msg = JSON.parse(data) as { type?: string };
        if (msg.type === 'subscribe') {
          queueMicrotask(() => {
            this.onmessage?.({ data: JSON.stringify({ type: 'subscribed', channel: 'newHeads' }) } as MessageEvent);
          });
        }
      }

      close(): void {
        queueMicrotask(() => {
          this.onclose?.({} as CloseEvent);
        });
      }
    }

    globalThis.WebSocket = MockSocket as unknown as typeof WebSocket;

    const sub = new BoingNewHeadsWs('http://localhost:8545', {
      initialReconnectDelayMs: 50,
      maxReconnectAttempts: 4,
    });
    sub.connect();
    await flushMicrotasks();
    expect(sockets).toHaveLength(1);
    expect(sub.getState()).toBe('subscribed');

    sockets[0].close();
    await flushMicrotasks();
    expect(sub.getState()).toBe('reconnecting');

    await vi.advanceTimersByTimeAsync(50);
    await flushMicrotasks();

    expect(sockets.length).toBeGreaterThanOrEqual(2);
    expect(sockets[1].sent.some((s) => s.includes('subscribe'))).toBe(true);
    expect(sub.getState()).toBe('subscribed');

    sub.close();
    await flushMicrotasks();
  });
});
