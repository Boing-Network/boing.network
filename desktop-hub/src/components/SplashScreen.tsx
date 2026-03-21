import { useEffect, useState, useRef } from "react";
import { useNavigate } from "react-router-dom";
import { isTauri } from "../lib/tauri";
import UpdateOverlay from "./SplashUpdateOverlay";
import "./SplashScreen.css";

const INTRO_DURATION_MS = 1800;

const PHASE = {
  INTRO: "intro",
  CHECKING: "checking",
  DOWNLOADING: "downloading",
  INSTALLING: "installing",
  OPENING: "opening",
} as const;

/**
 * Frameless splash window: intro animation → update check (and optional download) → close and show main.
 * Same pattern as dice.express and vibeminer for reliable cold start and update flow.
 */
export function SplashScreen() {
  const navigate = useNavigate();
  const [phase, setPhase] = useState<typeof PHASE[keyof typeof PHASE]>(PHASE.INTRO);
  const [introDone, setIntroDone] = useState(false);
  const [updateVersion, setUpdateVersion] = useState<string | null>(null);
  const cancelledRef = useRef(false);

  // Intro animation: minimal fade-in (dice.express / vibeminer style)
  useEffect(() => {
    if (!isTauri) {
      setIntroDone(true);
      return;
    }
    const t = setTimeout(() => setIntroDone(true), INTRO_DURATION_MS);
    return () => clearTimeout(t);
  }, []);

  // After intro, run update check and optional download
  useEffect(() => {
    if (!introDone || !isTauri) return;

    cancelledRef.current = false;

    const run = async () => {
      setPhase(PHASE.CHECKING);

      try {
        const [{ check }, { relaunch }] = await Promise.all([
          import("@tauri-apps/plugin-updater"),
          import("@tauri-apps/plugin-process"),
        ]);
        const update = await check({ timeout: 22_000 });
        if (cancelledRef.current) return;

        if (update) {
          setUpdateVersion(update.version);
          setPhase(PHASE.DOWNLOADING);

          await update.downloadAndInstall(() => {
            if (cancelledRef.current) return;
          });

          if (cancelledRef.current) return;
          setPhase(PHASE.INSTALLING);
          await relaunch();
          return;
        }
      } catch {
        // No updater config or network error: continue to app
      }

      if (cancelledRef.current) return;
      setPhase(PHASE.OPENING);

      if (isTauri) {
        try {
          const { invoke } = await import("@tauri-apps/api/core");
          await invoke("close_splash_and_show_main");
        } catch {
          /* ignore */
        }
      } else {
        navigate("/app", { replace: true });
      }
    };

    void run();
    return () => {
      cancelledRef.current = true;
    };
  }, [introDone, navigate]);

  const showUpdateOverlay = phase === PHASE.DOWNLOADING || phase === PHASE.INSTALLING;

  return (
    <>
      <div className="splash-screen splash-screen--intro">
        <div className="splash-screen__content">
          <div className="splash-screen__symbol" aria-hidden>
            <img src="/favicon.svg" alt="" width={72} height={72} />
          </div>
          <h1 className="splash-screen__name">Boing Network</h1>
          <p className="splash-screen__tagline">Hub</p>
        </div>
      </div>
      {showUpdateOverlay && (
        <UpdateOverlay phase={phase} version={updateVersion} />
      )}
    </>
  );
}
