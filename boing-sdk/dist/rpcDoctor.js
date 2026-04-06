/**
 * One-shot environment check: preflight HTTP discovery + capability probes + optional required methods.
 */
import { explainBoingRpcError } from './errors.js';
import { explainBoingRpcProbeGaps, probeBoingRpcCapabilities } from './rpcCapabilities.js';
/**
 * Run {@link BoingClient.preflightRpc}, {@link probeBoingRpcCapabilities}, and optional required-method checks.
 * `messages` collects short strings suitable for logs or UI (includes `explainBoingRpcProbeGaps` when probes fail).
 */
export async function doctorBoingRpcEnvironment(client, options) {
    const preflight = await client.preflightRpc();
    const capabilityProbe = await probeBoingRpcCapabilities(client);
    const messages = [];
    if (!preflight.httpLiveOk)
        messages.push('GET /live did not return HTTP 200.');
    if (!preflight.httpReadyOk)
        messages.push('GET /ready did not return HTTP 200 (orchestrator may hold traffic).');
    if (!preflight.jsonrpcBatchOk)
        messages.push('JSON-RPC batch probe failed (expect current boing-node).');
    if (!preflight.httpOpenApiJsonOk)
        messages.push('GET /openapi.json missing or not OpenAPI JSON (upgrade boing-node for HTTP discovery).');
    if (!preflight.wellKnownBoingRpcOk)
        messages.push('GET /.well-known/boing-rpc missing (optional HTTP discovery).');
    if (!preflight.httpLiveJsonOk)
        messages.push('GET /live.json did not return { ok: true } (optional JSON probes).');
    const probeExplain = explainBoingRpcProbeGaps(capabilityProbe);
    if (probeExplain)
        messages.push(probeExplain);
    const supported = new Set(capabilityProbe.supportedMethods ?? []);
    const required = options?.requiredMethods?.filter(Boolean) ?? [];
    const missingRequiredMethods = required.filter((m) => !supported.has(m));
    for (const m of missingRequiredMethods) {
        messages.push(`Required RPC method not advertised: ${m}`);
    }
    const ok = preflight.httpLiveOk &&
        preflight.httpReadyOk &&
        preflight.jsonrpcBatchOk &&
        missingRequiredMethods.length === 0 &&
        probeExplain == null;
    return {
        ok,
        preflight,
        capabilityProbe,
        missingRequiredMethods,
        messages,
    };
}
/** Format {@link BoingRpcDoctorResult} as a multi-line string for stdout. */
export function formatBoingRpcDoctorReport(result) {
    const lines = [
        `ok: ${result.ok}`,
        `methods_supported: ${result.preflight.supportedMethodCount}`,
        `http_openapi_json: ${result.preflight.httpOpenApiJsonOk}`,
        `well_known: ${result.preflight.wellKnownBoingRpcOk}`,
        `live_json: ${result.preflight.httpLiveJsonOk}`,
    ];
    if (result.messages.length > 0) {
        lines.push('---');
        lines.push(...result.messages);
    }
    return lines.join('\n');
}
/** Map arbitrary errors to a short doctor message (uses {@link explainBoingRpcError} for `BoingRpcError`). */
export function doctorErrorMessage(e) {
    return explainBoingRpcError(e);
}
