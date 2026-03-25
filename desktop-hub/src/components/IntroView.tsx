import { useState, useEffect, useCallback } from "react";

/** Match VibeMiner splash minimum display time (SPLASH_MIN_MS) */
const INTRO_DURATION_MS = 1800;
const FADE_OUT_MS = 400;

type Props = {
  onComplete: (skipIntroNextTime: boolean) => void;
};

export function IntroView({ onComplete }: Props) {
  const [phase, setPhase] = useState<"running" | "fadeout" | "done">("running");
  const [skipPressed, setSkipPressed] = useState(false);
  const [dontShowAgain, setDontShowAgain] = useState(false);

  const finish = useCallback(
    (skipIntroNextTime: boolean) => {
      setPhase("fadeout");
      const t = setTimeout(() => {
        setPhase("done");
        onComplete(skipIntroNextTime);
      }, FADE_OUT_MS);
      return () => clearTimeout(t);
    },
    [onComplete]
  );

  useEffect(() => {
    if (phase !== "running") return;
    const t = setTimeout(() => finish(dontShowAgain), INTRO_DURATION_MS);
    return () => clearTimeout(t);
  }, [phase, finish, dontShowAgain]);

  const handleSkip = useCallback(() => {
    if (skipPressed) return;
    setSkipPressed(true);
    finish(dontShowAgain);
  }, [finish, dontShowAgain, skipPressed]);

  return (
    <div
      className={`intro-view ${phase === "fadeout" ? "intro-view--fadeout" : ""}`}
      aria-hidden="true"
    >
      <button
        type="button"
        className="intro-view__skip"
        onClick={handleSkip}
        onKeyDown={(e) => e.key === "Enter" && handleSkip()}
        aria-label="Skip intro"
      >
        Skip
      </button>
      <div className="intro-view__content">
        <div className="intro-view__symbol" aria-hidden="true">
          <img src="/favicon.svg" alt="" className="intro-view__logo" />
        </div>
        <h1 className="intro-view__title">Boing Network Hub</h1>
        <p className="intro-view__tag">Observer, wallet &amp; apps — one desktop app.</p>
      </div>
      <label className="intro-view__checkbox">
        <input
          type="checkbox"
          checked={dontShowAgain}
          onChange={(e) => setDontShowAgain(e.target.checked)}
          aria-label="Don't show intro on next launch"
        />
        <span>Don't show intro on next launch</span>
      </label>
      <div className="intro-view__progress" aria-hidden="true">
        <div className="intro-view__progress-bar" style={{ animationDuration: `${INTRO_DURATION_MS}ms` }} />
      </div>
    </div>
  );
}
