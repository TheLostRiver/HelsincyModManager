import { AlertTriangle, CheckCircle2, FileCheck2, Loader2, Trash2, X } from "lucide-react";
import { useRef } from "react";
import {
  DetailSheet,
  Dialog,
  TaskNotice,
  TaskNoticeViewport,
  ToastViewport,
} from "../../shared/feedback";
import type { InstallPlanPreview, InstallRecoveryIssueSummary, UnsafeInstallStatus } from "./modInstallPlanTypes";
import { getManagedInstallTaskPhaseLabel, type ManagedInstallTaskState } from "./modInstallTaskState";
import type { ModLifecycleToast } from "./modLifecycleFeedbackState";
import "./ModLifecycleFeedback.css";

export type InstallPlanDetailSheetState =
  | { status: "idle" }
  | { status: "loading"; modName: string }
  | { status: "ready"; modName: string; plan: InstallPlanPreview }
  | { status: "error"; modName: string; message: string }
  | {
      status: "recovery-required";
      modName: string;
      recoveryStatus: UnsafeInstallStatus;
      managedFileCount: number;
      backupCount: number;
      issueCount: number;
      issues: InstallRecoveryIssueSummary[];
    };

export type UninstallConfirmationState = {
  modId: string;
  modName: string;
  managedFileCount: number;
  backupCount: number;
};

type InstallPlanDetailSheetProps = {
  state: InstallPlanDetailSheetState;
  onClose: () => void;
};

function recoveryTitle(status: UnsafeInstallStatus) {
  switch (status) {
    case "rollback_required":
      return "需要回滚";
    case "committed_cleanup_pending":
      return "重装待收尾";
    case "cleanup_pending":
      return "恢复待清理";
    case "unknown":
      return "安装状态未知";
    case "repair_required":
      return "需要人工处理";
  }
}

function sheetTitle(state: Exclude<InstallPlanDetailSheetState, { status: "idle" }>) {
  if (state.status === "recovery-required") {
    return recoveryTitle(state.recoveryStatus);
  }
  if (state.status === "ready" && state.plan.hasBlockingConflicts) {
    return "安装计划存在冲突";
  }
  return "安装计划预览";
}

export function InstallPlanDetailSheet({ state, onClose }: InstallPlanDetailSheetProps) {
  if (state.status === "idle") {
    return null;
  }

  const warning =
    state.status === "error" ||
    state.status === "recovery-required" ||
    (state.status === "ready" && state.plan.hasBlockingConflicts);
  const icon = state.status === "loading"
    ? <Loader2 className="mod-lifecycle-feedback__spinner" size={20} />
    : warning
      ? <AlertTriangle size={20} />
      : <FileCheck2 size={20} />;

  return (
    <DetailSheet
      open
      title={sheetTitle(state)}
      description={state.modName}
      icon={icon}
      onClose={onClose}
      closeLabel="关闭安装计划"
    >
      {state.status === "loading" ? (
        <p className="mod-lifecycle-feedback__status" role="status">正在生成安装计划</p>
      ) : null}
      {state.status === "error" ? (
        <p className="mod-lifecycle-feedback__status is-danger" role="alert">{state.message}</p>
      ) : null}
      {state.status === "recovery-required" ? <RecoveryRequiredSummary state={state} /> : null}
      {state.status === "ready" ? <InstallPlanSummary plan={state.plan} /> : null}
    </DetailSheet>
  );
}

const recoveryIssueLabels: Record<InstallRecoveryIssueSummary["issue"], string> = {
  missing_installed_file_summary: "缺少安装摘要",
  target_missing: "目标缺失",
  target_changed: "目标已变化",
  target_read_failed: "目标读取失败",
  backup_missing: "备份缺失",
  backup_read_failed: "备份读取失败",
};

function recoveryStatusMessage(status: UnsafeInstallStatus) {
  switch (status) {
    case "rollback_required":
      return "恢复记录显示上次写入未确认完成。请保留现场，前往恢复中心执行受控处理。";
    case "committed_cleanup_pending":
      return "新版本已提交，但完成记录尚未收敛。状态收敛前不要安装、卸载或重装。";
    case "cleanup_pending":
      return "重装事务已完成，但恢复数据尚待清理。清理完成前不要继续写入操作。";
    case "unknown":
      return "恢复扫描无法确认当前安装状态。请保留现场并重新扫描。";
    case "repair_required":
      return "当前安装状态不能安全自动处理。请先在恢复中心确认。";
  }
}

function RecoveryRequiredSummary({
  state,
}: {
  state: Extract<InstallPlanDetailSheetState, { status: "recovery-required" }>;
}) {
  return (
    <section className="mod-lifecycle-feedback__section" aria-label="恢复扫描摘要">
      <p className="mod-lifecycle-feedback__status is-danger">{recoveryStatusMessage(state.recoveryStatus)}</p>
      <SummaryMetrics
        items={[
          { label: "托管文件", value: state.managedFileCount },
          { label: "备份恢复点", value: state.backupCount },
          { label: "检查项", value: state.issueCount, danger: state.issueCount > 0 },
        ]}
      />
      {state.issues.length > 0 ? (
        <ul className="mod-lifecycle-feedback__rows" aria-label="恢复扫描问题">
          {state.issues.map((issue) => (
            <li key={issue.issue}>
              <span>{recoveryIssueLabels[issue.issue]}</span>
              <strong>{issue.count}</strong>
            </li>
          ))}
        </ul>
      ) : null}
    </section>
  );
}

