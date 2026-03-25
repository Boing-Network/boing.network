import { useState, useEffect, useCallback, useRef } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { HUB_APP_URLS, type HubView } from "./config";
import {
  STORAGE_KEY_VIEW,
  STORAGE_KEY_LAST_APP,
  getWelcomeDismissed,
  setWelcomeDismissed,
  clearWelcomeDismissed,
  getSignedIn,
  setSignedIn,
  clearSignedIn,
  getShowIntro,
  setShowIntro,
} from "./lib/storage";
import { AppIcon } from "./components/AppIcons";
import { IntroView } from "./components/IntroView";
import { UpdateOverlay } from "./components/UpdateOverlay";
import { HubFooter } from "./components/HubFooter";
import { useUpdateCheck } from "./hooks/useUpdateCheck";
import { isTauri } from "./lib/tauri";
import { HomeView } from "./views/HomeView";
import { EmbedView } from "./views/EmbedView";
import { WelcomeView } from "./views/WelcomeView";
import "./styles.css";

const NAV_ITEMS: { id: HubView; label: string; description: string }[] = [
  { id: "home", label: "Home", description: "Boing Network overview" },
  { id: "observer", label: "Observer", description: "Block explorer" },
  { id: "express", label: "Wallet", description: "Same wallet as the Chrome extension — send, stake, connect dApps" },
  { id: "finance", label: "Finance", description: "DEX & DeFi" },
  { id: "network", label: "Testnet", description: "Testnet ecosystem — register, faucet, quests, developers" },
];

function isValidView(v: string): v is HubView {
  return ["home", "observer", "express", "finance", "network"].includes(v);
}

const WINDOW_TITLE_BASE = "Boing Network Hub";

type AppPhase = "intro" | "welcome" | "app";

type EntryMode = "guest" | "signin" | "register";

/**
 * When coming from splash (at /app), update check already ran there — skip "updating".
 * Goes straight to intro (if enabled), welcome, or app.
 */
function getInitialPhase(): AppPhase {
  return getShowIntro() ? "intro" : getWelcomeDismissed() ? "app" : "welcome";
}

