import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { getInstallManifestStatus } from "../mods/modInstallPlanApi";
import { TASK_PROGRESS_EVENT_NAME, type TaskProgressEventDto } from "../mods/modImportTypes";
import { modReinstallCopy } from "../mods/modReinstallCopy";
import { getReinstallPreviewErrorMessage, getReinstallStartErrorMessage } from "../mods/modReinstallTaskState";
import type { ReinstallPlanPreview } from "../mods/modReinstallTypes";
import { ReinstallPreviewSummary } from "../mods/ReinstallPlanPreviewPanel";
import { previewEquipmentReapply, startEquipmentReapply } from "../replacements/equipmentRetargetApi";
import { cancelRetargetInstallTask } from "../replacements/replacementApi";
import { replacementCopy } from "../replacements/replacementCopy";
import { canCancelRetargetInstallTaskPhase, isRetargetInstallTaskPhase, nextRetargetInstallTaskState, type RetargetInstallTaskState } from "../replacements/replacementWorkflow";
import { RetargetFileDetails } from "../replacements/RetargetFileDetails";
import { pluginSelectionCopy } from "./pluginSelectionCopy";
import type { PluginSelectionController } from "./usePluginSelection";

/** The caller keys this controller by profile/Mod/revision. Reapply targets stay backend-owned. */
export function PluginReapplyActions({ profileId, modId, plugins, disabled, refreshToken, onBusyChange, onCompleted }: {
  profileId: string; modId: string; plugins: PluginSelectionController; disabled: boolean; refreshToken: number;
  onBusyChange: (busy: boolean) => void; onCompleted: () => void;
}) {
  const { locale } = useI18n();
  const copy = resolveCopy(pluginSelectionCopy, locale);
  const reCopy = resolveCopy(modReinstallCopy, locale);
  const replacement = resolveCopy(replacementCopy, locale);
  const [installed, setInstalled] = useState(false);
  const [preview, setPreview] = useState<ReinstallPlanPreview | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [task, setTask] = useState<RetargetInstallTaskState>({ status: "idle" });
  const taskRef = useRef(task); taskRef.current = task;
  const [listener, setListener] = useState<"loading" | "ready" | "failed">("loading");
  const [listenAttempt, setListenAttempt] = useState(0);
  const early = useRef(new Map<string, TaskProgressEventDto>());
  const mounted = useRef(true);
  const generation = useRef(0);
  const latest = useRef({ replacement, onCompleted }); latest.current = { replacement, onCompleted };
  const completed = useRef<string | null>(null);
  const active = task.status === "starting" || task.status === "running";
  const track = (next: RetargetInstallTaskState) => { taskRef.current = next; setTask(next); };
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; generation.current += 1; };
  }, []);
  useEffect(() => { generation.current += 1; setPreview(null); setLoading(false); }, [disabled, refreshToken]);
  useEffect(() => {
    let disposed = false;
    void getInstallManifestStatus({ gameId: "mhw", profileId, modIds: [modId] }).then((values) => {
      if (!disposed) setInstalled(values.some((value) => value.modId === modId && value.status === "installed"));
    }, () => { if (!disposed) setInstalled(false); });
    return () => { disposed = true; };
  }, [profileId, modId, refreshToken]);
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    setListener("loading");
    void listen<TaskProgressEventDto>(TASK_PROGRESS_EVENT_NAME, ({ payload }) => {
      if (disposed || payload.kind !== "install" || !isRetargetInstallTaskPhase(payload.phase)) return;
      if (taskRef.current.status === "starting") {
        const previous = early.current.get(payload.taskId);
        if (!previous || !["completed", "failed", "cancelled"].includes(previous.status)) early.current.set(payload.taskId, payload);
      } else {
        const next = nextRetargetInstallTaskState(taskRef.current, payload, latest.current.replacement.events);
        taskRef.current = next; setTask(next);
      }
    }).then((dispose) => { if (disposed) dispose(); else { unlisten = dispose; setListener("ready"); } }, () => { if (!disposed) setListener("failed"); });
    return () => { disposed = true; unlisten?.(); };
  }, [listenAttempt]);
  useEffect(() => { onBusyChange(active); return () => onBusyChange(false); }, [active, onBusyChange]);
  useEffect(() => {
    if ((task.status === "completed" || task.status === "failed" || task.status === "cancelled") && task.taskId && completed.current !== task.taskId) {
      completed.current = task.taskId;
      setPreview(null);
      latest.current.onCompleted();
    }
  }, [task]);
  const generatePreview = async () => {
    if (disabled || !installed || active || !plugins.ready) return;
    const current = ++generation.current;
    setLoading(true); setError(null); setPreview(null);
    try {
      const result = await previewEquipmentReapply({ gameId: "mhw", profileId, modId });
      if (generation.current === current) setPreview(result);
    } catch (failure) {
      if (generation.current === current) setError(getReinstallPreviewErrorMessage(failure, reCopy.task));
    } finally { if (generation.current === current) setLoading(false); }
  };
  const start = async () => {
    if (disabled || !plugins.ready || listener !== "ready" || !installed || preview?.status !== "ready" || ["starting", "running"].includes(taskRef.current.status)) return;
    const token = preview.planToken;
    track({ status: "starting" }); early.current.clear(); setError(null);
    try {
      await plugins.confirm();
      if (!mounted.current) return;
      const started = await startEquipmentReapply({ gameId: "mhw", profileId, modId }, token);
      if (!mounted.current) return;
      if (started.kind !== "install" || started.status !== "queued") throw { code: "reinstall_start_failed" };
      const running: RetargetInstallTaskState = { status: "running", taskId: started.taskId, phase: "install.reinstall.queued" };
      const event = early.current.get(started.taskId);
      track(event ? nextRetargetInstallTaskState(running, event, latest.current.replacement.events) : running);
      early.current.clear();
    } catch (failure) {
      if (mounted.current) { track({ status: "idle" }); setPreview(null); setError(getReinstallStartErrorMessage(failure, reCopy.task)); }
    }
  };
  if (!installed) return null;
  return <section className="plugin-selection">
    <button type="button" className="install-config__button" disabled={disabled || active || loading || !plugins.ready} onClick={() => void generatePreview()}>{copy.preview}</button>
    {loading && <p role="status">{reCopy.dialog.loadingPreview}</p>}
    {error && <p role="alert">{error}</p>}
    {preview && <><ReinstallPreviewSummary preview={preview} /><RetargetFileDetails files={preview.fileEffects} />
      {preview.status === "no_changes" && <p role="status">{copy.noChanges}</p>}
      <button type="button" className="install-config__button is-primary" disabled={disabled || active || !plugins.ready || listener !== "ready" || preview.status !== "ready"} onClick={() => void start()}>{copy.apply}</button>
    </>}
    {listener === "failed" && <p role="alert">{reCopy.dialog.listenerFailed} <button type="button" onClick={() => setListenAttempt((value) => value + 1)}>{reCopy.dialog.retryListener}</button></p>}
    {active && <p role="status">{task.status === "running" ? replacement.phases[task.phase] : reCopy.dialog.starting}</p>}
    {task.status === "running" && <button type="button" disabled={!canCancelRetargetInstallTaskPhase(task.phase)} onClick={() => {
      void cancelRetargetInstallTask({ taskId: task.taskId }).catch((failure: unknown) => setError(getReinstallStartErrorMessage(failure, reCopy.task)));
    }}>{copy.cancel}</button>}
    {task.status === "completed" && <p role="status">{reCopy.dialog.completed}</p>}
    {task.status === "cancelled" && <p role="status">{reCopy.dialog.cancelled}</p>}
    {task.status === "failed" && <p role="alert">{task.message}</p>}
  </section>;
}
