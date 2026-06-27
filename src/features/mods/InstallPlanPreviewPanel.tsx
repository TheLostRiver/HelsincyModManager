import { AlertTriangle, FileCheck2, Loader2, X } from "lucide-react";
import type { InstallPlanPreview, InstallRecoveryIssueSummary } from "./modInstallPlanTypes";
import "./InstallPlanPreviewPanel.css";

export type InstallPlanPreviewPanelState =
  | { status: "idle" }
  | { status: "loading"; modName: string }
  | { status: "ready"; modName: string; plan: InstallPlanPreview }
  | { status: "error"; modName: string; message: string }
  | {
      status: "recovery-required";
      modName: string;
      recoveryStatus: "rollback_required" | "repair_required" | "unknown";
      managedFileCount: number;
      backupCount: number;
      issueCount: number;
      issues: InstallRecoveryIssueSummary[];
    }
  | { status: "uninstall-confirming"; modName: string; managedFileCount: number; backupCount: number }
  | { status: "install-starting"; modName: string; phaseLabel: string }
  | { status: "install-running"; modName: string; phaseLabel: string }
  | { status: "install-completed"; modName: string; phaseLabel: string }
  | { status: "install-failed"; modName: string; phaseLabel: string; message: string }
  | { status: "install-cancelled"; modName: string; phaseLabel: string }
  | { status: "uninstall-starting"; modName: string; phaseLabel: string }
  | { status: "uninstall-running"; modName: string; phaseLabel: string }
  | { status: "uninstall-completed"; modName: string; phaseLabel: string }
  | { status: "uninstall-failed"; modName: string; phaseLabel: string; message: string };

type InstallPlanPreviewPanelProps = {
  state: InstallPlanPreviewPanelState;
  onClose: () => void;
  onConfirmUninstall?: () => void;
  onCancelUninstall?: () => void;
  closeDisabled?: boolean;
};

function panelTitle(state: InstallPlanPreviewPanelState) {
  if (state.status === "ready" && state.plan.hasBlockingConflicts) {
    return "安装计划存在冲突";
  }

  switch (state.status) {
    case "recovery-required":
      return state.recoveryStatus === "rollback_required"
        ? "需要回滚"
        : state.recoveryStatus === "unknown"
          ? "安装状态未知"
          : "需要人工处理";
    case "uninstall-confirming":
      return "确认卸载";
    case "uninstall-completed":
      return "卸载完成";
    case "uninstall-failed":
      return "卸载失败";
    case "install-completed":
      return "安装完成";
    case "install-failed":
      return "安装失败";
    case "install-cancelled":
      return "安装已取消";
    default:
      if (state.status.startsWith("uninstall-")) {
        return "卸载任务";
      }
      if (state.status.startsWith("install-")) {
        return "安装任务";
      }
      return "安装计划预览";
  }
}