function App() {
  const [phase, setPhase] = useState<AppPhase>(getInitialPhase);
  const [view, setViewState] = useState<HubView>("home");
  const [lastEmbedView, setLastEmbedView] = useState<HubView | null>(null);
  const [signedIn, setSignedInState] = useState(getSignedIn);
  const [showIntroNextLaunch, setShowIntroNextLaunchState] = useState(getShowIntro);
  const mainRef = useRef<HTMLElement>(null);
  const { status: updateStatus, runCheck, clearStatus } = useUpdateCheck();

  const enterApp = useCallback((mode: EntryMode) => {
    setWelcomeDismissed();
    if (mode === "signin" || mode === "register") {
      setSignedIn();
      setSignedInState(true);
    }
    setPhase("app");
  }, []);

  const handleSignOut = useCallback(() => {
    clearWelcomeDismissed();
    clearSignedIn();
    setSignedInState(false);
    setPhase("welcome");
  }, []);

  const handleShowWelcome = useCallback(() => {
    clearWelcomeDismissed();
    clearSignedIn();
    setSignedInState(false);
    setPhase("welcome");
  }, []);

  const handleShowIntroNextLaunchChange = useCallback((show: boolean) => {
    setShowIntro(show);
    setShowIntroNextLaunchState(show);
  }, []);

  const dismissUpdateFeedback = useCallback(() => {
    clearStatus();
  }, [clearStatus]);

  const handleCheckForUpdates = useCallback(() => {
    void runCheck({ persistError: true, notifyUpToDate: true }).then((result) => {
      if (result === "proceed") clearStatus();
    });
  }, [runCheck, clearStatus]);

  const handleIntroComplete = useCallback((skipIntroNextTime: boolean) => {
    if (skipIntroNextTime) {
      setShowIntro(false);
      setShowIntroNextLaunchState(false);
    }
    setPhase(getWelcomeDismissed() ? "app" : "welcome");
  }, []);

  useEffect(() => {
    if (phase !== "app") return;
    setSignedInState(getSignedIn());
  }, [phase]);

  useEffect(() => {
    if (phase !== "app") return;
    try {
      const stored = localStorage.getItem(STORAGE_KEY_VIEW);
      if (stored && isValidView(stored)) {
        setViewState(stored);
        if (stored !== "home") setLastEmbedView(stored);
      }
      const lastApp = localStorage.getItem(STORAGE_KEY_LAST_APP);
      if (lastApp && isValidView(lastApp) && lastApp !== "home") setLastEmbedView(lastApp);
    } catch {
      /* ignore */
    }
  }, [phase]);

  const setView = useCallback((next: HubView) => {
    setViewState(next);
    if (next !== "home") setLastEmbedView(next);
    try {
      localStorage.setItem(STORAGE_KEY_VIEW, next);
      if (next !== "home") localStorage.setItem(STORAGE_KEY_LAST_APP, next);
    } catch {
      /* ignore */
    }
    mainRef.current?.focus({ preventScroll: true });
  }, []);

  useEffect(() => {
    if (phase !== "app") return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setView("home");
        e.preventDefault();
        return;
      }
      if (e.altKey && e.key >= "1" && e.key <= "5") {
        const i = e.key === "5" ? 4 : Number(e.key) - 1;
        const item = NAV_ITEMS[i];
        if (item) {
          e.preventDefault();
          setView(item.id);
        }
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [phase, setView]);

  useEffect(() => {
    if (phase !== "app") return;
    mainRef.current?.focus({ preventScroll: true });
  }, [phase]);

  useEffect(() => {
    if (phase !== "app") return;
    const label = view === "home" ? "Home" : NAV_ITEMS.find((n) => n.id === view)?.label ?? view;
    const title = view === "home" ? WINDOW_TITLE_BASE : `${label} — ${WINDOW_TITLE_BASE}`;
    document.title = title;
    if (isTauri()) {
      void getCurrentWindow().setTitle(title).catch(() => {});
    }
  }, [phase, view]);

  const isEmbedView =
    view === "observer" || view === "express" || view === "finance" || view === "network";
  const lastUsedAppId: HubView | null = lastEmbedView;

  if (phase === "intro") {
    return <IntroView onComplete={handleIntroComplete} />;
  }

  if (phase === "welcome") {
    return (
      <>
        <UpdateOverlay
          status={updateStatus}
          onDismissError={dismissUpdateFeedback}
          onRetryError={() => void runCheck({ persistError: true })}
        />
        <WelcomeView
          onSignIn={() => enterApp("signin")}
          onRegister={() => enterApp("register")}
          onContinueAsGuest={() => enterApp("guest")}
        />
      </>
    );
  }

  return (
    <div className="hub">
      <UpdateOverlay
        status={updateStatus}
        onDismissError={dismissUpdateFeedback}
        onRetryError={() => void runCheck({ persistError: true })}
      />
      <a href="#main-content" className="skip-link">
        Skip to main content
      </a>
      <aside className="hub-sidebar">
        <div
          className="hub-brand"
          role="button"
          tabIndex={0}
          onClick={() => setView("home")}
          onKeyDown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              setView("home");
            }
          }}
          aria-label="Boing Network — go to Home"
        >
          <img src="/favicon.svg" alt="" className="hub-logo" />
          <span className="hub-title">Boing Network</span>
        </div>
        <nav className="hub-nav" aria-label="Main navigation">
          {NAV_ITEMS.map((item, index) => (
            <button
              key={item.id}
              type="button"
              className={`hub-nav-item ${view === item.id ? "active" : ""}`}
              onClick={() => setView(item.id)}
              title={`${item.description} (Alt+${index + 1})`}
              aria-current={view === item.id ? "page" : undefined}
            >
              <span className="hub-nav-icon">
                <AppIcon appId={item.id} size={18} />
              </span>
              <span className="hub-nav-label">{item.label}</span>
            </button>
          ))}
        </nav>
        <HubFooter
          signedIn={signedIn}
          onSignOut={handleSignOut}
          onShowWelcome={handleShowWelcome}
          showIntroNextLaunch={showIntroNextLaunch}
          onShowIntroNextLaunchChange={handleShowIntroNextLaunchChange}
          onCheckForUpdates={handleCheckForUpdates}
        />
      </aside>
      <main
        id="main-content"
        ref={mainRef}
        className="hub-main"
        tabIndex={-1}
        role="main"
        aria-label={view === "home" ? "Home" : isEmbedView ? NAV_ITEMS.find((n) => n.id === view)?.label ?? "App" : undefined}
      >
        {view === "home" && <HomeView onNavigate={setView} lastUsedAppId={lastUsedAppId} />}
        {isEmbedView && (
          <EmbedView
            appId={view}
            url={HUB_APP_URLS[view]}
            title={NAV_ITEMS.find((n) => n.id === view)?.label ?? view}
          />
        )}
      </main>
    </div>
  );
}

export default App;
