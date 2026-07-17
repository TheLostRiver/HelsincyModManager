import { Clipboard, Download, RefreshCw, ShieldCheck } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Dialog, useFeedback } from "../../shared/feedback";
import { exportSupportDiagnostics, getDiagnosticsPageSnapshot } from "./diagnosticsApi";
import type { DiagnosticsPageSnapshot } from "./diagnosticsTypes";
import "./DiagnosticsPage.css";

type PageState = { status: "loading" } | { status: "failed" } | { status: "ready"; snapshot: DiagnosticsPageSnapshot };

export function DiagnosticsPage() {
  const { pushToast } = useFeedback();
  const [state, setState] = useState<PageState>({ status: "loading" });
  const [confirming, setConfirming] = useState(false);
  const [exporting, setExporting] = useState(false);

  const load = useCallback(() => {
    setState({ status: "loading" });
    void getDiagnosticsPageSnapshot()
      .then((snapshot) => setState({ status: "ready", snapshot }))
      .catch(() => setState({ status: "failed" }));
  }, []);
  useEffect(load, [load]);

  const copyStableValue = useCallback((value: string) => {
    void navigator.clipboard.writeText(value)
      .then(() => pushToast({ eventKey: `diagnostics.copied.${value}`, title: "已复制诊断标识", message: value, tone: "success" }))
      .catch(() => pushToast({ eventKey: "diagnostics.copy.failed", title: "复制失败", message: "无法写入剪贴板，请手动记录稳定诊断标识。", tone: "danger" }));
  }, [pushToast]);

  const confirmExport = useCallback(() => {
    if (exporting) return;
    setExporting(true);
    void exportSupportDiagnostics().then((result) => {
      pushToast({ eventKey: `diagnostics.exported.${result.exportId}`, title: "诊断包已导出", message: `${result.fileName}，${formatBytes(result.sizeBytes)}`, tone: "success" });
      setConfirming(false);
    }).catch(() => pushToast({ eventKey: "diagnostics.export.failed", title: "诊断导出失败", message: "未生成诊断包，请稍后重试。", tone: "danger" }))
      .finally(() => setExporting(false));
  }, [exporting, pushToast]);

  return <section className="diagnostics-page" aria-labelledby="diagnostics-title">
    <header className="diagnostics-page__hero">
      <div><span>只读支持工具</span><h2 id="diagnostics-title">日志与诊断</h2><p>这里只显示后端已校验和脱敏的信息，不展示本地路径或原始错误。</p></div>
      <div className="diagnostics-page__actions"><button onClick={load}><RefreshCw size={16}/>刷新</button><button className="is-primary" onClick={() => setConfirming(true)}><Download size={16}/>导出诊断包</button></div>
    </header>
    {state.status === "loading" && <div className="diagnostics-page__state" role="status">正在读取安全诊断摘要…</div>}
    {state.status === "failed" && <div className="diagnostics-page__state is-error"><h3>诊断摘要不可用</h3><p>读取失败未暴露原始错误；可重试或直接使用受控导出。</p></div>}
    {state.status === "ready" && (
      <DiagnosticsContent snapshot={state.snapshot} onCopy={copyStableValue}/>
    )}
    <Dialog open={confirming} title="确认导出诊断包" description="导出包将包含平台摘要、已脱敏 App/Task 日志、已校验审计事件和健康聚合，不包含完整路径与原始错误。" onClose={() => !exporting && setConfirming(false)} footer={<><button onClick={() => setConfirming(false)} disabled={exporting}>取消</button><button onClick={confirmExport} disabled={exporting}>{exporting ? "导出中…" : "确认导出"}</button></>} />
  </section>;
}

function DiagnosticsContent({ snapshot, onCopy }: { snapshot: DiagnosticsPageSnapshot; onCopy: (value: string) => void }) {
  return <>
    <section className="diagnostics-page__health" aria-label="诊断健康摘要">
      <HealthCard label="平台" status={snapshot.platformStatus}/><HealthCard label="App Log" status={snapshot.appLogStatus}/><HealthCard label="Task Log" status={combinedStatus(snapshot.taskLogStatus, snapshot.evidenceHealth.taskLogStatus)}/><HealthCard label="Audit Log" status={combinedStatus(snapshot.auditLogStatus, snapshot.evidenceHealth.auditLogStatus)}/>
    </section>
    {snapshot.platformSummary && <section className="diagnostics-page__platform"><ShieldCheck size={20}/><div><strong>HMM {snapshot.platformSummary.appVersion}</strong><span>{snapshot.platformSummary.os} · {snapshot.platformSummary.arch} · adapters: {snapshot.platformSummary.gameAdapterIds.join(", ") || "none"}</span></div></section>}
    <section className="diagnostics-page__columns"><LogPanel title="App Log" status={snapshot.appLogStatus} lines={snapshot.appLogLines}/><LogPanel title="Task Log" status={snapshot.taskLogStatus} lines={snapshot.taskLogLines}/></section>
    <section className="diagnostics-page__panel"><h3>最近审计事件 <small>{snapshot.auditLogStatus}</small></h3>{snapshot.auditEvents.length === 0 ? <p className="is-empty">没有可显示的已校验事件。</p> : snapshot.auditEvents.map((event, index) => <article className="diagnostics-page__audit" key={`${event.timestampUnixMillis}-${index}`}><div><strong>{event.operation}</strong><span>{event.category} · {event.result}</span></div><div>{[event.fields.error_code, event.fields.task_id].filter(Boolean).map(value => <button key={value} onClick={() => onCopy(value)} title="复制稳定标识"><Clipboard size={14}/>{value}</button>)}</div></article>)}</section>
  </>;
}

function HealthCard({ label, status }: { label: string; status: string }) { const ok = status === "ok"; return <article className={`diagnostics-page__health-card ${ok ? "is-ok" : "is-warning"}`}><span>{label}</span><strong>{ok ? "正常" : status}</strong></article>; }
function combinedStatus(readStatus: string, writeStatus: string) { return readStatus === "ok" ? writeStatus : readStatus; }
function LogPanel({ title, status, lines }: { title: string; status: string; lines: { source: string; line: string }[] }) { return <section className="diagnostics-page__panel"><h3>{title} <small>{status}</small></h3>{lines.length === 0 ? <p className="is-empty">没有可显示的安全日志。</p> : <div className="diagnostics-page__log">{lines.map((item, index) => <div key={`${item.source}-${index}`}><span>{item.source}</span><code>{item.line}</code></div>)}</div>}</section>; }
function formatBytes(value: number) { return value < 1024 ? `${value} B` : `${(value / 1024).toFixed(1)} KB`; }
