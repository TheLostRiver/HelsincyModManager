import {
  CheckCheck,
  ClipboardList,
  Download,
  ListChecks,
  Plus,
  RefreshCcw,
  RotateCw,
  Shuffle,
  Trash2,
  X,
} from "lucide-react";
import type { ComponentType } from "react";
import type { LucideProps } from "lucide-react";
import { ExternalImportAction } from "./external-import/ExternalImportAction";
import { ModImportAction } from "./ModImportAction";
import { ModLibraryControlTooltip } from "./ModLibraryControlTooltip";
import {
  getCompactActionDisabledReason,
  MOD_LIBRARY_QUERY_BUSY_MESSAGE,
} from "./compactActionAvailability";
import { compactActions } from "./modsLibraryData";
import { MAX_MOD_SELECTION_COUNT, type ModSelectionMode } from "./modSelection";

type CompactActionPanelProps = {
  selectionMode: ModSelectionMode;
  selectedCount: number;
  selectedPageCount: number;
  pageCount: number;
  selectionNotice?: string | null;
  selectionInteractionDisabledReason?: string;
  batchPreviewUnavailableReason?: string;
  batchWriteUnavailableReason?: string;
  selectedModId?: string | null;
  installTaskActive?: boolean;
  libraryQueryBusy: boolean;
  profileReady: boolean;
  canInstallSelection?: boolean;
  canReinstallSelection?: boolean;
  canUninstallSelection?: boolean;
  onImportCompleted: () => Promise<void> | void;
  onAction: (actionId: string) => void;
};

const actionIcons: Record<string, ComponentType<LucideProps>> = {
  add: Plus,
  "select-all": CheckCheck,
  invert: Shuffle,
  refresh: RotateCw,
  "preview-plan": ClipboardList,
  install: Download,
  reinstall: RefreshCcw,
  uninstall: Trash2,
};