function InstallPlanSummary({ plan }: { plan: InstallPlanPreview }) {
  const previewActions = plan.actions.slice(0, 5);
  const previewConflicts = plan.conflicts.slice(0, 3);

  return (
    <section className="mod-lifecycle-feedback__section" aria-label="安装计划详情">
      <SummaryMetrics
        items={[
          { label: "可执行动作", value: plan.actions.length },
          { label: "阻断冲突", value: plan.conflicts.length, danger: plan.hasBlockingConflicts },
        ]}
      />
      {previewActions.length > 0 ? (
        <div className="mod-lifecycle-feedback__paths" aria-label="目标路径预览">
          {previewActions.map((action) => (
            <code key={`${action.modId}:${action.packageFileId}:${action.targetPath}`}>{action.targetPath}</code>
          ))}
        </div>
      ) : (
        <p className="mod-lifecycle-feedback__status">没有可执行动作</p>
      )}
      {previewConflicts.length > 0 ? (
        <div className="mod-lifecycle-feedback__paths is-danger" aria-label="冲突路径预览">
          {previewConflicts.map((conflict) => <code key={conflict.targetPath}>{conflict.targetPath}</code>)}
        </div>
      ) : null}
    </section>
  );
}

function SummaryMetrics({
  items,
}: {
  items: Array<{ label: string; value: number; danger?: boolean }>;
}) {
  return (
    <dl className="mod-lifecycle-feedback__metrics">
      {items.map((item) => (
        <div key={item.label} data-danger={item.danger || undefined}>
          <dt>{item.label}</dt>
          <dd>{item.value}</dd>
        </div>
      ))}
    </dl>
  );
}

type UninstallConfirmationDialogProps = {
  state: UninstallConfirmationState | null;
  blockerMessage: string | null;
  onCancel: () => void;
  onConfirm: () => void;
};

export function UninstallConfirmationDialog({
  state,
  blockerMessage,
  onCancel,
  onConfirm,
}: UninstallConfirmationDialogProps) {
  const cancelButtonRef = useRef<HTMLButtonElement | null>(null);
  if (state === null) {
    return null;
  }

  return (
    <Dialog
      open
      title="确认卸载"
      description={state.modName}
      icon={<AlertTriangle size={20} />}
      onClose={onCancel}
      closeLabel="取消卸载"
      closeOnBackdrop={false}
      initialFocusRef={cancelButtonRef}
      role="alertdialog"
      footer={
        <>
          <button ref={cancelButtonRef} type="button" className="mod-lifecycle-feedback__button" onClick={onCancel}>
            取消
          </button>
          <button
            type="button"
            className="mod-lifecycle-feedback__button is-danger"
            onClick={onConfirm}
            disabled={blockerMessage !== null}
          >
            <Trash2 size={16} aria-hidden="true" />
            确认卸载
          </button>
        </>
      }
    >
      <div className="mod-lifecycle-feedback__dialog-copy">
        <p>将删除本工具新增的托管文件，并从受控备份恢复被覆盖文件。</p>
        <SummaryMetrics
          items={[
            { label: "托管文件", value: state.managedFileCount },
            { label: "备份恢复点", value: state.backupCount },
          ]}
        />
        {blockerMessage ? (
          <p className="mod-lifecycle-feedback__status is-danger" role="alert">{blockerMessage}</p>
        ) : null}
      </div>
    </Dialog>
  );
}

type ManagedInstallTaskFeedbackProps = {
  taskState: ManagedInstallTaskState;
  toast: ModLifecycleToast | null;
  onDismissToast: () => void;
};

export function ManagedInstallTaskFeedback({
  taskState,
  toast,
  onDismissToast,
}: ManagedInstallTaskFeedbackProps) {
  const runningTask = taskState.status === "running" ? taskState : null;

  return (
    <>
      {runningTask ? (
        <TaskNoticeViewport label="Mod 任务进度">
          <TaskNotice
            taskId={runningTask.taskId}
            title={runningTask.operation === "uninstall" ? "正在卸载 Mod" : "正在安装 Mod"}
            message={`${runningTask.modName} · ${getManagedInstallTaskPhaseLabel(runningTask.phase)}`}
            tone="progress"
          >
            <div className="mod-lifecycle-feedback__task-progress" aria-hidden="true">
              <span />
            </div>
          </TaskNotice>
        </TaskNoticeViewport>
      ) : null}

      {toast ? (
        <ToastViewport label="Mod 操作通知">
          <article className={`mod-lifecycle-feedback__toast is-${toast.tone}`} data-toast-id={toast.id}>
            <span className="mod-lifecycle-feedback__toast-icon" aria-hidden="true">
              {toast.tone === "success" ? <CheckCircle2 size={18} /> : <AlertTriangle size={18} />}
            </span>
            <div>
              <strong>{toast.title}</strong>
              <p>{toast.message}</p>
            </div>
            <button type="button" onClick={onDismissToast} aria-label="关闭通知" title="关闭通知">
              <X size={16} aria-hidden="true" />
            </button>
          </article>
        </ToastViewport>
      ) : null}
    </>
  );
}
