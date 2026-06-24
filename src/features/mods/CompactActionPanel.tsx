import { Ban, BadgeCheck, Plus, RefreshCcw, RotateCw, Shuffle, Trash2, CheckCheck } from "lucide-react";
import type { ComponentType } from "react";
import type { LucideProps } from "lucide-react";
import { compactActions } from "./modsLibraryData";

type CompactActionPanelProps = {
  selectedCount: number;
  totalCount: number;
  onAction: (actionId: string) => void;
};

const actionIcons: Record<string, ComponentType<LucideProps>> = {
  add: Plus,
  "select-all": CheckCheck,
  invert: Shuffle,
  refresh: RotateCw,
  "enable-all": BadgeCheck,
  "disable-all": Ban,
  reinstall: RefreshCcw,
  uninstall: Trash2,
};

export function CompactActionPanel({ selectedCount, totalCount, onAction }: CompactActionPanelProps) {
  const addAction = compactActions.find((a) => a.id === "add");

  return (
    <aside className="compact-panel" aria-label="快捷操作">
      <header className="compact-panel__header">
        <h3 className="compact-panel__title">快捷操作</h3>
        <span className="compact-panel__selected-pill">已选 {selectedCount}</span>
      </header>

      <div className="compact-panel__stack">
        {addAction && (
          <button
            type="button"
            className={`compact-action is-${addAction.variant}`}
            data-variant={addAction.variant}
            onClick={() => onAction(addAction.id)}
          >
            <span className="compact-action__left">
              <Plus size={14} strokeWidth={3} aria-hidden="true" />
              <span className="compact-action__label">{addAction.label}</span>
            </span>
          </button>
        )}

        <div className="compact-action-group">
          {compactActions
            .filter((a) => ["select-all", "invert", "refresh"].includes(a.id))
            .map((action) => {
              const Icon = actionIcons[action.id] ?? Plus;
              return (
                <button
                  key={action.id}
                  type="button"
                  className={`compact-action is-${action.variant}`}
                  data-variant={action.variant}
                  onClick={() => onAction(action.id)}
                >
                  <span className="compact-action__left">
                    <Icon size={14} strokeWidth={2.4} aria-hidden="true" />
                    <span className="compact-action__label">{action.label}</span>
                  </span>
                  <span className="compact-action__dot" aria-hidden="true" />
                </button>
              );
            })}
        </div>

        {compactActions
          .filter((a) => !["select-all", "invert", "refresh", "add"].includes(a.id))
          .map((action) => {
            const Icon = actionIcons[action.id] ?? Plus;
            const needsSelection = ["reinstall", "uninstall"].includes(action.id);
            const disabled = needsSelection && selectedCount === 0;
            return (
              <button
                key={action.id}
                type="button"
                className={`compact-action is-${action.variant}`}
                data-variant={action.variant}
                onClick={() => onAction(action.id)}
                disabled={disabled}
              >
                <span className="compact-action__left">
                  <Icon size={14} strokeWidth={2.4} aria-hidden="true" />
                  <span className="compact-action__label">{action.label}</span>
                </span>
                <span className="compact-action__dot" aria-hidden="true" />
              </button>
            );
          })}

        <div className="compact-panel__spacer" aria-hidden="true" />
        <span className="compact-panel__selection-status">
          已选中 {selectedCount} / 共 {totalCount} 项
        </span>
      </div>
    </aside>
  );
}
