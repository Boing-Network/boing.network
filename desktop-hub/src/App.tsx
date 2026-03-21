import { useState, useEffect, useCallback, useRef } from "react";
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

type AppPhase = "intro" | "updating" | "welcome" | "app";

type EntryMode = "guest" | "signin" | "register";

function getInitialPhase(): AppPhase {
  return getShowIntro() ? "intro" : "updating";
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

  const handleCheckForUpdates = useCallback(() => {
    runCheck().then((result) => {
      if (result === "proceed") clearStatus();
    });
  }, [runCheck, clearStatus]);

  const handleIntroComplete = useCallback(
    (skipIntroNextTime: boolean) => {
      if (skipIntroNextTime) {
        setShowIntro(false);
        setShowIntroNextLaunchState(false);
      }
      setPhase("updating");
      runCheck().then((result) => {
        clearStatus();
        if (result === "restarting") return;
        setPhase(getWelcomeDismissed() ? "app" : "welcome");
      });
    },
    [runCheck, clearStatus]
  );

  // Ensure main window is shown when running in Tauri (helps if window was created hidden)
  useEffect(() => {
    if (typeof (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ === "undefined") return;
    import("@tauri-apps/api/core").then(({ invoke }) => {
      invoke("show_main_window").catch(() => {});
    }).catch(() => {});
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
    if (typeof (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ !== "undefined") {
      import("@tauri-apps/api/window")
        .then((w) => w.getCurrentWindow().setTitle(title))
        .catch(() => {});
    }
  }, [phase, view]);

  const isEmbedView =
    view === "observer" || view === "express" || view === "finance" || view === "network";
  const lastUsedAppId: HubView | null = lastEmbedView;

  if (phase === "intro") {
    return <IntroView onComplete={handleIntroComplete} />;
  }

  if (phase === "updating") {
    return (
      <div className="update-overlay-screen update-overlay-screen--card" role="status" aria-live="polite">
        <UpdateOverlay status={updateStatus} showCheckingWhenIdle />
      </div>
    );
  }

  if (phase === "welcome") {
    return (
      <>
        <UpdateOverlay status={updateStatus} />
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
      <UpdateOverlay status={updateStatus} />
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
