/**
 * Capture a JSON-serializable snapshot of RPC endpoint identity + capabilities (CI / local dev pins).
 */
import type { BoingClient } from './client.js';
import type { BoingNetworkProfile } from './types.js';
/** Fetch health, optional network info, supported methods, and {@link BoingClient.preflightRpc}. */
export declare function captureBoingNetworkProfile(client: BoingClient): Promise<BoingNetworkProfile>;
//# sourceMappingURL=networkProfile.d.ts.map