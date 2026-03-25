import { BoingLoaderDots } from "./BoingLoaderDots";

type Props = {
  phase: "downloading" | "installing";
  version: string | null;
};

/**
 * Full-screen overlay during update download/install only (dice.express DesktopUpdateOverlay pattern).
 * “Checking” stays on the main splash canvas, not here.
 */
export default function SplashDesktopUpdateOverlay({ phase, version }: Props) {
  const label =
    phase === "downloading"
      ? version
        ? `Downloading v${version}…`
        : "Downloading update…"
      : "Installing… The app will be restarting in a moment.";

  return (
    <div className="desktop-update-overlay" aria-live="polite" aria-busy="true">
      <div className="desktop-update-overlay__card">
        <div className="desktop-update-overlay__symbol" aria-hidden>
          <img src="/favicon.svg" alt="" width={56} height={56} />
        </div>
        <p className="desktop-update-overlay__name">Boing Network Hub</p>
        <div className="desktop-update-overlay__loader">
          <BoingLoaderDots size="sm" />
        </div>
        <p className="desktop-update-overlay__message">{label}</p>
      </div>
    </div>
  );
}
