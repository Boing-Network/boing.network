/**
 * Capture a JSON-serializable snapshot of RPC endpoint identity + capabilities (CI / local dev pins).
 */
/** Fetch health, optional network info, supported methods, and {@link BoingClient.preflightRpc}. */
export async function captureBoingNetworkProfile(client) {
    const preflight = await client.preflightRpc();
    let network_info = null;
    try {
        network_info = await client.getNetworkInfo();
    }
    catch {
        network_info = null;
    }
    let supported_methods = null;
    try {
        supported_methods = await client.rpcSupportedMethods();
    }
    catch {
        supported_methods = null;
    }
    return {
        captured_at_ms: Date.now(),
        base_url: client.getBaseUrl(),
        health: preflight.health,
        network_info,
        supported_methods,
        preflight,
    };
}
