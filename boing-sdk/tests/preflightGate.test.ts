import { describe, expect, it, vi, beforeEach, afterAll } from 'vitest';
import type { BoingClient } from '../src/client.js';
import type { BoingRpcDoctorResult } from '../src/rpcDoctor.js';
import * as rpcDoctor from '../src/rpcDoctor.js';
import type { BoingRpcPreflightResult } from '../src/types.js';
import type { BoingRpcProbeBundle } from '../src/rpcCapabilities.js';
import {
  assertBoingRpcEnvironment,
  BoingRpcPreflightError,
  isBoingRpcPreflightError,
} from '../src/preflightGate.js';

function stubPreflight(partial: Partial<BoingRpcPreflightResult>): BoingRpcPreflightResult {
  return {
    health: {
      ok: true,
      client_version: 'boing-node/0.0.0',
      chain_id: null,
      chain_name: null,
      head_height: 0,
    },
    supportedMethodCount: 1,
    catalogMethodCount: null,
    openApiPresent: false,
    httpLiveOk: true,
    httpReadyOk: true,
    jsonrpcBatchOk: true,
    httpOpenApiJsonOk: true,
    wellKnownBoingRpcOk: true,
    httpLiveJsonOk: true,
    ...partial,
  };
}

function stubProbe(): BoingRpcProbeBundle {
  const ok = { available: true as const };
  return {
    clientVersion: 'boing-node/0.0.0',
    supportedMethods: ['boing_chainHeight'],
    methods: {
      boing_chainHeight: ok,
      boing_getSyncState: ok,
      boing_getBlockByHeight: ok,
      boing_getLogs: ok,
      boing_getTransactionReceipt: ok,
      boing_getNetworkInfo: ok,
    },
  };
}

const doctorSpy = vi.spyOn(rpcDoctor, 'doctorBoingRpcEnvironment');

describe('preflightGate', () => {
  afterAll(() => {
    doctorSpy.mockRestore();
  });

  beforeEach(() => {
    doctorSpy.mockReset();
  });

  it('returns doctor result when ok', async () => {
    const doctorOk: BoingRpcDoctorResult = {
      ok: true,
      preflight: stubPreflight({}),
      capabilityProbe: stubProbe(),
      missingRequiredMethods: [],
      messages: [],
    };
    doctorSpy.mockResolvedValue(doctorOk);

    const client = {} as BoingClient;
    const r = await assertBoingRpcEnvironment(client);
    expect(r).toBe(doctorOk);
    expect(doctorSpy).toHaveBeenCalledWith(client, undefined);
  });

  it('throws BoingRpcPreflightError when doctor fails', async () => {
    const doctorBad: BoingRpcDoctorResult = {
      ok: false,
      preflight: stubPreflight({ httpLiveOk: false }),
      capabilityProbe: stubProbe(),
      missingRequiredMethods: [],
      messages: ['GET /live did not return HTTP 200.'],
    };
    doctorSpy.mockResolvedValue(doctorBad);

    await expect(assertBoingRpcEnvironment({} as BoingClient)).rejects.toThrow(BoingRpcPreflightError);

    try {
      await assertBoingRpcEnvironment({} as BoingClient);
    } catch (e) {
      expect(isBoingRpcPreflightError(e)).toBe(true);
      if (isBoingRpcPreflightError(e)) {
        expect(e.doctor.messages.some((m) => m.includes('/live'))).toBe(true);
      }
    }
  });
});
