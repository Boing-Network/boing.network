/**
 * Local storage keys and helpers for hub preferences (not authentication).
 */

export const STORAGE_KEY_VIEW = "boing-hub-last-view";
export const STORAGE_KEY_LAST_APP = "boing-hub-last-embed-view";
export const STORAGE_KEY_WELCOME_DISMISSED = "boing-hub-welcome-dismissed";
export const STORAGE_KEY_SHOW_INTRO = "boing-hub-show-intro";

/** Cleared on upgrade — former fake “signed in” flag (audit #61 item 61). */
const LEGACY_SIGNED_IN = "boing-hub-signed-in";

export function getWelcomeDismissed(): boolean {
  try {
    return localStorage.getItem(STORAGE_KEY_WELCOME_DISMISSED) === "1";
  } catch {
    return false;
  }
}

export function setWelcomeDismissed(): void {
  try {
    localStorage.setItem(STORAGE_KEY_WELCOME_DISMISSED, "1");
    localStorage.removeItem(LEGACY_SIGNED_IN);
  } catch {
    /* ignore */
  }
}

export function clearWelcomeDismissed(): void {
  try {
    localStorage.removeItem(STORAGE_KEY_WELCOME_DISMISSED);
    localStorage.removeItem(LEGACY_SIGNED_IN);
  } catch {
    /* ignore */
  }
}

/** Default true = show intro on next launch */
export function getShowIntro(): boolean {
  try {
    const v = localStorage.getItem(STORAGE_KEY_SHOW_INTRO);
    return v === null || v === "1";
  } catch {
    return true;
  }
}

export function setShowIntro(show: boolean): void {
  try {
    localStorage.setItem(STORAGE_KEY_SHOW_INTRO, show ? "1" : "0");
  } catch {
    /* ignore */
  }
}
