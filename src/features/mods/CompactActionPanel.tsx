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
import { resolveCopy, useI18n } from "../../shared/i18n";
import { getCompactActionDisabledReason } from "./compactActionAvailability";
import { modLibraryCopy } from "./modLibraryCopy";
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
  /** #275: import / delete are refused while the storage root migrates or awaits a restart. */
  storageWriteFreezeReason?: string;
  selectedModId?: string | null;
  installTaskActive?: boolean;
  libraryQueryBusy: boolean;
  profileReady: boolean;
  canInstallSelection?: boolean;
  canReinstallSelection?: boolean;
  canUninstallSelection?: boolean;
  canDeleteSelection?: boolean;
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
  delete: Trash2,
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
  storageWriteFreezeReason,
  selectedModId = null,
  installTaskActive = false,
  libraryQueryBusy,
  profileReady,
  canInstallSelection = true,
  canReinstallSelection = false,
  canUninstallSelection = false,
  canDeleteSelection = false,
  onImportCompleted,
  onAction,
}: CompactActionPanelProps) {
  const { locale } = useI18n();
  const compact = resolveCopy(modLibraryCopy, locale).compact;
  // mock 数据里的 action.label 不再用于渲染：按钮文本一律从 compact.buttons 取。
  const buttonText: Record<string, string> = {
    add: compact.buttons.add,
    "add-revision": compact.buttons.addRevision,
    "select-all": compact.buttons.selectAll,
    invert: compact.buttons.invert,
    refresh: compact.buttons.refresh,
    "preview-plan": compact.buttons.previewPlan,
    install: compact.buttons.install,
    reinstall: compact.buttons.reinstall,
    uninstall: compact.buttons.uninstall,
    delete: compact.buttons.delete,
  };
  const batchSelectionActive = selectionMode === "batch";
  const addAction = compactActions.find((a) => a.id === "add");
  const addRevisionAction = compactActions.find((a) => a.id === "add-revision");
  const revisionImportDisabledReason =
    storageWriteFreezeReason !== undefined
      ? storageWriteFreezeReason
      : libraryQueryBusy
      ? compact.queryBusy
      : batchSelectionActive
        ? compact.exitBatchToImportRevision
      : selectedCount !== 1 || !selectedModId
      ? compact.selectOneFirst
      : installTaskActive
        ? compact.waitInstallTask
        : undefined;
  const selectionSummary = batchSelectionActive
    ? compact.selectedSummary(selectedCount, MAX_MOD_SELECTION_COUNT, selectedPageCount, pageCount)
    : selectedCount === 1
      ? compact.selectedOne
      : compact.noneSelected;
  const lifecycleLabel = (actionId: string, fallback: string) => {
    if (!batchSelectionActive) {
      return fallback;
    }
    const labels: Record<string, string> = {
      "preview-plan": compact.batchActionLabels.previewPlan,
      install: compact.batchActionLabels.install,
      reinstall: compact.batchActionLabels.reinstall,
      uninstall: compact.batchActionLabels.uninstall,
      delete: compact.batchActionLabels.delete,
    };
    return labels[actionId] ?? fallback;
  };
  const batchCapabilityDisabledReason = (actionId: string) => {
    // Delete is a page-side loop over the single delete command and never enters the batch
    // lifecycle framework, so the sandbox-gated write capability does not apply to it; the
    // storage write freeze does (deletion reclaims a package sandbox).
    if (actionId === "delete") {
      return storageWriteFreezeReason;
    }
    return batchSelectionActive && actionId === "preview-plan"
      ? batchPreviewUnavailableReason
      : batchSelectionActive
        ? batchWriteUnavailableReason
        : undefined;
  };
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
      aria-label={compact.title}
      data-tour-id="mods.actions"
      data-selection-mode={selectionMode}
    >
      <header className="compact-panel__header">
        <h3 className="compact-panel__title">{compact.title}</h3>
        <span className="compact-panel__selected-pill">{compact.selectedPill(selectedCount)}</span>
      </header>

      <div className="compact-panel__stack">
        {addAction ? (
          <ModImportAction
            label={buttonText.add}
            disabledReason={storageWriteFreezeReason}
            onImported={onImportCompleted}
            tourId="mods.import-action"
          />
        ) : null}
        <ExternalImportAction onImported={onImportCompleted} disabledReason={storageWriteFreezeReason} />
        {addRevisionAction ? (
          <ModImportAction
            label={buttonText["add-revision"]}
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
                    canDeleteSelection,
                  }, compact);
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
                        <span className="compact-action__label">{buttonText[action.id] ?? action.label}</span>
                      </span>
                      <span className="compact-action__dot" aria-hidden="true" />
                    </button>
                  )}
                </ModLibraryControlTooltip>
              );
            })}
        </div>

        {compactActions
          // Delete is a batch-only entry point: single deletion lives in the card context menu,
          // so rendering it outside batch selection would leave a permanently disabled button.
          .filter((a) => !["select-all", "invert", "refresh", "add", "add-revision"].includes(a.id))
          .filter((a) => a.id !== "delete" || batchSelectionActive)
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
                canDeleteSelection,
                canUninstallSelection,
              }, compact),
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
                        {lifecycleLabel(action.id, buttonText[action.id] ?? action.label)}
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
                  {batchSelectionActive ? compact.exitBatch : compact.enterBatch}
                </span>
              </span>
            </button>
          )}
        </ModLibraryControlTooltip>

        {batchSelectionActive ? (
          <ModLibraryControlTooltip
            content={
              selectionInteractionDisabledReason
              ?? (selectedCount === 0 ? compact.noSelectedMods : undefined)
            }
          >
            {(descriptionId) => (
              <button
                type="button"
                className="compact-action is-neutral is-icon-only"
                aria-label={compact.clearSelectionAria}
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
              <strong>{compact.footerBatchSelected(selectedCount, MAX_MOD_SELECTION_COUNT)}</strong>
              <span>{compact.footerBatchPage(selectedPageCount, pageCount)}</span>
            </>
          ) : (
            <span>{compact.footerSinglePage(selectedPageCount, pageCount)}</span>
          )}
        </span>
        {/* 选择回执改由页级 toast 呈现；这里只保留无障碍播报。 */}
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
