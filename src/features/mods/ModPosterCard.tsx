import { useEffect, useState } from "react";
import { Check } from "lucide-react";
import type { ModViewMode } from "./ModLibraryPage";
import type { ModLibraryItem } from "./modLibraryTypes";

type ModPosterCardProps = {
  item: ModLibraryItem;
  selected: boolean;
  viewMode: ModViewMode;
  onSelect: (id: string) => void;
  onContextMenu?: (id: string, x: number, y: number) => void;
  index?: number;
};

const statusLabel: Record<ModLibraryItem["status"], string> = {
  not_installed: "未安装",
  installed: "已安装",
  disabled: "已禁用",
  conflict: "存在冲突",
  repair_required: "需要修复",
  unknown: "状态未知",
};

const techStatusLabel: Record<ModLibraryItem["status"], string> = {
  not_installed: "READY",
  installed: "ACTIVE",
  disabled: "DISABLED",
  conflict: "CONFLICT",
  repair_required: "REPAIR",
  unknown: "UNKNOWN",
};

const techValidityLabel: Record<ModLibraryItem["status"], string> = {
  not_installed: "PENDING",
  installed: "VALID",
  disabled: "STANDBY",
  conflict: "ERROR",
  repair_required: "CHECK",
  unknown: "UNKNOWN",
};

function statusLabelForItem(item: ModLibraryItem) {
  const summary = item.installSummary;
  if (item.status === "installed" && summary && summary.managedFileCount > 0) {
    return `${statusLabel[item.status]} · ${summary.managedFileCount} 文件`;
  }

  if (item.status === "repair_required" && summary && summary.managedFileCount > 0) {
    return `${statusLabel[item.status]} · ${summary.managedFileCount} 文件`;
  }

  return statusLabel[item.status];
}

export function ModPosterCard({ item, selected, viewMode, onSelect, onContextMenu, index = 0 }: ModPosterCardProps) {
  const isTech = viewMode === "tech";
  const isList = viewMode === "list";
  const isGrid = viewMode === "grid";
  const isClassic = viewMode === "classic";
  const [posterFailed, setPosterFailed] = useState(false);
  const previewThumbnail = item.previewImage?.kind === "thumbnail" ? item.previewImage : null;
  const canShowPoster = previewThumbnail !== null && !posterFailed;

  useEffect(() => {
    setPosterFailed(false);
  }, [previewThumbnail?.thumbnailUrl]);

  return (
    <div
      role="button"
      tabIndex={0}
      className={`mod-card${selected ? " is-selected" : ""}`}
      style={{ "--stagger-idx": index + 1 } as React.CSSProperties}
      onClick={() => onSelect(item.id)}
      onContextMenu={(e) => {
        e.preventDefault();
        e.stopPropagation();
        if (onContextMenu) {
          onContextMenu(item.id, e.clientX, e.clientY);
        }
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onSelect(item.id);
        }
      }}
      aria-pressed={selected}
      aria-label={`选择 ${item.name}`}
      data-status={item.status}
    >
      {/* 选中指示器：所有非经典视图共享 */}
      {!isClassic && (
        <div className="mod-card__selection-indicator">
          <Check size={14} strokeWidth={3} className="mod-card__check-icon" aria-hidden="true" />
        </div>
      )}

      {/* TECH 视图：彻底不展示封面 */}
      {isTech ? null : (
        /* 其他视图（Classic, Grid, List）的封面 */
        <div
          className="mod-card__poster"
          style={{ "--poster-from": item.posterFrom, "--poster-to": item.posterTo } as React.CSSProperties}
        >
          {isGrid && <div className="mod-card__version-badge">v1.0.0</div>}

          {canShowPoster && (
            <img
              className="mod-card__poster-img"
              src={previewThumbnail.thumbnailUrl}
              loading="lazy"
              decoding="async"
              alt=""
              onError={() => setPosterFailed(true)}
            />
          )}

          {/* 无预览图时的简化人形剪影占位 */}
          <span className="mod-card__silhouette" aria-hidden="true" data-visible={!canShowPoster}>
            <span className="mod-card__hair mod-card__hair--left" />
            <span className="mod-card__hair mod-card__hair--right" />
            <span className="mod-card__head" />
            <span className="mod-card__torso" />
          </span>

          {/* 状态徽标：Classic, Grid, List 通用 */}
          <span className={`mod-card__status-pill is-${item.status}`}>
            <Check size={15} strokeWidth={2.6} aria-hidden="true" />
            {statusLabelForItem(item)}
          </span>

          {/* 经典视图专属的选中边框 */}
          {isClassic && selected && <span className="mod-card__select-ring" aria-hidden="true" />}
        </div>
      )}

      {/* --- CLASSIC 经典视图文本 --- */}
      {isClassic && (
        <div className="mod-card__meta">
          <strong className="mod-card__title">{item.name}</strong>
          <span className="mod-card__size">{item.sizeLabel}</span>
        </div>
      )}

      {/* --- GRID 增强网格视图文本 --- */}
      {isGrid && (
        <div className="mod-card__info-enhanced">
          <strong className="mod-card__title">{item.name}</strong>
          <div className="mod-card__meta-row">
            <span className="mod-card__author">NexusUser123</span>
            <span className="mod-card__size">{item.sizeLabel}</span>
          </div>
        </div>
      )}

      {/* --- LIST 紧凑列表视图文本 --- */}
      {isList && (
        <div className="mod-card__info-list">
          <div>
            <div className="mod-card__header">
              <strong className="mod-card__title">{item.name}</strong>
            </div>
            <div className="mod-card__author">by NexusUser123</div>
            <div className="mod-card__desc">这是一个完全重新制作的模型替换 Mod，修复了原版服装在过场动画中的穿模问题，并提供了全套高清贴图支持。</div>
          </div>
          <div className="mod-card__footer-list">
            <span>版本: v1.0.0</span>
            <span>大小: {item.sizeLabel}</span>
          </div>
        </div>
      )}

      {/* --- TECH 机能面板视图文本 --- */}
      {isTech && (
        <div className="mod-card__info-tech">
          <div>
            <div className="mod-card__title">{item.name}</div>
            <div className="mod-card__tech-author" data-label="Author">{item.author || "NexusUser123"}</div>
          </div>
          <div className="mod-card__tech-footer">
            <span className="mod-card__tech-version" data-label="Version">{item.versionLabel || "v1.0.0"}</span>
            <span className="mod-card__tech-size" data-label="Size">{item.sizeLabel || "Unknown"}</span>
          </div>
          <div className="mod-card__tech-status" data-status={item.status}>
            <span style={{ display: "flex", alignItems: "center", gap: "8px" }}>
              <span className="tech-indicator"></span>
              {techStatusLabel[item.status]}
            </span>
            <span>[ {techValidityLabel[item.status]} ]</span>
          </div>
        </div>
      )}
    </div>
  );
}
