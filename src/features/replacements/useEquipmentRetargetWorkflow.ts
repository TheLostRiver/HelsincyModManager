import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";
import { TASK_PROGRESS_EVENT_NAME, type TaskProgressEventDto } from "../mods/modImportTypes";
import type { ReinstallPlanPreview } from "../mods/modReinstallTypes";
import { getEquipmentRetargetConfiguration, previewEquipmentRetargetInstall, previewEquipmentRetargetReinstall,
  startEquipmentRetargetInstall, startEquipmentRetargetReinstall } from "./equipmentRetargetApi";
import { previewEquipmentReapply, startEquipmentReapply } from "./equipmentRetargetApi";
import { equipmentSlotIntents, initialEquipmentChoices } from "./equipmentRetargetTypes";
import type { EquipmentRetargetConfiguration, EquipmentRetargetInstallPreview, EquipmentRetargetSelection, EquipmentTargetChoice } from "./equipmentRetargetTypes";
import type { EquipmentReapplyInput } from "./equipmentRetargetTypes";
import { cancelRetargetInstallTask } from "./replacementApi";
import type { ReplacementCopy } from "./replacementCopy";
import { replacementErrorMessage } from "./replacementErrorText";
import { canCancelRetargetInstallTaskPhase, installBlockMessage, isRetargetInstallTaskPhase, nextRetargetInstallTaskState, type RetargetInstallTaskState } from "./replacementWorkflow";
import type { ReplacementTargetPanelProps } from "./ReplacementTargetPanel";

type Preview =
  | { status: "idle" | "loading" }
  | { status: "error"; message: string }
  | { status: "ready"; mode: "initial"; request: EquipmentRetargetSelection; value: EquipmentRetargetInstallPreview }
  | { status: "ready"; mode: "switch"; request: EquipmentRetargetSelection; value: ReinstallPlanPreview }
  | { status: "ready"; mode: "reapply"; request: EquipmentReapplyInput; value: ReinstallPlanPreview };

