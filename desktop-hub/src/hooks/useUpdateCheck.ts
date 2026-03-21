import { useState, useCallback, useRef, useEffect } from "react";
import type { UpdateStatus } from "../components/UpdateOverlay";

const isTauri = typeof window !== "undefined" && typeof (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ !== "undefined";

const CHECK_TIMEOUT_MS = 22_000;

export type UpdateCheckOptions = {
  /** Keep an error state until dismissed (e.g. Settings → Check for updates). */
  persistError?: boolean;
  /** Show a short confirmation when no update is available (manual checks). */
  notifyUpToDate?: boolean;
};

export type UpdateCheckResult = "restarting" | "proceed" | "error" | "uptodate";

function formatUpdateError(err: unknown): string {
  const raw = err instanceof Error ? err.message : String(err);
  if (/timed?\s*out|timeout/i.test(raw)) {
    return "Update check timed out. Check your connection and try again.";
  }
  if (/network|fetch|dns|getaddrinfo|connection refused|econnrefused/i.test(raw)) {
    return "Couldn't reach the update server. Check your connection.";
  }
  if (/signature|verification|pubkey|invalid key/i.test(raw)) {
    return "Update could not be verified. Try again later or reinstall from boing.network/downloads.";
  }
  return raw.length > 140 ? `${raw.slice(0, 137)}…` : raw;
}

export function useUpdateCheck() {
  const [status, setStatus] = useState<UpdateStatus>({ phase: "idle" });
  const upToDateTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (upToDateTimerRef.current != null) clearTimeout(upToDateTimerRef.current);
    };
  }, []);

  const runCheck = useCallback(async (options?: UpdateCheckOptions): Promise<UpdateCheckResult> => {
    if (!isTauri) return "proceed";

    if (upToDateTimerRef.current != null) {
      clearTimeout(upToDateTimerRef.current);
      upToDateTimerRef.current = null;
    }

    setStatus({ phase: "checking" });
    try {
      const { check } = await import("@tauri-apps/plugin-updater");
      const update = await check({ timeout: CHECK_TIMEOUT_MS });

      if (update == null) {
        if (options?.notifyUpToDate) {
          setStatus({ phase: "uptodate" });
          upToDateTimerRef.current = setTimeout(() => {
            upToDateTimerRef.current = null;
            setStatus({ phase: "idle" });
          }, 2800);
          return "uptodate";
        }
        setStatus({ phase: "ready" });
        return "proceed";
      }

      setStatus({ phase: "downloading", percent: 0 });
      let contentLength = 0;
      let downloaded = 0;

      await update.downloadAndInstall((event) => {
        if (event.event === "Started" && event.data?.contentLength != null) {
          contentLength = event.data.contentLength;
        } else if (event.event === "Progress" && event.data?.chunkLength != null) {
          downloaded += event.data.chunkLength;
          const percent = contentLength > 0 ? Math.min(100, Math.round((downloaded / contentLength) * 100)) : 0;
          setStatus({ phase: "downloading", percent, detail: `${percent}%` });
        } else if (event.event === "Finished") {
          setStatus({ phase: "installing" });
        }
      });

      try {
        const { relaunch } = await import("@tauri-apps/plugin-process");
        await relaunch();
      } catch {
        /* process plugin not installed; installer may restart the app */
      }
      return "restarting";
    } catch (err) {
      const message = formatUpdateError(err);
      if (options?.persistError) {
        setStatus({ phase: "error", message });
        return "error";
      }
      return "proceed";
    }
  }, []);

  const clearStatus = useCallback(() => {
    if (upToDateTimerRef.current != null) {
      clearTimeout(upToDateTimerRef.current);
      upToDateTimerRef.current = null;
    }
    setStatus({ phase: "idle" });
  }, []);

  return { status, runCheck, clearStatus };
}
