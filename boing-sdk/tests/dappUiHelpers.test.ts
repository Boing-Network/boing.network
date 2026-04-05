import { describe, expect, it } from 'vitest';
import type { NetworkDeveloperHints, NetworkInfo } from '../src/types.js';
import {
  displayChainTitle,
  explorerAccountUrl,
  explorerBaseUrl,
  explorerTxUrl,
  faucetUrl,
  formatSupportHint,
} from '../src/dappUiHelpers.js';

function stubDeveloper(): NetworkDeveloperHints {
  return {
    repository_url: '',
    rpc_spec_url: '',
    dapp_integration_doc_url: '',
    sdk_npm_package: '',
    websocket: {
      path: '/ws',
      handshake: { type: 'subscribe', channel: 'newHeads' },
      event_types: ['newHead'],
    },
    api_discovery_methods: [],
    http: {
      live_path: '/live',
      ready_path: '/ready',
      jsonrpc_post_path: '/',
      response_header_rpc_version: 'X-Boing-RPC-Version',
      request_id_header: 'x-request-id',
      supports_jsonrpc_batch: true,
      jsonrpc_batch_max_env: 'BOING_RPC_MAX_BATCH',
      websocket_max_connections_env: 'BOING_RPC_WS_MAX_CONNECTIONS',
      ready_min_peers_env: 'BOING_RPC_READY_MIN_PEERS',
    },
  };
}

function baseInfo(overrides: Partial<NetworkInfo> = {}): NetworkInfo {
  return {
    chain_id: 42,
    chain_name: 'boing-test',
    head_height: 1,
    finalized_height: 1,
    latest_block_hash: `0x${'aa'.repeat(32)}`,
    target_block_time_secs: 1,
    client_version: 'boing-node/0',
    consensus: { validator_count: 1, model: 'test' },
    native_currency: { symbol: 'BOING', decimals: 18 },
    chain_native: {
      account_count: 0,
      total_balance: '0',
      total_stake: '0',
      total_native_held: '0',
      as_of_height: 1,
    },
    developer: stubDeveloper(),
    rpc: { not_available: [], not_available_note: '' },
    ...overrides,
  };
}

describe('dappUiHelpers', () => {
  it('displayChainTitle prefers end_user.chain_display_name', () => {
    expect(
      displayChainTitle(
        baseInfo({
          end_user: {
            chain_display_name: 'Boing Devnet',
            explorer_url: null,
            faucet_url: null,
          },
        }),
      ),
    ).toBe('Boing Devnet');
  });

  it('displayChainTitle falls back to chain_name then chain_id', () => {
    expect(displayChainTitle(baseInfo({ chain_name: 'Named', end_user: undefined }))).toBe('Named');
    expect(
      displayChainTitle(baseInfo({ chain_name: null, chain_id: 7, end_user: undefined })),
    ).toBe('Boing chain 7');
  });

  it('explorer and faucet URLs trim and return undefined when missing', () => {
    expect(explorerBaseUrl(baseInfo())).toBeUndefined();
    expect(faucetUrl(baseInfo())).toBeUndefined();
    const info = baseInfo({
      end_user: {
        chain_display_name: null,
        explorer_url: 'https://ex.example/',
        faucet_url: ' https://faucet.example ',
      },
    });
    expect(explorerBaseUrl(info)).toBe('https://ex.example');
    expect(faucetUrl(info)).toBe('https://faucet.example');
  });

  it('explorerTxUrl and explorerAccountUrl add 0x and paths', () => {
    const info = baseInfo({
      end_user: {
        chain_display_name: null,
        explorer_url: 'https://ex.example',
        faucet_url: null,
      },
    });
    expect(explorerTxUrl(info, `${'bb'.repeat(32)}`)).toBe(
      `https://ex.example/tx/0x${'bb'.repeat(32)}`,
    );
    expect(explorerAccountUrl(info, `${'cc'.repeat(32)}`)).toBe(
      `https://ex.example/address/0x${'cc'.repeat(32)}`,
    );
  });

  it('formatSupportHint appends support id when present', () => {
    expect(formatSupportHint('RPC error', null)).toBe('RPC error');
    expect(formatSupportHint('RPC error', 'req-9')).toContain('Support ID: req-9');
  });
});
