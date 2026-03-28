import { useEffect, useState, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { relaunch } from "@tauri-apps/plugin-process";
import { check } from "@tauri-apps/plugin-updater";
import { useNavigate } from "react-router-dom";
import { isTauri as isTauriApp, isWindowsWebview } from "../lib/tauri";
import SplashDesktopUpdateOverlay from "./SplashUpdateOverlay";
import { BoingLoaderDots } from "./BoingLoaderDots";
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
 * Frameless splash: dice.express-style intro → inline “checking” on the same canvas →
 * full-card overlay only while downloading/installing (quiet check like BountyHub) → main window.
 */
export function SplashScreen() {
  const navigate = useNavigate();
  const [phase, setPhase] = useState<(typeof PHASE)[keyof typeof PHASE]>(PHASE.INTRO);
  const [introDone, setIntroDone] = useState(false);
  const [updateVersion, setUpdateVersion] = useState<string | null>(null);
  const cancelledRef = useRef(false);

  useEffect(() => {
    if (!isTauriApp()) {
      setIntroDone(true);
      return;
    }
    const t = setTimeout(() => setIntroDone(true), INTRO_DURATION_MS);
    return () => clearTimeout(t);
  }, []);

  useEffect(() => {
    if (isTauriApp()) return;
    if (!introDone) return;
    navigate("/app", { replace: true });
  }, [introDone, navigate]);

  useEffect(() => {
    if (!introDone || !isTauriApp()) return;

    cancelledRef.current = false;

    const run = async () => {
      setPhase(PHASE.CHECKING);

      let update: Awaited<ReturnType<typeof check>> = null;
      try {
        update = await check({ timeout: 22_000 });
      } catch {
        /* missing updater / network / TLS — continue (dice.express / BountyHub) */
      }
      if (cancelledRef.current) return;

      if (update) {
        setUpdateVersion(update.version);
        setPhase(PHASE.DOWNLOADING);

        try {
          await update.downloadAndInstall((ev) => {
            if (cancelledRef.current) return;
            if (ev.event === "Finished") {
              setPhase(PHASE.INSTALLING);
            }
          });
          if (cancelledRef.current) return;
          // Windows: successful updates exit the process inside the updater after spawning NSIS.
          if (!isWindowsWebview()) {
            try {
              await relaunch();
            } catch {
              /* process plugin missing */
            }
          }
          return;
        } catch {
          /* download / verify / install failed — open main on current build */
        }
      }

      if (cancelledRef.current) return;
      setPhase(PHASE.OPENING);

      try {
        await invoke("close_splash_and_show_main");
      } catch {
        /* ignore */
      }
    };

    void run();
    return () => {
      cancelledRef.current = true;
    };
  }, [introDone]);

  const showDownloadOverlay = phase === PHASE.DOWNLOADING || phase === PHASE.INSTALLING;

  return (
    <>
      <div className="splash-screen splash-screen--intro">
        <div className="splash-screen__content">
          <div className="splash-screen__symbol" aria-hidden>
            <img src="/favicon.svg" alt="" width={72} height={72} />
          </div>
          <h1 className="splash-screen__name">Boing Network Hub</h1>
          <p className="splash-screen__tagline">Observer, wallet &amp; apps — one desktop app.</p>
          {phase === PHASE.CHECKING && (
            <div className="splash-screen__checking">
              <BoingLoaderDots size="sm" />
              <p className="splash-screen__checking-label">Checking for updates…</p>
            </div>
          )}
        </div>
      </div>
      {showDownloadOverlay && (
        <SplashDesktopUpdateOverlay phase={phase} version={updateVersion} />
      )}
    </>
  );
}
