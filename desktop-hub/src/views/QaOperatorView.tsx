import { useCallback, useMemo, useState } from "react";
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

const LS_RPC = "boing-hub-qa-rpc-url";
const LS_TOKEN = "boing-hub-qa-operator-token";
const LS_VOTER = "boing-hub-qa-voter-hex";

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

function loadStored(key: string, fallback: string): string {
  try {
    const v = localStorage.getItem(key);
    return v != null && v !== "" ? v : fallback;
  } catch {
    return fallback;
  }
}

function store(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch {
    /* ignore */
  }
}

export function QaOperatorView() {
  const [rpcUrl, setRpcUrl] = useState(() => loadStored(LS_RPC, "http://127.0.0.1:8545"));
  const [operatorToken, setOperatorToken] = useState(() => loadStored(LS_TOKEN, ""));
  const [voterHex, setVoterHex] = useState(() => loadStored(LS_VOTER, ""));
  const [registryDraft, setRegistryDraft] = useState("");
  const [poolDraft, setPoolDraft] = useState("");
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [configJson, setConfigJson] = useState<string | null>(null);
  const [itemsJson, setItemsJson] = useState<string | null>(null);
  const [doctorReport, setDoctorReport] = useState<string | null>(null);
  const [doctorLoading, setDoctorLoading] = useState(false);

  const client = useMemo(() => {
    const extra =
      operatorToken.trim() !== "" ? { "X-Boing-Operator": operatorToken.trim() } : undefined;
    return new BoingClient({
      baseUrl: rpcUrl.trim() || "http://127.0.0.1:8545",
      extraHeaders: extra,
    });
  }, [rpcUrl, operatorToken]);

  const rpcSnapshot = useBoingHubRpcMonitor(client, { pollIntervalMs: 20_000 });

  const persistConnection = useCallback(() => {
    store(LS_RPC, rpcUrl.trim());
    store(LS_TOKEN, operatorToken);
    store(LS_VOTER, voterHex.trim());
  }, [rpcUrl, operatorToken, voterHex]);

  const runDoctor = useCallback(async () => {
    setDoctorLoading(true);
    setDoctorReport(null);
    setError(null);
    persistConnection();
    try {
      const doctor = await doctorBoingRpcEnvironment(client, {
        requiredMethods: [...QA_DOCTOR_REQUIRED_METHODS],
      });
      setDoctorReport(formatBoingRpcDoctorReport(doctor));
      if (!doctor.ok) {
        setError("RPC environment check reported issues — see Diagnose report below.");
      }
    } catch (e) {
      setError(rpcUiMessage(e, client));
    } finally {
      setDoctorLoading(false);
    }
  }, [client, persistConnection]);

  const refresh = useCallback(async () => {
    setError(null);
    setStatus(null);
    setLoading(true);
    persistConnection();
    try {
      const [cfg, list] = await Promise.all([client.qaPoolConfig(), client.qaPoolList()]);
      setConfigJson(JSON.stringify(cfg, null, 2));
      setItemsJson(JSON.stringify(list.items, null, 2));
      setStatus(`Loaded pool config and ${list.items.length} pending item(s).`);
    } catch (e) {
      setError(rpcUiMessage(e, client));
      setConfigJson(null);
      setItemsJson(null);
    } finally {
      setLoading(false);
    }
  }, [client, persistConnection]);

  const vote = useCallback(
    async (txHash: string, vote: "allow" | "reject" | "abstain") => {
      const v = voterHex.trim();
      if (!v) {
        setError("Set admin voter (32-byte account hex) before voting.");
        return;
      }
      setError(null);
      setStatus(null);
      setLoading(true);
      persistConnection();
      try {
        const res = await client.qaPoolVote(txHash, v, vote);
        setStatus(`Vote ${vote}: ${JSON.stringify(res)}`);
        await refresh();
      } catch (e) {
        setError(rpcUiMessage(e, client));
      } finally {
        setLoading(false);
      }
    },
    [client, voterHex, persistConnection, refresh]
  );

  const applyPolicy = useCallback(async () => {
    setError(null);
    setStatus(null);
    if (!registryDraft.trim() || !poolDraft.trim()) {
      setError("Paste both registry JSON and pool config JSON before applying.");
      return;
    }
    setLoading(true);
    persistConnection();
    try {
      await client.operatorApplyQaPolicy(registryDraft.trim(), poolDraft.trim());
      setStatus("Applied QA policy on node.");
      await refresh();
    } catch (e) {
      setError(rpcUiMessage(e, client));
    } finally {
      setLoading(false);
    }
  }, [client, registryDraft, poolDraft, persistConnection, refresh]);

  let items: { tx_hash: string; allow_votes: number; reject_votes: number; age_secs: number }[] = [];
  try {
    if (itemsJson) items = JSON.parse(itemsJson) as typeof items;
  } catch {
    /* keep empty */
  }

  return (
    <div className="qa-operator">
      <header className="qa-operator__header">
        <h1 className="qa-operator__title">QA operator</h1>
        <p className="qa-operator__lead">
          This screen is the main place for day-to-day pool work: refresh the queue, inspect config, vote on pending deploys,
          and apply updated registry / pool JSON—no terminal required. When the node is configured with{" "}
          <code className="qa-operator__code">BOING_OPERATOR_RPC_TOKEN</code>, enter the same value here as the operator token
          so requests include <code className="qa-operator__code">X-Boing-Operator</code>.
        </p>
      </header>

      <section className="qa-operator__panel" aria-label="Connection">
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
            <span>Operator token (optional locally)</span>
            <input
              type="password"
              value={operatorToken}
              onChange={(e) => setOperatorToken(e.target.value)}
              autoComplete="off"
              placeholder="Matches node BOING_OPERATOR_RPC_TOKEN"
            />
          </label>
          <label className="qa-operator__field">
            <span>Admin voter (hex account id)</span>
            <input
              value={voterHex}
              onChange={(e) => setVoterHex(e.target.value)}
              autoComplete="off"
              spellCheck={false}
              placeholder="0x… 32 bytes"
            />
          </label>
        </div>

        <div className="qa-operator__rpc-live" aria-live="polite">
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
              <p className="qa-operator__muted qa-operator__rpc-hint">{describeGetLogsLimits(rpcSnapshot.rpcSurface)}</p>
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

        <div className="qa-operator__actions">
          <button type="button" className="qa-operator__btn qa-operator__btn--primary" disabled={loading} onClick={() => void refresh()}>
            Refresh list &amp; config
          </button>
          <button
            type="button"
            className="qa-operator__btn"
            disabled={loading || doctorLoading}
            onClick={() => void runDoctor()}
          >
            Diagnose RPC environment
          </button>
        </div>
      </section>

      {doctorReport != null && doctorReport.length > 0 ? (
        <section className="qa-operator__panel" aria-label="RPC diagnose report">
          <h2 className="qa-operator__h2">Diagnose report</h2>
          <pre className="qa-operator__pre qa-operator__pre--doctor">{doctorReport}</pre>
        </section>
      ) : null}

      {status && <p className="qa-operator__status">{status}</p>}
      {error && <p className="qa-operator__error" role="alert">{error}</p>}

      {configJson && (
        <section className="qa-operator__panel" aria-label="Pool config">
          <h2 className="qa-operator__h2">Effective pool config</h2>
          <pre className="qa-operator__pre">{configJson}</pre>
        </section>
      )}

      <section className="qa-operator__panel" aria-label="Pending items">
        <h2 className="qa-operator__h2">Pending items</h2>
        {items.length === 0 ? (
          <p className="qa-operator__muted">{itemsJson == null ? "Refresh to load." : "No pending items."}</p>
        ) : (
          <div className="qa-operator__table-wrap">
            <table className="qa-operator__table">
              <thead>
                <tr>
                  <th>tx_hash</th>
                  <th>votes</th>
                  <th>age (s)</th>
                  <th>actions</th>
                </tr>
              </thead>
              <tbody>
                {items.map((row) => (
                  <tr key={row.tx_hash}>
                    <td className="qa-operator__mono">{row.tx_hash}</td>
                    <td>
                      {row.allow_votes} / {row.reject_votes}
                    </td>
                    <td>{row.age_secs}</td>
                    <td>
                      <div className="qa-operator__vote-btns">
                        <button
                          type="button"
                          className="qa-operator__btn qa-operator__btn--small"
                          disabled={loading}
                          onClick={() => void vote(row.tx_hash, "allow")}
                        >
                          Allow
                        </button>
                        <button
                          type="button"
                          className="qa-operator__btn qa-operator__btn--small"
                          disabled={loading}
                          onClick={() => void vote(row.tx_hash, "reject")}
                        >
                          Reject
                        </button>
                        <button
                          type="button"
                          className="qa-operator__btn qa-operator__btn--small"
                          disabled={loading}
                          onClick={() => void vote(row.tx_hash, "abstain")}
                        >
                          Abstain
                        </button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>

      <section className="qa-operator__panel" aria-label="Apply policy">
        <h2 className="qa-operator__h2">Apply QA policy (operator RPC)</h2>
        <p className="qa-operator__muted">
          Paste full JSON here (same shape as <code className="qa-operator__code">qa_registry.json</code> and{" "}
          <code className="qa-operator__code">qa_pool_config.json</code>), then apply—this is the usual workflow in the hub.
          For CI or scripts that read files from disk, the optional CLI command{" "}
          <code className="qa-operator__code">boing qa apply</code> calls the same RPC.
        </p>
        <label className="qa-operator__field qa-operator__field--block">
          <span>Registry JSON</span>
          <textarea value={registryDraft} onChange={(e) => setRegistryDraft(e.target.value)} rows={8} spellCheck={false} />
        </label>
        <label className="qa-operator__field qa-operator__field--block">
          <span>Pool config JSON</span>
          <textarea value={poolDraft} onChange={(e) => setPoolDraft(e.target.value)} rows={8} spellCheck={false} />
        </label>
        <button type="button" className="qa-operator__btn qa-operator__btn--primary" disabled={loading} onClick={() => void applyPolicy()}>
          Apply on node
        </button>
      </section>
    </div>
  );
}