export function useEquipmentRetargetWorkflow(props: ReplacementTargetPanelProps, initial: EquipmentRetargetConfiguration, copy: ReplacementCopy) {
  const [configuration, setConfiguration] = useState(initial);
  const [choices, setChoices] = useState(() => initialEquipmentChoices(initial));
  const [preview, setPreview] = useState<Preview>({ status: "idle" });
  const [task, setTask] = useState<RetargetInstallTaskState>({ status: "idle" });
  const taskRef = useRef(task);
  const [listener, setListener] = useState<"connecting" | "ready" | "failed">("connecting");
  const [listenerAttempt, setListenerAttempt] = useState(0);
  const [refresh, setRefresh] = useState<"idle" | "refreshing" | "failed">("idle");
  const [cancel, setCancel] = useState<"idle" | "requesting">("idle");
  const [cancelError, setCancelError] = useState<string | null>(null);
  const latest = useRef({ props, copy });
  latest.current = { props, copy };
  const lifetime = useRef(0);
  const previewGeneration = useRef(0);
  const pendingEvents = useRef(new Map<string, TaskProgressEventDto>());
  const refreshedTask = useRef<string | null>(null);

  const trackTask = useCallback((next: RetargetInstallTaskState) => {
    taskRef.current = next;
    setTask(next);
  }, []);

  useEffect(() => () => { lifetime.current += 1; previewGeneration.current += 1; }, []);
  useEffect(() => { previewGeneration.current += 1; setPreview({ status: "idle" }); }, [props.installStatus, props.completedLocally]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    setListener("connecting");
    void listen<TaskProgressEventDto>(TASK_PROGRESS_EVENT_NAME, ({ payload }) => {
      if (disposed || payload.kind !== "install" || !isRetargetInstallTaskPhase(payload.phase)) return;
      const current = taskRef.current;
      if (current.status === "starting") {
        const previous = pendingEvents.current.get(payload.taskId);
        if (previous && ["completed", "failed", "cancelled"].includes(previous.status)) return;
        pendingEvents.current.set(payload.taskId, payload);
      } else {
        trackTask(nextRetargetInstallTaskState(current, payload, latest.current.copy.events));
      }
    }).then((dispose) => {
      if (disposed) dispose(); else { unlisten = dispose; setListener("ready"); }
    }).catch(() => { if (!disposed) setListener("failed"); });
    return () => { disposed = true; unlisten?.(); };
  }, [listenerAttempt, trackTask]);

  const refreshCompleted = useCallback(async () => {
    const generation = lifetime.current;
    setRefresh("refreshing");
    try {
      await latest.current.props.onInstallCompleted();
      if (lifetime.current !== generation) return;
      const next = await getEquipmentRetargetConfiguration(latest.current.props);
      if (lifetime.current !== generation) return;
      setConfiguration(next);
      setChoices(initialEquipmentChoices(next));
      setPreview({ status: "idle" });
      trackTask({ status: "idle" });
      setRefresh("idle");
    } catch {
      if (lifetime.current === generation) setRefresh("failed");
    }
  }, [trackTask]);

  useEffect(() => {
    if (task.status !== "completed" || refreshedTask.current === task.taskId) return;
    refreshedTask.current = task.taskId;
    void refreshCompleted();
  }, [task, refreshCompleted]);

  const taskActive = task.status === "starting" || task.status === "running";
  const busy = taskActive || refresh === "refreshing" || (task.status === "completed" && refresh === "idle");
  const { onBusyChange } = props;
  useEffect(() => onBusyChange(busy), [onBusyChange, busy]);
  useEffect(() => () => latest.current.props.onBusyChange(false), []);

  const block = installBlockMessage(props.profileId, props.installStatus, props.completedLocally || task.status === "completed", copy.block)
    ?? (props.installStatus === "installed" && configuration.installedTargets === null ? copy.panel.currentTargetsUnknown : null);
  const validChoices = configuration.sources.every(({ source, targets, originalTargetId }) => {
    const choice = choices[source.id];
    return choice ? targets.some((target) => target.id === choice.targetId) : originalTargetId !== null;
  });
  const canPreview = !busy && block === null && validChoices;
  const canReapply = !busy && block === null && props.profileId !== null && props.installStatus === "installed";
  const canStart = (preview.status === "ready" && preview.mode === "reapply" ? canReapply : canPreview) && listener === "ready" && preview.status === "ready"
    && (preview.mode === "initial"
      ? props.installStatus === "not_installed" && !preview.value.installPlan.hasBlockingConflicts && preview.value.prerequisiteDecision.status !== "blocked"
      : props.installStatus === "installed" && preview.value.status === "ready");

  const choose = (sourceId: string, choice: EquipmentTargetChoice) => {
    if (busy || task.status === "completed") return;
    previewGeneration.current += 1;
    setChoices((current) => ({ ...current, [sourceId]: choice }));
    setPreview({ status: "idle" });
    trackTask({ status: "idle" });
    setCancelError(null);
  };

  const createPreview = async () => {
    if (!canPreview || props.profileId === null) return;
    const generation = ++previewGeneration.current;
    const request: EquipmentRetargetSelection = {
      gameId: props.gameId, profileId: props.profileId, modId: props.modId,
      slots: equipmentSlotIntents(configuration, choices), layerName: "base", layerPriority: 0,
    };
    setPreview({ status: "loading" });
    trackTask({ status: "idle" });
    try {
      const next: Preview = props.installStatus === "installed"
        ? { status: "ready", mode: "switch", request, value: await previewEquipmentRetargetReinstall(request) }
        : { status: "ready", mode: "initial", request, value: await previewEquipmentRetargetInstall(request) };
      if (previewGeneration.current === generation) setPreview(next);
    } catch (error) {
      if (previewGeneration.current === generation) setPreview({ status: "error", message: replacementErrorMessage(error, latest.current.copy.events.previewFallback, latest.current.copy.errors) });
    }
  };

  const createReapplyPreview = async () => {
    if (!canReapply || props.profileId === null) return;
    const generation = ++previewGeneration.current;
    const request = { gameId: props.gameId, profileId: props.profileId, modId: props.modId };
    setPreview({ status: "loading" });
    trackTask({ status: "idle" });
    try {
      const value = await previewEquipmentReapply(request);
      if (previewGeneration.current === generation) setPreview({ status: "ready", mode: "reapply", request, value });
    } catch (error) {
      if (previewGeneration.current === generation) setPreview({ status: "error", message: replacementErrorMessage(error, latest.current.copy.events.previewFallback, latest.current.copy.errors) });
    }
  };

  const start = async () => {
    if (!canStart || preview.status !== "ready" || taskRef.current.status === "starting" || taskRef.current.status === "running") return;
    const generation = lifetime.current;
    const switching = preview.mode !== "initial";
    const failedPhase = switching ? "install.reinstall.failed" : "install.retarget.failed";
    pendingEvents.current.clear();
    setCancelError(null);
    trackTask({ status: "starting" });
    try {
      const started = preview.mode === "reapply" && preview.value.status === "ready"
        ? await startEquipmentReapply(preview.request, preview.value.planToken)
        : preview.mode === "switch" && preview.value.status === "ready"
        ? await startEquipmentRetargetReinstall(preview.request, preview.value.planToken)
        : preview.mode === "initial" ? await startEquipmentRetargetInstall(preview.request) : null;
      if (lifetime.current !== generation) return;
      if (!started || started.kind !== "install" || started.status !== "queued") throw { code: "invalid_task_type" };
      const running: RetargetInstallTaskState = { status: "running", taskId: started.taskId, phase: switching ? "install.reinstall.queued" : "install.retarget.queued" };
      const early = pendingEvents.current.get(started.taskId);
      pendingEvents.current.clear();
      trackTask(early ? nextRetargetInstallTaskState(running, early, latest.current.copy.events) : running);
    } catch (error) {
      if (lifetime.current !== generation) return;
      pendingEvents.current.clear();
      trackTask({ status: "failed", taskId: null, phase: failedPhase, message: replacementErrorMessage(error, latest.current.copy.events.startFailed, latest.current.copy.errors) });
    }
    if (lifetime.current === generation) setPreview({ status: "idle" });
  };

  const cancelTask = async () => {
    const current = taskRef.current;
    if (current.status !== "running" || cancel === "requesting" || !canCancelRetargetInstallTaskPhase(current.phase)) return;
    const generation = lifetime.current;
    setCancel("requesting");
    setCancelError(null);
    try {
      const result = await cancelRetargetInstallTask({ taskId: current.taskId });
      if (lifetime.current !== generation) return;
      if (result.taskId !== current.taskId || result.kind !== "install" || result.status !== "cancelled") throw { code: "invalid_cancel_result" };
      if (taskRef.current.status === "running" && taskRef.current.taskId === current.taskId) {
        trackTask({ status: "cancelled", taskId: current.taskId, phase: current.phase.startsWith("install.reinstall.") ? "install.reinstall.cancelled" : "install.cancelled" });
      }
    } catch (error) {
      if (lifetime.current === generation && taskRef.current.status === "running") setCancelError(replacementErrorMessage(error, latest.current.copy.events.cancelFailed, latest.current.copy.errors));
    } finally {
      if (lifetime.current === generation) setCancel("idle");
    }
  };

  return { configuration, choices, choose, preview, task, busy, block, canPreview, canReapply, canStart, createPreview, createReapplyPreview, start,
    listener, retryListener: () => setListenerAttempt((value) => value + 1), refresh, refreshCompleted, cancel, cancelError, cancelTask };
}
