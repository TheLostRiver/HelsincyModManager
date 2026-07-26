import { useEffect, useState, type CSSProperties } from "react";
import { Check } from "lucide-react";
import type { ModViewMode } from "./ModLibraryPage";
import { isUnsafeInstallStatus } from "./modLibraryLoadState";
import type { ModLibraryItem } from "./modLibraryTypes";
import { visibleCategoryLabelsForCard } from "./modLibraryFilters";
import "./ModPosterCard.css";

type ModPosterCardProps = {
  item: ModLibraryItem;
  selected: boolean;
  interactionDisabled?: boolean;
  viewMode: ModViewMode;
  onSelect: (id: string) => void;
  onContextMenu?: (id: string, x: number, y: number) => void;
  index?: number;
  showCategoryLabels?: boolean;
};

const statusLabel: Record<ModLibraryItem["status"], string> = {
  not_installed: "未安装",
  installed: "已安装",
  disabled: "已禁用",
  conflict: "存在冲突",
  committed_cleanup_pending: "重装待收尾",
  cleanup_pending: "恢复待清理",
  rollback_required: "需要回滚",
  repair_required: "需要修复",
  unknown: "状态未知",
};

const techStatusLabel: Record<ModLibraryItem["status"], string> = {
  not_installed: "READY",
  installed: "ACTIVE",
  disabled: "DISABLED",
  conflict: "CONFLICT",
  committed_cleanup_pending: "COMMITTED",
  cleanup_pending: "CLEANUP",
  rollback_required: "ROLLBACK",
  repair_required: "REPAIR",
  unknown: "UNKNOWN",
};

const techValidityLabel: Record<ModLibraryItem["status"], string> = {
  not_installed: "PENDING",
  installed: "VALID",
  disabled: "STANDBY",
  conflict: "ERROR",
  committed_cleanup_pending: "PENDING",
  cleanup_pending: "PENDING",
  rollback_required: "RECOVER",
  repair_required: "CHECK",
  unknown: "UNKNOWN",
};

function statusLabelForItem(item: ModLibraryItem) {
  const summary = item.installSummary;
  if (item.status === "installed" && summary && summary.managedFileCount > 0) {
    return `${statusLabel[item.status]} · ${summary.managedFileCount} 文件`;
  }

  if (isUnsafeInstallStatus(item.status) && summary) {
    if (summary.issueCount && summary.issueCount > 0) {
      return `${statusLabel[item.status]} · ${summary.issueCount} 项`;
    }

    if (summary.managedFileCount > 0) {
      return `${statusLabel[item.status]} · ${summary.managedFileCount} 文件`;
    }
  }

  return statusLabel[item.status];
}

