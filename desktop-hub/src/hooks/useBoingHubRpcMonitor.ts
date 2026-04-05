import { useEffect, useState } from "react";
import { BoingConnectionMonitor, type BoingClient, type BoingConnectionSnapshot } from "boing-sdk";

export interface UseBoingHubRpcMonitorOptions {
  /** Default **20_000** ms. */
  pollIntervalMs?: number;
  /** When false, no polling (default **true**). */
  enabled?: boolean;
}

/**
 * Polls **`boing_health`** and **`boing_getNetworkInfo`** for Hub / operator screens (status line, support ID).
 */
export function useBoingHubRpcMonitor(
  client: BoingClient,
  options: UseBoingHubRpcMonitorOptions = {},
): BoingConnectionSnapshot | null {
  const { pollIntervalMs = 20_000, enabled = true } = options;
  const [snapshot, setSnapshot] = useState<BoingConnectionSnapshot | null>(null);

  useEffect(() => {
    if (!enabled) {
      setSnapshot(null);
      return;
    }

    const mon = new BoingConnectionMonitor(client, {
      pollIntervalMs,
      includeNetworkInfo: true,
      onUpdate: setSnapshot,
    });
    mon.start();
    return () => {
      mon.stop();
    };
  }, [client, enabled, pollIntervalMs]);

  return snapshot;
}
