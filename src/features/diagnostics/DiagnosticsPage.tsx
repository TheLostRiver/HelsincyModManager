import { AlertTriangle, Clipboard, Download, RefreshCw, ShieldCheck } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { Dialog, useFeedback } from "../../shared/feedback";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { exportSupportDiagnostics, getDiagnosticsPageSnapshot } from "./diagnosticsApi";
import { diagnosticsCopy, type DiagnosticsCopy } from "./diagnosticsCopy";
import { createLatestRequestController, createSingleFlightController, runDeferred } from "./diagnosticsPageLogic";
import type { DiagnosticsPageSnapshot } from "./diagnosticsTypes";
import "./DiagnosticsPage.css";

type PageState = { status: "loading" } | { status: "failed" } | { status: "ready"; snapshot: DiagnosticsPageSnapshot };

export function DiagnosticsPage() {
  const { pushToast } = useFeedback();
  const { locale } = useI18n();
  const copy = resolveCopy(diagnosticsCopy, locale);
  const [state, setState] = useState<PageState>({ status: "loading" });
  const [confirming, setConfirming] = useState(false);
  const [exporting, setExporting] = useState(false);
  const cancelExportRef = useRef<HTMLButtonElement>(null);
  const loadControllerRef = useRef(createLatestRequestController());
  const exportControllerRef = useRef(createSingleFlightController());

  const load = useCallback(() => {
    setState({ status: "loading" });
    void loadControllerRef.current.run(getDiagnosticsPageSnapshot, {
      onSuccess: (snapshot) => setState({ status: "ready", snapshot }),
      onFailure: () => setState({ status: "failed" }),
    });
  }, []);
  useEffect(() => {
    load();
    const loadController = loadControllerRef.current;
    return () => loadController.invalidate();
  }, [load]);

  const copyStableValue = useCallback((value: string) => {
    void runDeferred(() => navigator.clipboard.writeText(value))
      .then(() => pushToast({ eventKey: `diagnostics.copied.${value}`, title: copy.toasts.copiedTitle, message: value, tone: "success" }))
      .catch(() => pushToast({ eventKey: "diagnostics.copy.failed", title: copy.toasts.copyFailedTitle, message: copy.toasts.copyFailedMessage, tone: "danger" }));
  }, [copy, pushToast]);

  const confirmExport = useCallback(() => {
    const exportPromise = exportControllerRef.current.run(exportSupportDiagnostics);
    if (!exportPromise) return;
    setExporting(true);
    void exportPromise.then((result) => {
      pushToast({
        eventKey: `diagnostics.exported.${result.exportId}`,
        title: copy.toasts.exportedTitle,
        message: copy.toasts.exportedMessage({
          fileName: result.fileName,
          size: formatBytes(result.sizeBytes),
          appLogLineCount: result.appLogLineCount,
          debugLogLineCount: result.debugLogLineCount,
          taskLogLineCount: result.taskLogLineCount,
          auditEventCount: result.auditEventCount,
        }),
        tone: "success",
      });
      setConfirming(false);
    }).catch(() => pushToast({ eventKey: "diagnostics.export.failed", title: copy.toasts.exportFailedTitle, message: copy.toasts.exportFailedMessage, tone: "danger" }))
      .finally(() => setExporting(false));
  }, [copy, pushToast]);

  return (
    <section className="diagnostics-page" aria-labelledby="diagnostics-title">
      <header className="diagnostics-page__hero" data-tour-id="diagnostics.actions">
        <div>
          <span>{copy.page.eyebrow}</span>
          <h2 id="diagnostics-title">{copy.page.title}</h2>
          <p>{copy.page.subtitle}</p>
        </div>
        <div className="diagnostics-page__actions">
          <button type="button" onClick={load}>
            <RefreshCw size={16} aria-hidden="true" />
            {copy.page.refresh}
          </button>
          <button type="button" className="is-primary" onClick={() => setConfirming(true)}>
            <Download size={16} aria-hidden="true" />
            {copy.page.exportBundle}
          </button>
        </div>
      </header>

      {state.status === "loading" && (
        <div className="diagnostics-page__state" role="status" data-tour-id="diagnostics.state">
          <span className="diagnostics-page__state-icon" aria-hidden="true">
            <RefreshCw size={20} />
          </span>
          <p>{copy.page.loading}</p>
        </div>
      )}

      {/*
       * 失败态用 role="alert"（隐含 assertive live region）：加载态是 role="status"，
       * 若失败态不带 role，读屏用户在错误出现时不会收到任何通知，只能自己浏览到才发现。
       */}
      {state.status === "failed" && (
        <div
          className="diagnostics-page__state is-error"
          role="alert"
          data-tour-id="diagnostics.state"
        >
          <span className="diagnostics-page__state-icon" aria-hidden="true">
            <AlertTriangle size={22} />
          </span>
          <h3>{copy.page.failedTitle}</h3>
          <p>{copy.page.failedHint}</p>
          {/* 文案承诺了"可重试"，重试入口就应当在同一处，而不是让用户回到页头去找。 */}
          <button type="button" onClick={load}>
            <RefreshCw size={16} aria-hidden="true" />
            {copy.page.retry}
          </button>
        </div>
      )}

      {state.status === "ready" && (
        <DiagnosticsContent copy={copy} snapshot={state.snapshot} onCopy={copyStableValue} />
      )}

      <Dialog
        open={confirming}
        title={copy.dialog.title}
        description={copy.dialog.description}
        busy={exporting}
        initialFocusRef={cancelExportRef}
        onClose={() => setConfirming(false)}
        footer={
          <>
            <button
              ref={cancelExportRef}
              type="button"
              className="diagnostics-page__dialog-action is-secondary"
              onClick={() => setConfirming(false)}
              disabled={exporting}
            >
              {copy.dialog.cancel}
            </button>
            <button
              type="button"
              className="diagnostics-page__dialog-action is-primary"
              onClick={confirmExport}
              disabled={exporting}
            >
              <Download size={16} aria-hidden="true" />
              {exporting ? copy.dialog.exporting : copy.dialog.confirm}
            </button>
          </>
        }
      />
    </section>
  );
}

