import { describe, expect, it } from 'vitest';
import { boingRpcWebSocketUrl } from '../src/newHeadsWs.js';

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