export function ModPosterCard({
  item,
  selected,
  interactionDisabled = false,
  viewMode,
  onSelect,
  onContextMenu,
  index = 0,
  showCategoryLabels = true,
}: ModPosterCardProps) {
  const isTech = viewMode === "tech";
  const isList = viewMode === "list";
  const isGrid = viewMode === "grid";
  const isClassic = viewMode === "classic";
  const versionLabel = item.versionLabel ?? "v1.0.0";
  /*
   * 作者只来自真实数据。原实现在 grid / list 视图硬编码 "NexusUser123"、
   * 在 list 视图硬编码一整段中文描述，导致整屏卡片显示同一个作者和同一段文案；
   * tech 视图虽然读了 item.author，但也用同一个假名做 fallback。
   * ModLibraryItem 上没有描述字段，因此列表视图不再渲染描述位，而不是编一段占位文案。
   */
  const authorLabel = item.author?.trim() ? item.author.trim() : null;
  const [posterFailed, setPosterFailed] = useState(false);
  const previewThumbnail = item.previewImage?.kind === "thumbnail" ? item.previewImage : null;
  const canShowPoster = previewThumbnail !== null && !posterFailed;
  const categoryLabelLimit = isList || isTech ? 3 : 2;
  const categoryLabels = visibleCategoryLabelsForCard(item.categoryLabels, categoryLabelLimit);
  const categorySummary =
    showCategoryLabels && item.categoryLabels.length > 0
      ? `，分类：${item.categoryLabels.map((label) => label.name).join("、")}`
      : "";
  const categoryStrip =
    categoryLabels.visible.length > 0 ? (
      <div
        className="mod-card__categories"
        data-visible={showCategoryLabels ? "true" : "false"}
        aria-hidden="true"
      >
        {categoryLabels.visible.map((label) => (
          <span
            className="mod-card__category"
            key={label.name}
            title={label.name}
            style={label.color ? ({ "--category-color": label.color } as CSSProperties) : undefined}
          >
            <span className="mod-card__category-dot" aria-hidden="true" />
            <span className="mod-card__category-name">{label.name}</span>
          </span>
        ))}
        {categoryLabels.overflowCount > 0 ? (
          <span className="mod-card__category-overflow">+{categoryLabels.overflowCount}</span>
        ) : null}
      </div>
    ) : null;

  useEffect(() => {
    setPosterFailed(false);
  }, [previewThumbnail?.thumbnailUrl]);

  return (
    <div
      role="button"
      tabIndex={0}
      className={`mod-card${selected ? " is-selected" : ""}`}
      style={{ "--stagger-idx": index + 1 } as CSSProperties}
      onClick={() => {
        if (!interactionDisabled) {
          onSelect(item.id);
        }
      }}
      onContextMenu={(e) => {
        e.preventDefault();
        e.stopPropagation();
        if (interactionDisabled) {
          return;
        }
        if (onContextMenu) {
          onContextMenu(item.id, e.clientX, e.clientY);
        }
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          if (interactionDisabled) {
            return;
          }
          onSelect(item.id);
        }
      }}
      aria-pressed={selected}
      aria-disabled={interactionDisabled || undefined}
      aria-label={`选择 ${item.name}${categorySummary}`}
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
          style={{ "--poster-from": item.posterFrom, "--poster-to": item.posterTo } as CSSProperties}
        >
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
            <Check size={13} strokeWidth={2.6} aria-hidden="true" />
            <span className="mod-card__status-label">{statusLabelForItem(item)}</span>
          </span>

          {/* 经典视图专属的选中边框 */}
          {isClassic && selected && <span className="mod-card__select-ring" aria-hidden="true" />}
        </div>
      )}

      {/* --- CLASSIC 经典视图文本 --- */}
      {isClassic && (
        <div className="mod-card__meta">
          <strong className="mod-card__title">{item.name}</strong>
          {categoryStrip}
          <span className="mod-card__size">{item.sizeLabel}</span>
        </div>
      )}

      {/* --- GRID 增强网格视图文本 --- */}
      {isGrid && (
        <div className="mod-card__info-enhanced">
          <strong className="mod-card__title">{item.name}</strong>
          {categoryStrip}
          <div className="mod-card__meta-row">
            <span className="mod-card__meta-lead">
              {authorLabel ? <span className="mod-card__author">{authorLabel}</span> : null}
              <span className="mod-card__version-badge">{versionLabel}</span>
            </span>
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
            {authorLabel ? <div className="mod-card__author">by {authorLabel}</div> : null}
            {categoryStrip}
          </div>
          <div className="mod-card__footer-list">
            <span>版本: {versionLabel}</span>
            <span>大小: {item.sizeLabel}</span>
          </div>
        </div>
      )}

      {/* --- TECH 机能面板视图文本 --- */}
      {isTech && (
        <div className="mod-card__info-tech">
          <div>
            <div className="mod-card__title">{item.name}</div>
            {authorLabel ? (
              <div className="mod-card__tech-author" data-label="Author">{authorLabel}</div>
            ) : null}
            {categoryStrip}
          </div>
          <div className="mod-card__tech-footer">
            <span className="mod-card__tech-version" data-label="Version">{versionLabel}</span>
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
