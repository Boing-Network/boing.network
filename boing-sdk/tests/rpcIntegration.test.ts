/**
 * Live JSON-RPC checks against a running Boing node.
 *
 * **Skipped entirely** when **`BOING_INTEGRATION_RPC_URL`** is unset.
 *
 * **Optional methods:** Some endpoints return **-32601 Method not found** for **`boing_getSyncState`**
 * or **`boing_getLogs`** (older binaries or minimal gateways). Those cases use fallbacks or no-op
 * success so **`npm run verify`** still passes against a minimal Boing JSON-RPC surface.
 * Always-on when live: **`getTransactionReceipt`** for an all-zero tx id returns **`null`**.
 *
 * **Strict discovery:** Set **`BOING_EXPECT_FULL_RPC=1`** (e.g. CI with a current **`boing-node`**) to run an
 * extra test that asserts **`boing_clientVersion`**, **`boing_rpcSupportedMethods`**, a full **`probeBoingRpcCapabilities`** pass
 * (six core probes, including **`boing_getNetworkInfo`** / **`boing_getBlockByHeight`** / **`boing_getTransactionReceipt`**), and **`planIndexerCatchUp`** from cursor **-1**.
 * **Chain metadata:** With **`BOING_EXPECT_CHAIN_ID`** / **`BOING_EXPECT_CHAIN_NAME`** set (CI sets **6913** / **Boing Testnet**), **`getNetworkInfo`** must match — requires the node process to export **`BOING_CHAIN_ID`** / **`BOING_CHAIN_NAME`**.
 */
import { describe, expect, it } from 'vitest';
import {
  countAvailableBoingRpcMethods,
  createClient,
  fetchBlocksWithReceiptsForHeightRange,
  getIndexerChainTips,
  getLogsChunked,
  isBoingRpcMethodNotFound,
  planIndexerCatchUp,
  probeBoingRpcCapabilities,
} from '../src/index.js';

const rpcUrl = process.env.BOING_INTEGRATION_RPC_URL?.trim();
const run = rpcUrl != null && rpcUrl.length > 0;
const expectFullDiscovery =
  process.env.BOING_EXPECT_FULL_RPC === '1' || process.env.BOING_EXPECT_FULL_RPC === 'true';
const expectChainIdRaw = process.env.BOING_EXPECT_CHAIN_ID?.trim();
const expectChainNameRaw = process.env.BOING_EXPECT_CHAIN_NAME?.trim();

