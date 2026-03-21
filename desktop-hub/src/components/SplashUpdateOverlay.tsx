type Props = {
  phase: "downloading" | "installing";
  version: string | null;
};

/**
 * Full-screen overlay during update download/install in the splash window.
 * Matches dice.express / vibeminer: centered card with spinner and message.
 */
export default function SplashUpdateOverlay({ phase, version }: Props) {
  const label =
    phase === "downloading"
      ? version
        ? `Downloading v${version}…`
        : "Downloading update…"
      : "Installing… The app will be restarting in a moment.";

  return (
    <div
      className="splash-update-overlay"
      role="status"
      aria-live="polite"
      aria-busy="true"
    >
      <div className="splash-update-overlay__card">
        <div className="splash-update-overlay__symbol" aria-hidden>
          <img src="/favicon.svg" alt="" width={56} height={56} />
        </div>
        <p className="splash-update-overlay__name">Boing Network Hub</p>
        <div className="splash-update-overlay__spinner" aria-hidden />
        <p className="splash-update-overlay__message">{label}</p>
      </div>
    </div>
  );
}
