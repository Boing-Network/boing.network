import { useCallback, useEffect, useMemo, useState, type ReactElement } from "react";
import {
  BoingClient,
  type BoingConnectionSnapshot,
  describeGetLogsLimits,
  displayChainTitle,
  doctorBoingRpcEnvironment,
  explainBoingRpcError,
  formatBoingRpcDoctorReport,
  formatSupportHint,
  isBoingRpcPreflightError,
} from "boing-sdk";
import { useBoingHubRpcMonitor } from "../hooks/useBoingHubRpcMonitor";
import { useHubRpcConfig } from "../lib/hubRpcConfig";

const QA_DOCTOR_REQUIRED_METHODS = [
  "boing_qaPoolList",
  "boing_qaPoolConfig",
  "boing_qaPoolVote",
  "boing_operatorApplyQaPolicy",
] as const;

function rpcUiMessage(e: unknown, client: BoingClient): string {
  if (isBoingRpcPreflightError(e)) {
    return formatSupportHint(e.message, client.getLastXRequestId());
  }
  return formatSupportHint(explainBoingRpcError(e), client.getLastXRequestId());
}

function connectionHeadline(s: BoingConnectionSnapshot): string {
  if (s.networkInfo != null) return displayChainTitle(s.networkInfo);
  const name = s.health?.chain_name?.trim();
  if (name) return name;
  const id = s.health?.chain_id;
  if (id != null) return `Boing chain ${id}`;
  return "Boing RPC";
}

type Props = {
  open: boolean;
  onClose: () => void;
};

export function RpcDiagnosticsModal({ open, onClose }: Props): ReactElement | null {
  const { rpcUrl, setRpcUrl, operatorToken, setOperatorToken } = useHubRpcConfig();
  const [doctorReport, setDoctorReport] = useState<string | null>(null);
  const [doctorLoading, setDoctorLoading] = useState(false);
  const [doctorError, setDoctorError] = useState<string | null>(null);

  const client = useMemo(() => {
    const extra =
      operatorToken.trim() !== "" ? { "X-Boing-Operator": operatorToken.trim() } : undefined;
    return new BoingClient({
      baseUrl: rpcUrl.trim() || "http://127.0.0.1:8545",
      extraHeaders: extra,
    });
  }, [rpcUrl, operatorToken]);

  const rpcSnapshot = useBoingHubRpcMonitor(client, { pollIntervalMs: 20_000, enabled: open });

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  const runDoctor = useCallback(async () => {
    setDoctorLoading(true);
    setDoctorReport(null);
    setDoctorError(null);
    try {
      const doctor = await doctorBoingRpcEnvironment(client, {
        requiredMethods: [...QA_DOCTOR_REQUIRED_METHODS],
      });
      setDoctorReport(formatBoingRpcDoctorReport(doctor));
      if (!doctor.ok) {
        setDoctorError("Environment check reported issues — see report below.");
      }
    } catch (e) {
      setDoctorError(rpcUiMessage(e, client));
    } finally {
      setDoctorLoading(false);
    }
  }, [client]);

  if (!open) return null;

  return (
    <div
      className="hub-rpc-modal-backdrop"
      role="presentation"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        className="hub-rpc-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="hub-rpc-modal-title"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="hub-rpc-modal__header">
          <h2 id="hub-rpc-modal-title" className="hub-rpc-modal__title">
            RPC &amp; diagnostics
          </h2>
          <button type="button" className="hub-rpc-modal__close" onClick={onClose} aria-label="Close">
            ×
          </button>
        </div>

        <div className="hub-rpc-modal__body">
          <p className="qa-operator__muted">
            Same RPC URL and operator token as the QA operator view (saved automatically). Use this to watch node health and
            run a full environment check before pool work.
          </p>

          <div className="qa-operator__grid">
            <label className="qa-operator__field">
              <span>RPC URL</span>
              <input
                type="url"
                value={rpcUrl}
                onChange={(e) => setRpcUrl(e.target.value)}
                autoComplete="off"
                spellCheck={false}
              />
            </label>
            <label className="qa-operator__field">
              <span>Operator token (optional)</span>
              <input
                type="password"
                value={operatorToken}
                onChange={(e) => setOperatorToken(e.target.value)}
                autoComplete="off"
                placeholder="BOING_OPERATOR_RPC_TOKEN"
              />
            </label>
          </div>

          <div className="qa-operator__rpc-live hub-rpc-modal__live" aria-live="polite">
            <h3 className="qa-operator__h3">Live RPC status</h3>
            {rpcSnapshot == null ? (
              <p className="qa-operator__muted">Starting RPC monitor…</p>
            ) : rpcSnapshot.loading && rpcSnapshot.health == null ? (
              <p className="qa-operator__muted">Fetching health and network info…</p>
            ) : (
              <>
                <p className="qa-operator__rpc-title">
                  <strong>{connectionHeadline(rpcSnapshot)}</strong>
                </p>
                <ul className="qa-operator__rpc-list">
                  <li>Head height: {rpcSnapshot.headHeight ?? "—"}</li>
                  <li>
                    Health:{" "}
                    {rpcSnapshot.health?.ok === true ? "ok" : rpcSnapshot.health != null ? "not ok" : "—"}
                  </li>
                  {rpcSnapshot.lastRequestId != null && rpcSnapshot.lastRequestId.length > 0 ? (
                    <li>
                      Last JSON-RPC request ID:{" "}
                      <code className="qa-operator__code">{rpcSnapshot.lastRequestId}</code>
                    </li>
                  ) : null}
                </ul>
                <p className="qa-operator__muted qa-operator__rpc-hint">
                  {describeGetLogsLimits(rpcSnapshot.rpcSurface)}
                </p>
                {rpcSnapshot.lastError != null ? (
                  <p className="qa-operator__error qa-operator__error--compact" role="status">
                    Status poll:{" "}
                    {rpcSnapshot.lastError instanceof Error
                      ? rpcSnapshot.lastError.message
                      : String(rpcSnapshot.lastError)}
                  </p>
                ) : null}
              </>
            )}
          </div>

          <div className="hub-rpc-modal__actions">
            <button
              type="button"
              className="qa-operator__btn qa-operator__btn--primary"
              disabled={doctorLoading}
              onClick={() => void runDoctor()}
            >
              Diagnose RPC environment
            </button>
          </div>

          {doctorError != null ? <p className="qa-operator__error" role="alert">{doctorError}</p> : null}

          {doctorReport != null && doctorReport.length > 0 ? (
            <div className="hub-rpc-modal__report">
              <h3 className="qa-operator__h3">Diagnose report</h3>
              <pre className="qa-operator__pre qa-operator__pre--doctor">{doctorReport}</pre>
            </div>
          ) : null}
        </div>
      </div>
    </div>
  );
}
