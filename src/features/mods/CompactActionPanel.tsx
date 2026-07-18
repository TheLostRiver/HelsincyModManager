import {
  Ban,
  BadgeCheck,
  CheckCheck,
  ClipboardList,
  Download,
  Plus,
  RefreshCcw,
  RotateCw,
  Shuffle,
  Trash2,
} from "lucide-react";
import type { ComponentType } from "react";
import type { LucideProps } from "lucide-react";
import { ModImportAction } from "./ModImportAction";
import { ModLibraryControlTooltip } from "./ModLibraryControlTooltip";
import {
  getCompactActionDisabledReason,
  MOD_LIBRARY_QUERY_BUSY_MESSAGE,
} from "./compactActionAvailability";
import { compactActions } from "./modsLibraryData";

type CompactActionPanelProps = {
  selectedCount: number;
  totalCount: number;
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
  "enable-all": BadgeCheck,
  "disable-all": Ban,
  "preview-plan": ClipboardList,
  install: Download,
  reinstall: RefreshCcw,
  uninstall: Trash2,
};

export function CompactActionPanel({
  selectedCount,
  totalCount,
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
  const addAction = compactActions.find((a) => a.id === "add");
  const addRevisionAction = compactActions.find((a) => a.id === "add-revision");
  const revisionImportDisabledReason =
    libraryQueryBusy
      ? MOD_LIBRARY_QUERY_BUSY_MESSAGE
      : selectedCount !== 1 || !selectedModId
      ? "请先选择一个 MOD"
      : installTaskActive
        ? "请等待当前安装任务完成"
        : undefined;

  return (
    <aside className="compact-panel" aria-label="快捷操作">
      <header className="compact-panel__header">
        <h3 className="compact-panel__title">快捷操作</h3>
        <span className="compact-panel__selected-pill">已选 {selectedCount}</span>
      </header>

      <div className="compact-panel__stack">
        {addAction ? <ModImportAction label={addAction.label} onImported={onImportCompleted} /> : null}
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
              const disabledReason = getCompactActionDisabledReason({
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
            const disabledReason = getCompactActionDisabledReason({
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

        <div className="compact-panel__spacer" aria-hidden="true" />
        <span className="compact-panel__selection-status" role="status" aria-live="polite" aria-atomic="true">
          本页已选 {selectedCount} / 当前页 {totalCount} 项
        </span>
      </div>
    </aside>
  );
}
