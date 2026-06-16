import { Ban, BadgeCheck, Plus, RefreshCcw, RotateCw, Shuffle, Trash2, CheckCheck } from "lucide-react";
import type { ComponentType } from "react";
import type { LucideProps } from "lucide-react";
import { compactActions } from "./modsLibraryData";

type CompactActionPanelProps = {
  selectedCount: number;
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

export function CompactActionPanel({ selectedCount, onAction }: CompactActionPanelProps) {
  return (
    <aside className="compact-panel" aria-label="快捷操作">
      <header className="compact-panel__header">
        <h3 className="compact-panel__title">快捷操作</h3>
        <span className="compact-panel__selected-pill">已选 {selectedCount}</span>
      </header>

      <div className="compact-panel__stack">
        {compactActions.map((action) => {
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
      </div>
    </aside>
  );
}
