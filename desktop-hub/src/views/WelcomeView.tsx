import { useCallback } from "react";
import { HubEngraveBackground } from "../components/HubEngraveBackground";
import { openExternal } from "../lib/openExternal";

const TESTNET_URL = "https://boing.network/testnet";
const EXPRESS_URL = "https://boing.express";

type Props = {
  onOpenWallet: () => void;
  onRegister: () => void;
  onContinueAsGuest: () => void;
};

/**
 * Entry chooser — opening Express or the testnet site is convenience navigation,
 * not hub authentication (audit #61 item 61).
 */
export function WelcomeView({ onOpenWallet, onRegister, onContinueAsGuest }: Props) {
  const handleOpenWallet = useCallback(() => {
    void openExternal(EXPRESS_URL).catch(() => {
      /* ignore */
    });
    onOpenWallet();
  }, [onOpenWallet]);

  const handleRegister = useCallback(() => {
    void openExternal(TESTNET_URL).catch(() => {
      /* ignore */
    });
    onRegister();
  }, [onRegister]);

  return (
    <div className="welcome-view" role="main" aria-label="Welcome">
      <HubEngraveBackground />
      <div className="welcome-view__card">
        <img src="/favicon.svg" alt="" className="welcome-view__logo" />
        <h1 className="welcome-view__title">Welcome to Boing Network Hub</h1>
        <p className="welcome-view__subtitle">
          Open the Express wallet or testnet site in your browser, or continue here to explore
          Observer, Finance, and the QA operator tools. The hub does not authenticate accounts.
        </p>
        <div className="welcome-view__actions">
          <button
            type="button"
            className="welcome-view__btn welcome-view__btn--primary"
            onClick={handleOpenWallet}
          >
            Open wallet
          </button>
          <button type="button" className="welcome-view__btn" onClick={handleRegister}>
            Register on testnet
          </button>
          <button
            type="button"
            className="welcome-view__btn welcome-view__btn--secondary"
            onClick={onContinueAsGuest}
          >
            Continue exploring
          </button>
        </div>
      </div>
    </div>
  );
}