export function CompactActionPanel({
  selectionMode,
  selectedCount,
  selectedPageCount,
  pageCount,
  selectionNotice = null,
  selectionInteractionDisabledReason,
  batchPreviewUnavailableReason,
  batchWriteUnavailableReason,
  selectedModId = null,
  installTaskActive = false,
  libraryQueryBusy,
  profileReady,
  canInstallSelection = true,
  canReinstallSelection = false,
  canUninstallSelection = false,
  onImportCompleted,
  onAction,
}: CompactActionPanelProps) {
  const batchSelectionActive = selectionMode === "batch";
  const addAction = compactActions.find((a) => a.id === "add");
  const addRevisionAction = compactActions.find((a) => a.id === "add-revision");
  const revisionImportDisabledReason =
    libraryQueryBusy
      ? MOD_LIBRARY_QUERY_BUSY_MESSAGE
      : batchSelectionActive
        ? "退出批量选择后可导入新版本"
      : selectedCount !== 1 || !selectedModId
      ? "请先选择一个 MOD"
      : installTaskActive
        ? "请等待当前安装任务完成"
        : undefined;
  const selectionSummary = batchSelectionActive
    ? `已选 ${selectedCount} / ${MAX_MOD_SELECTION_COUNT}，本页已选 ${selectedPageCount} / ${pageCount} 项`
    : selectedCount === 1
      ? "已选择 1 项"
      : "尚未选择 Mod";
  const lifecycleLabel = (actionId: string, fallback: string) => {
    if (!batchSelectionActive) {
      return fallback;
    }
    const labels: Record<string, string> = {
      "preview-plan": "预览批量计划",
      install: "批量安装",
      reinstall: "批量重装",
      uninstall: "批量卸载",
    };
    return labels[actionId] ?? fallback;
  };
  const batchCapabilityDisabledReason = (actionId: string) =>
    batchSelectionActive && actionId === "preview-plan"
      ? batchPreviewUnavailableReason
      : batchSelectionActive
        ? batchWriteUnavailableReason
        : undefined;
  const lifecycleDisabledReason = (
    actionId: string,
    fallbackReason: string | undefined,
  ) =>
    selectionInteractionDisabledReason
    ?? batchCapabilityDisabledReason(actionId)
    ?? fallbackReason;

  return (
    <aside
      className="compact-panel"
      aria-label="快捷操作"
      data-tour-id="mods.actions"
      data-selection-mode={selectionMode}
    >
      <header className="compact-panel__header">
        <h3 className="compact-panel__title">快捷操作</h3>
        <span className="compact-panel__selected-pill">已选 {selectedCount}</span>
      </header>

      <div className="compact-panel__stack">
        {addAction ? (
          <ModImportAction
            label={addAction.label}
            onImported={onImportCompleted}
            tourId="mods.import-action"
          />
        ) : null}
        <ExternalImportAction onImported={onImportCompleted} />
        {addRevisionAction ? (
          <ModImportAction
            label={addRevisionAction.label}
            mode="revision"
            modId={selectedModId}
            disabledReason={revisionImportDisabledReason}
            onImported={onImportCompleted}
          />
        ) : null}

        <div className="compact-action-group">
          {compactActions
            .filter((a) => ["select-all", "invert", "refresh"].includes(a.id))
            .map((action) => {
              const Icon = actionIcons[action.id] ?? Plus;
              const disabledReason = selectionInteractionDisabledReason
                ?? getCompactActionDisabledReason({
                    actionId: action.id,
                    selectedCount,
                    profileReady,
                    installTaskActive,
                    libraryQueryBusy,
                    canInstallSelection,
                    canReinstallSelection,
                    canUninstallSelection,
                  });
              return (
                <ModLibraryControlTooltip key={action.id} content={disabledReason}>
                  {(descriptionId) => (
                    <button
                      type="button"
                      className={`compact-action is-${action.variant}`}
                      data-variant={action.variant}
                      aria-disabled={disabledReason ? true : undefined}
                      aria-describedby={descriptionId}
                      onClick={(event) => {
                        if (disabledReason) {
                          event.preventDefault();
                          event.stopPropagation();
                          return;
                        }
                        onAction(action.id);
                      }}
                    >
                      <span className="compact-action__left">
                        <Icon size={14} strokeWidth={2.4} aria-hidden="true" />
                        <span className="compact-action__label">{action.label}</span>
                      </span>
                      <span className="compact-action__dot" aria-hidden="true" />
                    </button>
                  )}
                </ModLibraryControlTooltip>
              );
            })}
        </div>

        {compactActions
          .filter((a) => !["select-all", "invert", "refresh", "add", "add-revision"].includes(a.id))
          .map((action) => {
            const Icon = actionIcons[action.id] ?? Plus;
            const disabledReason = lifecycleDisabledReason(
              action.id,
              getCompactActionDisabledReason({
                actionId: action.id,
                selectedCount,
                profileReady,
                installTaskActive,
                libraryQueryBusy,
                canInstallSelection,
                canReinstallSelection,
                canUninstallSelection,
              }),
            );
            return (
              <ModLibraryControlTooltip key={action.id} content={disabledReason}>
                {(descriptionId) => (
                  <button
                    type="button"
                    className={`compact-action is-${action.variant}`}
                    data-variant={action.variant}
                    aria-disabled={disabledReason ? true : undefined}
                    aria-describedby={descriptionId}
                    onClick={(event) => {
                      if (disabledReason) {
                        event.preventDefault();
                        event.stopPropagation();
                        return;
                      }
                      onAction(action.id);
                    }}
                  >
                    <span className="compact-action__left">
                      <Icon size={14} strokeWidth={2.4} aria-hidden="true" />
                      <span className="compact-action__label">
                        {lifecycleLabel(action.id, action.label)}
                      </span>
                    </span>
                    <span className="compact-action__dot" aria-hidden="true" />
                  </button>
                )}
              </ModLibraryControlTooltip>
              );
            })}

        <ModLibraryControlTooltip content={selectionInteractionDisabledReason}>
          {(descriptionId) => (
            <button
              type="button"
              className="compact-action is-neutral is-mode-toggle"
              aria-pressed={batchSelectionActive}
              aria-disabled={selectionInteractionDisabledReason ? true : undefined}
              aria-describedby={descriptionId}
              onClick={(event) => {
                if (selectionInteractionDisabledReason) {
                  event.preventDefault();
                  event.stopPropagation();
                  return;
                }
                onAction(batchSelectionActive ? "exit-batch-selection" : "enter-batch-selection");
              }}
            >
              <span className="compact-action__left">
                <ListChecks size={15} strokeWidth={2.4} aria-hidden="true" />
                <span className="compact-action__label">
                  {batchSelectionActive ? "退出批量选择" : "批量选择"}
                </span>
              </span>
            </button>
          )}
        </ModLibraryControlTooltip>

        {batchSelectionActive ? (
          <ModLibraryControlTooltip
            content={
              selectionInteractionDisabledReason
              ?? (selectedCount === 0 ? "当前没有已选 Mod" : undefined)
            }
          >
            {(descriptionId) => (
              <button
                type="button"
                className="compact-action is-neutral is-icon-only"
                aria-label="清空选择"
                aria-disabled={
                  selectionInteractionDisabledReason || selectedCount === 0 ? true : undefined
                }
                aria-describedby={descriptionId}
                onClick={(event) => {
                  if (selectionInteractionDisabledReason || selectedCount === 0) {
                    event.preventDefault();
                    event.stopPropagation();
                    return;
                  }
                  onAction("clear-selection");
                }}
              >
                <X size={15} strokeWidth={2.5} aria-hidden="true" />
              </button>
            )}
          </ModLibraryControlTooltip>
        ) : null}

        <div className="compact-panel__spacer" aria-hidden="true" />
        <span className="compact-panel__selection-status" role="status" aria-live="polite" aria-atomic="true">
          {batchSelectionActive ? (
            <>
              <strong>已选 {selectedCount} / {MAX_MOD_SELECTION_COUNT}</strong>
              <span>本页已选 {selectedPageCount} / {pageCount} 项</span>
            </>
          ) : (
            <span>本页已选 {selectedPageCount} / 当前页 {pageCount} 项</span>
          )}
        </span>
        {selectionNotice ? (
          <span className="compact-panel__selection-feedback">{selectionNotice}</span>
        ) : null}
        <span
          className="compact-panel__selection-announcement"
          role="status"
          aria-live="polite"
          aria-atomic="true"
        >
          {selectionNotice ?? selectionSummary}
        </span>
      </div>
    </aside>
  );
}
