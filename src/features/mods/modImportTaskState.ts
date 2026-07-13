import type { TaskProgressEventDto } from "./modImportTypes";

export type ModImportTaskState =
  | { status: "idle" }
  | { status: "choosing" }
  | { status: "starting" }
  | { status: "running"; taskId: string; phase: string }
  | { status: "completed"; taskId: string; phase: string }
  | { status: "cancelled"; taskId: string; phase: string }
  | { status: "failed"; taskId: string | null; phase: string; message: string };

const modImportPhaseLabels: Readonly<Record<string, string>> = {
  "mod_import.queued": "等待导入",
  "mod_import.cancelled": "导入已取消",
  "mod_import.unpack.started": "正在安全解包",
  "mod_import.unpack.completed": "安全解包完成",
  "mod_import.unpack.failed": "安全解包失败",
  "mod_import.preview_image.processing": "正在处理预览图",
  "mod_import.preview_image.fallback": "预览图已使用回退方案",
  "mod_import.analyze.processing": "正在分析 Mod",
  "mod_import.commit.processing": "正在保存导入结果",
  "mod_import.prepare.completed": "导入完成",
};

export function isModImportTaskPhase(phase: string) {
  return Object.hasOwn(modImportPhaseLabels, phase);
}

export function getModImportTaskPhaseLabel(phase: string) {
  return modImportPhaseLabels[phase] ?? "正在导入";
}

export function nextModImportTaskStateFromProgress(
  current: ModImportTaskState,
  event: TaskProgressEventDto,
): ModImportTaskState {
  if (
    current.status === "completed" ||
    current.status === "cancelled" ||
    current.status === "failed"
  ) {
    return current;
  }

  if (
    event.kind !== "mod_import" ||
    !isModImportTaskPhase(event.phase) ||
    !("taskId" in current) ||
    current.taskId === null ||
    current.taskId !== event.taskId
  ) {
    return current;
  }

  if (event.status === "completed") {
    return { status: "completed", taskId: event.taskId, phase: event.phase };
  }
  if (event.status === "cancelled") {
    return { status: "cancelled", taskId: event.taskId, phase: event.phase };
  }
  if (event.status === "failed") {
    return {
      status: "failed",
      taskId: event.taskId,
      phase: event.phase,
      message: "导入失败，请检查压缩包后重试",
    };
  }

  return { status: "running", taskId: event.taskId, phase: event.phase };
}
