/**
 * Open a URL in the system browser. In Tauri, shell.open is scoped to Boing
 * hosts (+ localhost for staging); reject anything else before invoking.
 */

const ALLOWED =
  /^https:\/\/([a-z0-9-]+\.)*boing\.(network|express|observer|finance)(\/.*)?$/i;
const LOCAL = /^https?:\/\/(localhost|127\.0\.0\.1)(:\d+)?(\/.*)?$/i;

export function isAllowedExternalUrl(url: string): boolean {
  try {
    const u = new URL(url);
    if (u.protocol !== "https:" && u.protocol !== "http:") return false;
    const href = u.href;
    return ALLOWED.test(href) || LOCAL.test(href);
  } catch {
    return false;
  }
}

export async function openExternal(url: string): Promise<void> {
  if (!isAllowedExternalUrl(url)) {
    throw new Error(`Refusing to open URL outside the hub allowlist: ${url}`);
  }
  try {
    const { open } = await import("@tauri-apps/plugin-shell");
    await open(url);
  } catch {
    window.open(url, "_blank", "noopener,noreferrer");
  }
}