describe.skipIf(!run)(
  run
    ? 'RPC integration (BOING_INTEGRATION_RPC_URL set)'
    : 'RPC integration (SKIPPED — set BOING_INTEGRATION_RPC_URL to run live tests)',
  () => {
    it('chainHeight matches getIndexerChainTips when boing_getSyncState exists', async () => {
      const client = createClient(rpcUrl!);
      const height = await client.chainHeight();
      try {
        const tips = await getIndexerChainTips(client);
        expect(tips.headHeight).toBe(height);
        expect(tips.finalizedHeight).toBe(height);
        expect(tips.durableIndexThrough).toBe(height);
        expect(tips.latestBlockHash).toMatch(/^0x[0-9a-f]{64}$/i);
      } catch (e) {
        if (!isBoingRpcMethodNotFound(e)) throw e;
        const head = await client.getBlockByHeight(height, false);
        expect(head).not.toBeNull();
        expect(head!.header.height).toBe(height);
      }
    });

    it('getBlockByHeight(0 or tip, true) returns object or null', async () => {
      const client = createClient(rpcUrl!);
      const tip = await client.chainHeight();
      const genesis = await client.getBlockByHeight(0, true);
      const head = await client.getBlockByHeight(tip, true);
      if (genesis != null) {
        expect(genesis.header).toBeDefined();
        expect(Array.isArray(genesis.transactions)).toBe(true);
      }
      expect(head).not.toBeNull();
      if (head != null) {
        expect(head.header.height).toBe(tip);
      }
    });

    it('getLogsChunked when boing_getLogs exists', async () => {
      const client = createClient(rpcUrl!);
      try {
        const logs = await getLogsChunked(client, { fromBlock: 0, toBlock: 0 });
        expect(Array.isArray(logs)).toBe(true);
      } catch (e) {
        if (!isBoingRpcMethodNotFound(e)) throw e;
      }
    });

    it('fetchBlocksWithReceiptsForHeightRange returns tip block', async () => {
      const client = createClient(rpcUrl!);
      const tip = await client.chainHeight();
      const bundles = await fetchBlocksWithReceiptsForHeightRange(client, tip, tip, {
        maxConcurrent: 1,
      });
      expect(bundles).toHaveLength(1);
      expect(bundles[0]!.height).toBe(tip);
      expect(bundles[0]!.block.header.height).toBe(tip);
    });

    it('getTransactionReceipt for unknown tx id returns null', async () => {
      const client = createClient(rpcUrl!);
      const unknownTx = `0x${'00'.repeat(32)}`;
      const receipt = await client.getTransactionReceipt(unknownTx);
      expect(receipt).toBeNull();
    });

    it('getNetworkInfo when boing_getNetworkInfo exists', async () => {
      const client = createClient(rpcUrl!);
      try {
        const info = await client.getNetworkInfo();
        expect(info.head_height).toBeGreaterThanOrEqual(0);
        expect(info.finalized_height).toBe(info.head_height);
        expect(info.latest_block_hash).toMatch(/^0x[0-9a-f]{64}$/i);
        expect(info.target_block_time_secs).toBeGreaterThan(0);
        expect(info.client_version).toMatch(/^boing-node\//);
        expect(info.consensus.validator_count).toBeGreaterThan(0);
        expect(info.consensus.model).toBe('single_round_quorum_commit');
        expect(info.native_currency.symbol).toBe('BOING');
        expect(info.chain_native.account_count).toBeGreaterThanOrEqual(1);
        expect(BigInt(info.chain_native.total_balance)).toBeGreaterThanOrEqual(0n);
        expect(BigInt(info.chain_native.total_stake)).toBeGreaterThanOrEqual(0n);
        expect(info.chain_native.as_of_height).toBe(info.head_height);
        expect(info.rpc.not_available).not.toContain('chain_wide_total_stake');
        expect(info.rpc.not_available).toContain('staking_apy');
        expect(typeof info.rpc.not_available_note).toBe('string');
        expect(info.developer.sdk_npm_package).toBe('@boing-network/boing-sdk');
        expect(info.developer.websocket.path).toBe('/ws');
        expect(info.developer.api_discovery_methods).toContain('boing_getRpcMethodCatalog');
        expect(info.developer.http.live_path).toBe('/live');
        expect(info.developer.http.ready_path).toBe('/ready');
        expect(info.developer.http.supports_jsonrpc_batch).toBe(true);
        expect(info.developer.http.jsonrpc_batch_max_env).toBe('BOING_RPC_MAX_BATCH');
        expect(info.developer.http.request_id_header).toBe('x-request-id');
        if (info.developer.http.openapi_http_path != null) {
          expect(info.developer.http.openapi_http_path).toBe('/openapi.json');
        }
        if (info.rpc_surface != null) {
          expect(info.rpc_surface.http_max_body_megabytes).toBeGreaterThanOrEqual(1);
          expect(info.rpc_surface.get_logs_max_block_range).toBeGreaterThan(0);
        }
        if (expectChainIdRaw != null && expectChainIdRaw.length > 0) {
          expect(info.chain_id).toBe(Number(expectChainIdRaw));
        }
        if (expectChainNameRaw != null && expectChainNameRaw.length > 0) {
          expect(info.chain_name).toBe(expectChainNameRaw);
        }
      } catch (e) {
        if (!isBoingRpcMethodNotFound(e)) throw e;
      }
    });

    it.skipIf(!expectFullDiscovery)(
      'discovery + probe: current boing-node surface (BOING_EXPECT_FULL_RPC=1)',
      async () => {
        const client = createClient(rpcUrl!);
        const v = await client.clientVersion();
        expect(v).toMatch(/^boing-node\//);
        const methods = await client.rpcSupportedMethods();
        expect(methods.length).toBeGreaterThan(10);
        expect(methods).toContain('boing_chainHeight');
        expect(methods).toContain('boing_getSyncState');
        expect(methods).toContain('boing_getLogs');
        expect(methods).toContain('boing_clientVersion');
        expect(methods).toContain('boing_getBlockByHeight');
        expect(methods).toContain('boing_getTransactionReceipt');
        expect(methods).toContain('boing_getNetworkInfo');

        const probe = await probeBoingRpcCapabilities(client);
        expect(probe.clientVersion).toBe(v);
        expect(countAvailableBoingRpcMethods(probe)).toBe(6);
        expect(probe.methods.boing_chainHeight.available).toBe(true);
        expect(probe.methods.boing_getSyncState.available).toBe(true);
        expect(probe.methods.boing_getBlockByHeight.available).toBe(true);
        expect(probe.methods.boing_getLogs.available).toBe(true);
        expect(probe.methods.boing_getTransactionReceipt.available).toBe(true);
        expect(probe.methods.boing_getNetworkInfo.available).toBe(true);

        const preflight = await client.preflightRpc();
        expect(preflight.httpOpenApiJsonOk).toBe(true);
        expect(preflight.wellKnownBoingRpcOk).toBe(true);
        expect(preflight.httpLiveJsonOk).toBe(true);
        expect(preflight.health.rpc_metrics).toBeDefined();
        expect(preflight.health.rpc_surface?.http_max_body_megabytes).toBeGreaterThanOrEqual(1);
      }
    );

    it.skipIf(!expectFullDiscovery)(
      'planIndexerCatchUp from cursor -1 matches durable tip (BOING_EXPECT_FULL_RPC=1)',
      async () => {
        const client = createClient(rpcUrl!);
        const tip = await client.chainHeight();
        const plan = await planIndexerCatchUp(client, -1);
        expect(plan).not.toBeNull();
        expect(plan!.fromHeight).toBe(0);
        expect(plan!.toHeight).toBeGreaterThanOrEqual(0);
        expect(plan!.toHeight).toBeLessThanOrEqual(tip);
        expect(plan!.tips.headHeight).toBe(tip);
        expect(plan!.tips.durableIndexThrough).toBe(tip);
      }
    );
  }
);