function DiagnosticsContent({
  copy,
  snapshot,
  onCopy,
}: {
  copy: DiagnosticsCopy;
  snapshot: DiagnosticsPageSnapshot;
  onCopy: (value: string) => void;
}) {
  return (
    <>
      <section
        className="diagnostics-page__health"
        aria-label={copy.content.healthAria}
        data-tour-id="diagnostics.health"
      >
        <HealthCard copy={copy} label={copy.content.platformLabel} status={snapshot.platformStatus} />
        <HealthCard copy={copy} label="App Log" status={snapshot.appLogStatus} />
        <HealthCard copy={copy} label="Debug Log" status={combinedStatus(snapshot.debugLogStatus, snapshot.evidenceHealth.debugLogStatus)} />
        <HealthCard
          copy={copy}
          label="Task Log"
          status={combinedStatus(snapshot.taskLogStatus, snapshot.evidenceHealth.taskLogStatus)}
        />
        <HealthCard
          copy={copy}
          label="Audit Log"
          status={combinedStatus(snapshot.auditLogStatus, snapshot.evidenceHealth.auditLogStatus)}
        />
        <HealthCard copy={copy} label={copy.content.logStorageLabel} status={snapshot.evidenceHealth.logStorageStatus} />
      </section>

      {snapshot.platformSummary && (
        <section className="diagnostics-page__platform">
          <ShieldCheck size={20} aria-hidden="true" />
          <div>
            <strong>HMM {snapshot.platformSummary.appVersion}</strong>
            <span>
              {snapshot.platformSummary.os} · {snapshot.platformSummary.arch} · adapters:{" "}
              {snapshot.platformSummary.gameAdapterIds.join(", ") || "none"}
            </span>
          </div>
        </section>
      )}

      <section className="diagnostics-page__columns" data-tour-id="diagnostics.logs">
        <LogPanel copy={copy} title="App Log" status={snapshot.appLogStatus} lines={snapshot.appLogLines} />
        <LogPanel copy={copy} title="Debug Log" status={snapshot.debugLogStatus} lines={snapshot.debugLogLines} />
        <LogPanel copy={copy} title="Task Log" status={snapshot.taskLogStatus} lines={snapshot.taskLogLines} />
      </section>

      <section className="diagnostics-page__panel">
        <h3>
          {copy.content.auditTitle} <small>{snapshot.auditLogStatus}</small>
        </h3>
        {snapshot.auditEvents.length === 0 ? (
          <p className="is-empty">{copy.content.auditEmpty}</p>
        ) : (
          snapshot.auditEvents.map((event, index) => (
            <article
              className="diagnostics-page__audit"
              key={`${event.timestampUnixMillis}-${index}`}
            >
              <div>
                <strong>{event.operation}</strong>
                <span>
                  {event.category} · {event.result}
                </span>
              </div>
              <div>
                {[event.fields.error_code, event.fields.task_id].filter(Boolean).map((value) => (
                  <button
                    type="button"
                    key={value}
                    onClick={() => onCopy(value)}
                    title={copy.content.copyStableIdTitle}
                  >
                    <Clipboard size={14} aria-hidden="true" />
                    {value}
                  </button>
                ))}
              </div>
            </article>
          ))
        )}
      </section>
    </>
  );
}

function HealthCard({ copy, label, status }: { copy: DiagnosticsCopy; label: string; status: string }) {
  const ok = status === "ok";
  return (
    <article className={`diagnostics-page__health-card ${ok ? "is-ok" : "is-warning"}`}>
      <span>{label}</span>
      <strong>{ok ? copy.content.healthOk : status}</strong>
    </article>
  );
}

function combinedStatus(readStatus: string, writeStatus: string) {
  return readStatus === "ok" ? writeStatus : readStatus;
}

function LogPanel({
  copy,
  title,
  status,
  lines,
}: {
  copy: DiagnosticsCopy;
  title: string;
  status: string;
  lines: { source: string; line: string }[];
}) {
  return (
    <section className="diagnostics-page__panel">
      <h3>
        {title} <small>{status}</small>
      </h3>
      {lines.length === 0 ? (
        <p className="is-empty">{copy.content.logEmpty}</p>
      ) : (
        <div className="diagnostics-page__log">
          {lines.map((item, index) => (
            <div key={`${item.source}-${index}`}>
              <span>{item.source}</span>
              <code>{item.line}</code>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
function formatBytes(value: number) {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / (1024 * 1024)).toFixed(1)} MB`;
}
