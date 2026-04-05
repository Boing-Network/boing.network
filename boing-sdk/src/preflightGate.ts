/**
 * Fail fast before heavy flows (swap UI, indexer tick) when the RPC environment is unhealthy.
 */

import type { BoingClient } from './client.js';
import { doctorBoingRpcEnvironment, type BoingRpcDoctorOptions, type BoingRpcDoctorResult } from './rpcDoctor.js';

export class BoingRpcPreflightError extends Error {
  readonly doctor: BoingRpcDoctorResult;

  constructor(doctor: BoingRpcDoctorResult) {
    const msg =
      doctor.messages.length > 0
        ? doctor.messages.join('\n')
        : 'RPC environment check failed (see BoingRpcPreflightError.doctor).';
    super(msg);
    this.name = 'BoingRpcPreflightError';
    this.doctor = doctor;
    Object.setPrototypeOf(this, BoingRpcPreflightError.prototype);
  }
}

export function isBoingRpcPreflightError(e: unknown): e is BoingRpcPreflightError {
  return e instanceof BoingRpcPreflightError;
}

/**
 * Runs {@link doctorBoingRpcEnvironment}; throws {@link BoingRpcPreflightError} when **`ok`** is false.
 * Use in dApp “connect” or before starting polling loops.
 */
export async function assertBoingRpcEnvironment(
  client: BoingClient,
  options?: BoingRpcDoctorOptions,
): Promise<BoingRpcDoctorResult> {
  const doctor = await doctorBoingRpcEnvironment(client, options);
  if (!doctor.ok) {
    throw new BoingRpcPreflightError(doctor);
  }
  return doctor;
}
