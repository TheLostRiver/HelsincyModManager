import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { getInstallManifestStatus } from "../mods/modInstallPlanApi";
import { TASK_PROGRESS_EVENT_NAME, type TaskProgressEventDto } from "../mods/modImportTypes";
import { modReinstallCopy } from "../mods/modReinstallCopy";
import { getReinstallPreviewErrorMessage, getReinstallStartErrorMessage } from "../mods/modReinstallTaskState";
import type { ReinstallPlanPreview } from "../mods/modReinstallTypes";
import { previewEquipmentReapply, startEquipmentReapply } from "../replacements/equipmentRetargetApi";
import { cancelRetargetInstallTask } from "../replacements/replacementApi";
import { replacementCopy } from "../replacements/replacementCopy";
import { canCancelRetargetInstallTaskPhase, isRetargetInstallTaskPhase, nextRetargetInstallTaskState, type RetargetInstallTaskState } from "../replacements/replacementWorkflow";
import type { PluginSelectionController } from "./usePluginSelection";

/** 布局开关不拥有任务；目标、token 和重新应用判定继续由原后端接口提供。 */
export function usePluginReapply({ profileId, modId, plugins, disabled, refreshToken, onCompleted }: {
  profileId: string | null; modId: string; plugins: PluginSelectionController; disabled: boolean;
  refreshToken: number; onCompleted: () => void;
}) {
  const { locale } = useI18n();
  const reCopy = resolveCopy(modReinstallCopy, locale);
  const replacement = resolveCopy(replacementCopy, locale);
  const scope = JSON.stringify([profileId, modId]);
  const scopeRef = useRef(scope); scopeRef.current = scope;
  const [stateScope, setStateScope] = useState(scope);
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
  const currentScope = stateScope === scope;
  const active = currentScope && (task.status === "starting" || task.status === "running");
  const track = (next: RetargetInstallTaskState) => { taskRef.current = next; setTask(next); };
  const stillHere = () => mounted.current && scopeRef.current === scope;

  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; generation.current += 1; };
  }, []);
  useEffect(() => {
    generation.current += 1;
    setStateScope(scope); setInstalled(false); setPreview(null); setLoading(false); setError(null);
    taskRef.current = { status: "idle" }; setTask(taskRef.current);
    early.current.clear(); completed.current = null;
  }, [scope]);
  useEffect(() => { generation.current += 1; setPreview(null); setLoading(false); }, [disabled, refreshToken]);
  useEffect(() => {
    let disposed = false;
    if (!profileId) return;
    void getInstallManifestStatus({ gameId: "mhw", profileId, modIds: [modId] }).then((values) => {
      if (!disposed) setInstalled(values.some((value) => value.modId === modId && value.status === "installed"));
    }, () => { if (!disposed) setInstalled(false); });
    return () => { disposed = true; };
  }, [profileId, modId, refreshToken]);
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    setListener("loading");
    if (!profileId) return;
    void listen<TaskProgressEventDto>(TASK_PROGRESS_EVENT_NAME, ({ payload }) => {
      if (disposed || scopeRef.current !== scope || payload.kind !== "install" || !isRetargetInstallTaskPhase(payload.phase)) return;
      if (taskRef.current.status === "starting") {
        const previous = early.current.get(payload.taskId);
        if (!previous || !["completed", "failed", "cancelled"].includes(previous.status)) early.current.set(payload.taskId, payload);
      } else {
        const next = nextRetargetInstallTaskState(taskRef.current, payload, latest.current.replacement.events);
        taskRef.current = next; setTask(next);
      }
    }).then((dispose) => { if (disposed) dispose(); else { unlisten = dispose; setListener("ready"); } }, () => { if (!disposed) setListener("failed"); });
    return () => { disposed = true; unlisten?.(); };
  }, [listenAttempt, profileId, scope]);
  useEffect(() => {
    if (currentScope && (task.status === "completed" || task.status === "failed" || task.status === "cancelled") && task.taskId && completed.current !== task.taskId) {
      completed.current = task.taskId; setPreview(null); latest.current.onCompleted();
    }
  }, [task, currentScope]);

  const generatePreview = async () => {
    if (!profileId || !currentScope || disabled || !installed || active || !plugins.ready) return;
    const current = ++generation.current;
    setLoading(true); setError(null); setPreview(null);
    try {
      const result = await previewEquipmentReapply({ gameId: "mhw", profileId, modId });
      if (stillHere() && generation.current === current) setPreview(result);
    } catch (failure) {
      if (stillHere() && generation.current === current) setError(getReinstallPreviewErrorMessage(failure, reCopy.task));
    } finally { if (stillHere() && generation.current === current) setLoading(false); }
  };
  const start = async () => {
    if (!profileId || !currentScope || disabled || !plugins.ready || listener !== "ready" || !installed || preview?.status !== "ready" || ["starting", "running"].includes(taskRef.current.status)) return;
    const token = preview.planToken;
    const current = generation.current;
    track({ status: "starting" }); early.current.clear(); setError(null);
    try {
      await plugins.confirm();
      if (!stillHere()) return;
      if (generation.current !== current) { track({ status: "idle" }); setPreview(null); return; }
      const started = await startEquipmentReapply({ gameId: "mhw", profileId, modId }, token);
      if (!stillHere()) return;
      if (started.kind !== "install" || started.status !== "queued") throw { code: "reinstall_start_failed" };
      const running: RetargetInstallTaskState = { status: "running", taskId: started.taskId, phase: "install.reinstall.queued" };
      const event = early.current.get(started.taskId);
      track(event ? nextRetargetInstallTaskState(running, event, latest.current.replacement.events) : running);
      early.current.clear();
    } catch (failure) {
      if (stillHere()) { track({ status: "idle" }); setPreview(null); setError(getReinstallStartErrorMessage(failure, reCopy.task)); }
    }
  };
  const cancel = async () => {
    if (!currentScope || task.status !== "running" || !canCancelRetargetInstallTaskPhase(task.phase)) return;
    try { await cancelRetargetInstallTask({ taskId: task.taskId }); }
    catch (failure) { if (stillHere()) setError(getReinstallStartErrorMessage(failure, reCopy.task)); }
  };
  const visibleTask: RetargetInstallTaskState = currentScope ? task : { status: "idle" };
  const visiblePreview = currentScope ? preview : null;
  const visibleError = currentScope ? error : null;
  const visibleLoading = currentScope && loading;
  const previewStatus: "idle" | "loading" | "ready" | "error" = visibleLoading ? "loading" : visibleError ? "error" : visiblePreview || visibleTask.status !== "idle" ? "ready" : "idle";
  return { installed: currentScope && installed, preview: visiblePreview, loading: visibleLoading, error: visibleError,
    task: visibleTask, active, listener, disabled, previewStatus,
    canPreview: currentScope && !disabled && installed && !active && !loading && plugins.ready,
    canApply: currentScope && !disabled && installed && !active && plugins.ready && listener === "ready" && preview?.status === "ready",
    generatePreview, start, cancel, retryListener: () => setListenAttempt((value) => value + 1) };
}

export type PluginReapplyWorkflow = ReturnType<typeof usePluginReapply>;
