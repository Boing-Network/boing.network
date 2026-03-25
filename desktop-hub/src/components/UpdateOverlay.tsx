import { BoingLoaderDots } from "./BoingLoaderDots";

export type UpdateStatus =
  | { phase: "idle" }
  | { phase: "checking" }
  | { phase: "ready" }
  | { phase: "downloading"; percent: number; version?: string; detail?: string }
  | { phase: "installing"; version?: string }
  | { phase: "error"; message: string }
  | { phase: "uptodate" };

type Props = {
  status: UpdateStatus;
  /** When true, show the card with "Checking for updates…" when status is idle (e.g. on updating screen). */
  showCheckingWhenIdle?: boolean;
  /** Clears persisted update error (Settings → Check for updates). */
  onDismissError?: () => void;
  /** BountyHub-style retry after a failed check/download. */
  onRetryError?: () => void;
};

/** dice.express / BountyHub-style overlay: dark blur, bordered card, loader + phase label. */
export function UpdateOverlay({ status, showCheckingWhenIdle, onDismissError, onRetryError }: Props) {
  if (status.phase === "uptodate") {
    return (
      <div className="update-toast update-toast--success" role="status" aria-live="polite">
        <p className="update-toast__message">You’re on the latest version.</p>
      </div>
    );
  }

  if (status.phase === "error" && onDismissError) {
    return (
      <div className="desktop-update-overlay desktop-update-overlay--inline" role="alert" aria-live="assertive">
        <div className="desktop-update-overlay__card desktop-update-overlay__card--error">
          <div className="desktop-update-overlay__error-icon" aria-hidden>
            <svg xmlns="http://www.w3.org/2000/svg" width="22" height="22" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
            </svg>
          </div>
          <p className="desktop-update-overlay__error-title">Update failed</p>
          <p className="desktop-update-overlay__error-detail">{status.message}</p>
          <div className="desktop-update-overlay__actions">
            <button type="button" className="desktop-update-overlay__btn desktop-update-overlay__btn--secondary" onClick={onDismissError}>
              Continue
            </button>
            {onRetryError ? (
              <button type="button" className="desktop-update-overlay__btn desktop-update-overlay__btn--primary" onClick={onRetryError}>
                Retry
              </button>
            ) : null}
          </div>
        </div>
      </div>
    );
  }

  if (status.phase === "ready" || status.phase === "error") {
    return null;
  }

  const showCard =
    status.phase === "checking" ||
    status.phase === "downloading" ||
    status.phase === "installing" ||
    (showCheckingWhenIdle && status.phase === "idle");
  if (!showCard) {
    return null;
  }

  const version = status.phase === "downloading" || status.phase === "installing" ? status.version : undefined;
  const label =
    status.phase === "idle" || status.phase === "checking"
      ? "Checking for updates…"
      : status.phase === "downloading"
        ? version
          ? `Downloading v${version}…`
          : "Downloading update…"
        : status.phase === "installing"
          ? "Installing… The app will be restarting in a moment."
          : "Preparing…";

  return (
    <div className="desktop-update-overlay desktop-update-overlay--inline" role="status" aria-live="polite" aria-busy="true" aria-label={label}>
      <div className="desktop-update-overlay__card">
        <div className="desktop-update-overlay__symbol" aria-hidden>
          <img src="/favicon.svg" alt="" width={56} height={56} />
        </div>
        <p className="desktop-update-overlay__name">Boing Network Hub</p>
        <div className="desktop-update-overlay__loader">
          <BoingLoaderDots size="sm" />
        </div>
        <p className="desktop-update-overlay__message">{label}</p>
        {(status.phase === "downloading" || status.phase === "installing") && (
          <div className="hub-update-progress-wrap">
            <div
              className="hub-update-progress-bar"
              style={{
                width: status.phase === "downloading" ? `${status.percent}%` : "100%",
              }}
            />
          </div>
        )}
        {status.phase === "downloading" && status.detail !== undefined && (
          <p className="hub-update-progress-detail">{status.detail}</p>
        )}
      </div>
    </div>
  );
}
