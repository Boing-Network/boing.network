/**
 * One-shot environment check: preflight HTTP discovery + capability probes + optional required methods.
 */
import type { BoingClient } from './client.js';
import type { BoingRpcPreflightResult } from './types.js';
import { type BoingRpcProbeBundle } from './rpcCapabilities.js';
export interface BoingRpcDoctorOptions {
    /** If set, `ok` is false when any of these are missing from `boing_rpcSupportedMethods`. */
    requiredMethods?: string[];
}
export interface BoingRpcDoctorResult {
    ok: boolean;
    preflight: BoingRpcPreflightResult;
    capabilityProbe: BoingRpcProbeBundle;
    missingRequiredMethods: string[];
    messages: string[];
}
/**
 * Run {@link BoingClient.preflightRpc}, {@link probeBoingRpcCapabilities}, and optional required-method checks.
 * `messages` collects short strings suitable for logs or UI (includes `explainBoingRpcProbeGaps` when probes fail).
 */
export declare function doctorBoingRpcEnvironment(client: BoingClient, options?: BoingRpcDoctorOptions): Promise<BoingRpcDoctorResult>;
/** Format {@link BoingRpcDoctorResult} as a multi-line string for stdout. */
export declare function formatBoingRpcDoctorReport(result: BoingRpcDoctorResult): string;
/** Map arbitrary errors to a short doctor message (uses {@link explainBoingRpcError} for `BoingRpcError`). */
export declare function doctorErrorMessage(e: unknown): string;
//# sourceMappingURL=rpcDoctor.d.ts.map