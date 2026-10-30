import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { isTauri } from "./tauri";

/** Shared with QA operator view and Settings → RPC diagnostics. */
export const HUB_LS_RPC_URL = "boing-hub-qa-rpc-url";
/** Legacy plaintext key — cleared after migration to OS keychain (item 61). */
export const HUB_LS_OPERATOR_TOKEN = "boing-hub-qa-operator-token";

function loadStored(key: string, fallback: string): string {
  try {
    const v = localStorage.getItem(key);
    return v != null && v !== "" ? v : fallback;
  } catch {
    return fallback;
  }
}

async function loadOperatorToken(): Promise<string> {
  if (isTauri()) {
    try {
      const fromKeychain = await invoke<string>("get_operator_token");
      if (fromKeychain) return fromKeychain;
    } catch {
      /* fall through to migrate legacy localStorage */
    }
  }
  const legacy = loadStored(HUB_LS_OPERATOR_TOKEN, "");
  if (legacy && isTauri()) {
    try {
      await invoke("set_operator_token", { token: legacy });
      try {
        localStorage.removeItem(HUB_LS_OPERATOR_TOKEN);
      } catch {
        /* ignore */
      }
    } catch {
      /* keep legacy in memory for this session if keychain write fails */
    }
  }
  return legacy;
}

async function persistOperatorToken(value: string): Promise<void> {
  if (isTauri()) {
    await invoke("set_operator_token", { token: value });
    try {
      localStorage.removeItem(HUB_LS_OPERATOR_TOKEN);
    } catch {
      /* ignore */
    }
    return;
  }
  // Browser / vite preview: sessionStorage only (never durable plaintext on disk via localStorage).
  try {
    if (value) sessionStorage.setItem(HUB_LS_OPERATOR_TOKEN, value);
    else sessionStorage.removeItem(HUB_LS_OPERATOR_TOKEN);
    localStorage.removeItem(HUB_LS_OPERATOR_TOKEN);
  } catch {
    /* ignore */
  }
}

export type HubRpcConfig = {
  rpcUrl: string;
  setRpcUrl: (value: string) => void;
  operatorToken: string;
  setOperatorToken: (value: string) => void;
  operatorTokenReady: boolean;
};

const HubRpcConfigContext = createContext<HubRpcConfig | null>(null);

export function HubRpcConfigProvider({ children }: { children: ReactNode }): React.ReactElement {
  const [rpcUrl, setRpcUrlState] = useState(() => loadStored(HUB_LS_RPC_URL, "http://127.0.0.1:8545"));
  const [operatorToken, setOperatorTokenState] = useState("");
  const [operatorTokenReady, setOperatorTokenReady] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      const token = await loadOperatorToken();
      if (!cancelled) {
        setOperatorTokenState(token);
        setOperatorTokenReady(true);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

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
    void persistOperatorToken(value).catch(() => {
      /* ignore persistence errors; in-memory value still used */
    });
  }, []);

  const value = useMemo(
    () => ({ rpcUrl, setRpcUrl, operatorToken, setOperatorToken, operatorTokenReady }),
    [rpcUrl, setRpcUrl, operatorToken, setOperatorToken, operatorTokenReady],
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