export function InstallPlanPreviewPanel({
  state,
  onClose,
  onConfirmUninstall,
  onCancelUninstall,
  closeDisabled = false,
}: InstallPlanPreviewPanelProps) {
  if (state.status === "idle") {
    return null;
  }

  const title = panelTitle(state);
  const isLoading =
    state.status === "loading" ||
    state.status === "install-starting" ||
    state.status === "install-running" ||
    state.status === "uninstall-starting" ||
    state.status === "uninstall-running";
  const isWarning =
    state.status === "error" ||
    state.status === "recovery-required" ||
    state.status === "install-failed" ||
    state.status === "install-cancelled" ||
    state.status === "uninstall-confirming" ||
    state.status === "uninstall-failed" ||
    (state.status === "ready" && state.plan.hasBlockingConflicts);
  const isCompleted = state.status === "install-completed" || state.status === "uninstall-completed";

  return (
    <section className="install-plan-preview" aria-live="polite">
      <header className="install-plan-preview__header">
        <div className="install-plan-preview__title-group">
          {isLoading ? (
            <Loader2 className="install-plan-preview__icon is-loading" size={18} aria-hidden="true" />
          ) : isWarning ? (
            <AlertTriangle className="install-plan-preview__icon is-warning" size={18} aria-hidden="true" />
          ) : (
            <FileCheck2
              className={`install-plan-preview__icon ${isCompleted ? "is-completed" : "is-ready"}`}
              size={18}
              aria-hidden="true"
            />
          )}
          <div>
            <h3 className="install-plan-preview__title">{title}</h3>
            <p className="install-plan-preview__mod-name">{state.modName}</p>
          </div>
        </div>
        <button
          type="button"
          className="install-plan-preview__close"
          onClick={onClose}
          aria-label="关闭安装计划预览"
          disabled={closeDisabled}
        >
          <X size={16} aria-hidden="true" />
        </button>
      </header>

      {state.status === "loading" ? (
        <p className="install-plan-preview__status">生成中</p>
      ) : null}

      {state.status === "error" ? (
        <p className="install-plan-preview__status is-error">{state.message}</p>
      ) : null}

      {state.status === "recovery-required" ? (
        <RecoveryRequiredSummary
          recoveryStatus={state.recoveryStatus}
          managedFileCount={state.managedFileCount}
          backupCount={state.backupCount}
          issueCount={state.issueCount}
          issues={state.issues}
        />
      ) : null}

      {state.status === "uninstall-confirming" ? (
        <div className="install-plan-preview__body">
          <p className="install-plan-preview__status">将删除或恢复此 Mod 的托管文件。</p>
          <div className="install-plan-preview__metrics" aria-label="卸载影响摘要">
            <span>
              <strong>{state.managedFileCount}</strong>
              托管文件
            </span>
            <span>
              <strong>{state.backupCount}</strong>
              备份恢复点
            </span>
          </div>
          <div className="install-plan-preview__actions">
            <button type="button" className="install-plan-preview__action" onClick={onCancelUninstall ?? onClose}>
              取消
            </button>
            <button
              type="button"
              className="install-plan-preview__action is-danger"
              onClick={onConfirmUninstall}
              disabled={!onConfirmUninstall}
            >
              确认卸载
            </button>
          </div>
        </div>
      ) : null}

      {state.status === "install-starting" ||
      state.status === "install-running" ||
      state.status === "install-completed" ||
      state.status === "install-cancelled" ||
      state.status === "uninstall-starting" ||
      state.status === "uninstall-running" ||
      state.status === "uninstall-completed" ? (
        <p className="install-plan-preview__status">{state.phaseLabel}</p>
      ) : null}

      {state.status === "install-failed" || state.status === "uninstall-failed" ? (
        <>
          <p className="install-plan-preview__status is-error">{state.phaseLabel}</p>
          <p className="install-plan-preview__status is-error">{state.message}</p>
        </>
      ) : null}

      {state.status === "ready" ? <InstallPlanPreviewSummary plan={state.plan} /> : null}
    </section>
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

function RecoveryRequiredSummary({
  recoveryStatus,
  managedFileCount,
  backupCount,
  issueCount,
  issues,
}: {
  recoveryStatus: "rollback_required" | "repair_required" | "unknown";
  managedFileCount: number;
  backupCount: number;
  issueCount: number;
  issues: InstallRecoveryIssueSummary[];
}) {
  return (
    <div className="install-plan-preview__body">
      <p className="install-plan-preview__status is-error">
        {recoveryStatus === "rollback_required"
          ? "恢复记录显示上次写入未确认完成。请保留现场，等待受控回滚流程。"
          : "恢复扫描发现当前安装状态不能安全自动处理。请先人工确认后再安装或卸载。"}
      </p>
      <div className="install-plan-preview__metrics" aria-label="恢复扫描摘要">
        <span>
          <strong>{managedFileCount}</strong>
          托管文件
        </span>
        <span>
          <strong>{backupCount}</strong>
          备份恢复点
        </span>
        <span data-conflict={issueCount > 0 ? "true" : "false"}>
          <strong>{issueCount}</strong>
          检查项
        </span>
      </div>
      {issues.length > 0 ? (
        <div className="install-plan-preview__list" aria-label="恢复扫描问题摘要">
          {issues.map((issue) => (
            <code key={issue.issue}>
              {recoveryIssueLabels[issue.issue]} × {issue.count}
            </code>
          ))}
        </div>
      ) : null}
    </div>
  );
}

function InstallPlanPreviewSummary({ plan }: { plan: InstallPlanPreview }) {
  const previewActions = plan.actions.slice(0, 5);
  const previewConflicts = plan.conflicts.slice(0, 3);

  return (
    <div className="install-plan-preview__body">
      <div className="install-plan-preview__metrics" aria-label="安装计划统计">
        <span>
          <strong>{plan.actions.length}</strong>
          可执行动作
        </span>
        <span data-conflict={plan.hasBlockingConflicts ? "true" : "false"}>
          <strong>{plan.conflicts.length}</strong>
          阻断冲突
        </span>
      </div>

      {previewActions.length > 0 ? (
        <div className="install-plan-preview__list" aria-label="目标路径预览">
          {previewActions.map((action) => (
            <code key={`${action.modId}:${action.packageFileId}:${action.targetPath}`}>{action.targetPath}</code>
          ))}
        </div>
      ) : (
        <p className="install-plan-preview__status">没有可执行动作</p>
      )}

      {previewConflicts.length > 0 ? (
        <div className="install-plan-preview__conflicts" aria-label="冲突路径预览">
          {previewConflicts.map((conflict) => (
            <code key={conflict.targetPath}>{conflict.targetPath}</code>
          ))}
        </div>
      ) : null}
    </div>
  );
}
