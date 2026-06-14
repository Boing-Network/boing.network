import { useState, useCallback, useEffect } from "react";
import { AppIcon } from "../components/AppIcons";
import { BoingLoaderDots } from "../components/BoingLoaderDots";

type Props = { url: string; title: string; appId: string };

function LoadingSpinner() {
  return (
    <div className="app-loading" role="status" aria-live="polite" aria-busy="true" aria-label="Loading app">
      <BoingLoaderDots size="lg" />
      <span className="app-loading-text">Loading…</span>
    </div>
  );
}

function ErrorFallback({
  onRetry,
  onOpenInBrowser,
  title,
}: {
  onRetry: () => void;
  onOpenInBrowser: () => void;
  title: string;
}) {
  return (
    <div className="app-error" role="alert">
      <p className="app-error-title">Couldn’t load {title}</p>
      <p className="app-error-desc">Check your connection or open the app in your browser.</p>
      <div className="app-error-actions">
        <button type="button" className="app-error-btn" onClick={onRetry}>
          Retry
        </button>
        <button type="button" className="app-error-btn primary" onClick={onOpenInBrowser}>
          Open in browser
        </button>
      </div>
    </div>
  );
}

export function EmbedView({ url, title, appId }: Props) {
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(false);
  const [iframeKey, setIframeKey] = useState(0);

  const handleLoad = useCallback(() => {
    setLoading(false);
    setError(false);
  }, []);

  const handleError = useCallback(() => {
    setLoading(false);
    setError(true);
  }, []);

  const handleRetry = useCallback(() => {
    setError(false);
    setLoading(true);
    setIframeKey((k) => k + 1);
  }, []);

  const iframeUrl = url;

  useEffect(() => {
    if (!loading || error) return;
    const t = setTimeout(() => {
      setLoading(false);
      setError(true);
    }, 20000);
    return () => clearTimeout(t);
  }, [loading, error, iframeKey]);

  const handleOpenInBrowser = useCallback(async () => {
    try {
      const { open } = await import("@tauri-apps/plugin-shell");
      await open(iframeUrl);
    } catch {
      window.open(iframeUrl, "_blank", "noopener,noreferrer");
    }
  }, [iframeUrl]);

  return (
    <div className="app-window">
      <header className="app-title-bar" aria-label={`${title} window`}>
        <span className="app-title-bar-icon">
          <AppIcon appId={appId} size={18} />
        </span>
        <span className="app-title-bar-title">{title}</span>
        <div className="app-title-bar-actions">
          <button
            type="button"
            className="app-title-bar-btn"
            onClick={handleOpenInBrowser}
            title={`Open ${title} in browser`}
            aria-label={`Open ${title} in default browser`}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden>
              <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" />
              <polyline points="15 3 21 3 21 9" />
              <line x1="10" y1="14" x2="21" y2="3" />
            </svg>
            <span className="app-title-bar-btn-label">Open in browser</span>
          </button>
        </div>
      </header>
      <div className="app-content">
        {error ? (
          <ErrorFallback onRetry={handleRetry} onOpenInBrowser={handleOpenInBrowser} title={title} />
        ) : (
          <>
            {loading && <LoadingSpinner />}
            <iframe
              key={iframeKey}
              className="app-embed"
              src={iframeUrl}
              title={title}
              sandbox="allow-scripts allow-same-origin allow-forms allow-popups allow-popups-to-escape-sandbox"
              allow="clipboard-read; clipboard-write"
              onLoad={handleLoad}
              onError={handleError}
              style={{ opacity: loading ? 0 : 1 }}
            />
          </>
        )}
      </div>
    </div>
  );
}
