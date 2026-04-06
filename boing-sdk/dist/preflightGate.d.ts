/**
 * Fail fast before heavy flows (swap UI, indexer tick) when the RPC environment is unhealthy.
 */
import type { BoingClient } from './client.js';
import { type BoingRpcDoctorOptions, type BoingRpcDoctorResult } from './rpcDoctor.js';
export declare class BoingRpcPreflightError extends Error {
    readonly doctor: BoingRpcDoctorResult;
    constructor(doctor: BoingRpcDoctorResult);
}
export declare function isBoingRpcPreflightError(e: unknown): e is BoingRpcPreflightError;
/**
 * Runs {@link doctorBoingRpcEnvironment}; throws {@link BoingRpcPreflightError} when **`ok`** is false.
 * Use in dApp “connect” or before starting polling loops.
 */
export declare function assertBoingRpcEnvironment(client: BoingClient, options?: BoingRpcDoctorOptions): Promise<BoingRpcDoctorResult>;
//# sourceMappingURL=preflightGate.d.ts.map