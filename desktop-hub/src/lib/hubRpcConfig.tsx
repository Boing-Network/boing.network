import { createContext, useCallback, useContext, useMemo, useState, type ReactNode } from "react";

/** Shared with QA operator view and Settings → RPC diagnostics. */
export const HUB_LS_RPC_URL = "boing-hub-qa-rpc-url";
export const HUB_LS_OPERATOR_TOKEN = "boing-hub-qa-operator-token";

function loadStored(key: string, fallback: string): string {
  try {
    const v = localStorage.getItem(key);
    return v != null && v !== "" ? v : fallback;
  } catch {
    return fallback;
  }
}

export type HubRpcConfig = {
  rpcUrl: string;
  setRpcUrl: (value: string) => void;
  operatorToken: string;
  setOperatorToken: (value: string) => void;
};

const HubRpcConfigContext = createContext<HubRpcConfig | null>(null);

export function HubRpcConfigProvider({ children }: { children: ReactNode }): React.ReactElement {
  const [rpcUrl, setRpcUrlState] = useState(() => loadStored(HUB_LS_RPC_URL, "http://127.0.0.1:8545"));
  const [operatorToken, setOperatorTokenState] = useState(() => loadStored(HUB_LS_OPERATOR_TOKEN, ""));

  const setRpcUrl = useCallback((value: string) => {
    setRpcUrlState(value);
    try {
      localStorage.setItem(HUB_LS_RPC_URL, value.trim());
    } catch {
      /* ignore */
    }
  }, []);

  const setOperatorToken = useCallback((value: string) => {
    setOperatorTokenState(value);
    try {
      localStorage.setItem(HUB_LS_OPERATOR_TOKEN, value);
    } catch {
      /* ignore */
    }
  }, []);

  const value = useMemo(
    () => ({ rpcUrl, setRpcUrl, operatorToken, setOperatorToken }),
    [rpcUrl, setRpcUrl, operatorToken, setOperatorToken],
  );

  return <HubRpcConfigContext.Provider value={value}>{children}</HubRpcConfigContext.Provider>;
}

export function useHubRpcConfig(): HubRpcConfig {
  const ctx = useContext(HubRpcConfigContext);
  if (ctx == null) {
    throw new Error("useHubRpcConfig must be used within HubRpcConfigProvider");
  }
  return ctx;
}
