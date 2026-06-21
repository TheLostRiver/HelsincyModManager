import { Check } from "lucide-react";
import type { ModLibraryItem } from "./modsLibraryData";

type ModPosterCardProps = {
  item: ModLibraryItem;
  selected: boolean;
  onSelect: (id: string) => void;
  index?: number;
};

const statusLabel: Record<ModLibraryItem["status"], string> = {
  installed: "已安装",
  disabled: "已禁用",
  conflict: "存在冲突",
};

export function ModPosterCard({ item, selected, onSelect, index = 0 }: ModPosterCardProps) {
  return (
    <button
      type="button"
      className={`mod-card anim-stagger-item${selected ? " is-selected" : ""}`}
      style={{ "--stagger-idx": index + 1 } as React.CSSProperties}
      onClick={() => onSelect(item.id)}
      aria-pressed={selected}
      aria-label={`选择 ${item.name}`}
      data-status={item.status}
    >
      <div className="mod-card__selection-indicator">
        <Check size={14} strokeWidth={3} className="mod-card__check-icon" aria-hidden="true" />
      </div>

      <div
        className="mod-card__poster"
        style={{ "--poster-from": item.posterFrom, "--poster-to": item.posterTo } as React.CSSProperties}
      >
        {/* 无预览图时的简化人形剪影占位，仅用于还原设计稿视觉。 */}
        <span className="mod-card__silhouette" aria-hidden="true">
          <span className="mod-card__hair mod-card__hair--left" />
          <span className="mod-card__hair mod-card__hair--right" />
          <span className="mod-card__head" />
          <span className="mod-card__torso" />
        </span>

        <span className={`mod-card__status-pill is-${item.status}`}>
          <Check size={15} strokeWidth={2.6} aria-hidden="true" />
          {statusLabel[item.status]}
        </span>
      </div>

      <div className="mod-card__meta">
        <strong className="mod-card__title">{item.name}</strong>
        <span className="mod-card__size">{item.sizeLabel}</span>
      </div>
    </button>
  );
}
