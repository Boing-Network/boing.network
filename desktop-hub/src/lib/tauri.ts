import { isTauri as isTauriRuntime } from "@tauri-apps/api/core";

/** True when running inside the Tauri webview (desktop). Uses the official runtime flag, not ad-hoc globals. */
export function isTauri(): boolean {
  return isTauriRuntime();
}

/**
 * Windows desktop webview (WebView2). Used for update flow: on Windows the updater plugin spawns
 * NSIS and terminates the process; JS `relaunch()` never runs after a successful in-place update.
 */
export function isWindowsWebview(): boolean {
  if (typeof navigator === "undefined") return false;
  return /Windows/i.test(navigator.userAgent);
}
