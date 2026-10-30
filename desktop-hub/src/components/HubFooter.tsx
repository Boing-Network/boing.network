import { useState, useRef, useEffect } from "react";
import { APP_VERSION } from "../config";

type Props = {
  onShowWelcome: () => void;
  showIntroNextLaunch: boolean;
  onShowIntroNextLaunchChange: (show: boolean) => void;
  onCheckForUpdates?: () => void;
  onOpenRpcDiagnostics?: () => void;
};

export function HubFooter({
  onShowWelcome,
  showIntroNextLaunch,
  onShowIntroNextLaunchChange,
  onCheckForUpdates,
  onOpenRpcDiagnostics,
}: Props) {
  const [settingsOpen, setSettingsOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!settingsOpen) return;
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setSettingsOpen(false);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [settingsOpen]);

  const handleShowWelcome = () => {
    setSettingsOpen(false);
    onShowWelcome();
  };

  return (
    <div className="hub-footer">
      <span className="hub-footer-text">Alt+1–5 · Esc Home</span>
      <div className="hub-footer-links">
        <button
          type="button"
          className="hub-footer-link"
          onClick={onShowWelcome}
          aria-label="Show welcome screen"
        >
          Welcome
        </button>
        <div className="hub-footer-settings" ref={menuRef}>
          <button
            type="button"
            className="hub-footer-link hub-footer-settings-trigger"
            onClick={() => setSettingsOpen((o) => !o)}
            aria-expanded={settingsOpen}
            aria-haspopup="true"
            aria-label="Settings"
            title="Settings"
          >
            Settings
          </button>
          {settingsOpen && (
            <div className="hub-footer-settings-menu" role="menu">
              <button
                type="button"
                role="menuitem"
                className="hub-footer-settings-item"
                onClick={handleShowWelcome}
              >
                Show welcome on next launch
              </button>
              <label
                className="hub-footer-settings-item hub-footer-settings-item--checkbox"
                role="menuitemcheckbox"
                aria-checked={showIntroNextLaunch}
              >
                <input
                  type="checkbox"
                  checked={showIntroNextLaunch}
                  onChange={(e) => onShowIntroNextLaunchChange(e.target.checked)}
                  onClick={(e) => e.stopPropagation()}
                />
                <span>Show intro on next launch</span>
              </label>
              {onOpenRpcDiagnostics && (
                <button
                  type="button"
                  role="menuitem"
                  className="hub-footer-settings-item"
                  onClick={() => {
                    setSettingsOpen(false);
                    onOpenRpcDiagnostics();
                  }}
                >
                  RPC &amp; diagnostics
                </button>
              )}
              {onCheckForUpdates && (
                <button
                  type="button"
                  role="menuitem"
                  className="hub-footer-settings-item"
                  onClick={() => {
                    setSettingsOpen(false);
                    onCheckForUpdates();
                  }}
                >
                  Check for updates
                </button>
              )}
            </div>
          )}
        </div>
        <a
          className="hub-footer-version"
          href="https://boing.network"
          target="_blank"
          rel="noopener noreferrer"
          title="Boing Network"
        >
          v{APP_VERSION}
        </a>
      </div>
    </div>
  );
}
