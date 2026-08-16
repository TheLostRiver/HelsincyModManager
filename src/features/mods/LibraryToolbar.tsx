import { Search, Grid, LayoutGrid, List, Tags, TerminalSquare } from "lucide-react";
import type { CSSProperties } from "react";
import type { ModViewMode } from "./ModLibraryPage";
import { ModLibraryControlTooltip } from "./ModLibraryControlTooltip";
import { isSameLibraryFilter, type LibraryFilterChip, type ModLibraryFilter } from "./modLibraryFilters";

type LibraryToolbarProps = {
  query: string;
  activeFilter: ModLibraryFilter;
  filterChips: LibraryFilterChip[];
  viewMode: ModViewMode;
  showCardCategoryLabels: boolean;
  onQueryChange: (value: string) => void;
  onQuerySubmit: () => void;
  onFilterChange: (value: ModLibraryFilter) => void;
  onToggleCardCategoryLabels: () => void;
  onViewModeChange: (mode: ModViewMode) => void;
};

const viewModeOrder: ModViewMode[] = ["classic", "grid", "list", "tech"];

export function LibraryToolbar({
  query,
  activeFilter,
  filterChips,
  viewMode,
  showCardCategoryLabels,
  onQueryChange,
  onQuerySubmit,
  onFilterChange,
  onToggleCardCategoryLabels,
  onViewModeChange,
}: LibraryToolbarProps) {
  const viewModeIndex = Math.max(0, viewModeOrder.indexOf(viewMode));
  const labelToggleTitle = showCardCategoryLabels ? "隐藏分类标签" : "显示分类标签";

  return (
    <div className="library-toolbar" data-tour-id="mods.toolbar">
      <div className="library-toolbar__top-row">
        <div className="library-search">
          <Search size={16} aria-hidden="true" />
          <input
            type="search"
            className="library-search__input"
            placeholder="搜索 Mod 名称、作者或标签…"
            value={query}
            onChange={(event) => onQueryChange(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && !event.nativeEvent.isComposing) {
                event.preventDefault();
                onQuerySubmit();
              }
            }}
            aria-label="搜索 Mod"
          />
        </div>

        <div className="library-toolbar__display-controls">
          <ModLibraryControlTooltip content={labelToggleTitle} describeControl={false}>
            {() => (
              <button
                type="button"
                className={`library-label-toggle${showCardCategoryLabels ? " is-active" : ""}`}
                aria-label={labelToggleTitle}
                aria-pressed={showCardCategoryLabels}
                onClick={onToggleCardCategoryLabels}
              >
                <Tags size={16} strokeWidth={2.3} aria-hidden="true" />
              </button>
            )}
          </ModLibraryControlTooltip>

          <div
            className="library-view-toggles"
            role="group"
            aria-label="排版视图切换"
            style={{ "--view-toggle-index": viewModeIndex } as CSSProperties}
          >
            <span className="library-view-toggle-indicator" aria-hidden="true" />
            <ModLibraryControlTooltip content="经典简约视图" describeControl={false}>
              {() => (
                <button
                  type="button"
                  className={`toggle-btn${viewMode === "classic" ? " active" : ""}`}
                  aria-label="经典简约视图"
                  aria-pressed={viewMode === "classic"}
                  onClick={() => onViewModeChange("classic")}
                >
                  <Grid size={18} strokeWidth={2.5} aria-hidden="true" />
                </button>
              )}
            </ModLibraryControlTooltip>
            <ModLibraryControlTooltip content="增强网格视图" describeControl={false}>
              {() => (
                <button
                  type="button"
                  className={`toggle-btn${viewMode === "grid" ? " active" : ""}`}
                  aria-label="增强网格视图"
                  aria-pressed={viewMode === "grid"}
                  onClick={() => onViewModeChange("grid")}
                >
                  <LayoutGrid size={18} strokeWidth={2.5} aria-hidden="true" />
                </button>
              )}
            </ModLibraryControlTooltip>
            <ModLibraryControlTooltip content="紧凑列表视图" describeControl={false}>
              {() => (
                <button
                  type="button"
                  className={`toggle-btn${viewMode === "list" ? " active" : ""}`}
                  aria-label="紧凑列表视图"
                  aria-pressed={viewMode === "list"}
                  onClick={() => onViewModeChange("list")}
                >
                  <List size={18} strokeWidth={2.5} aria-hidden="true" />
                </button>
              )}
            </ModLibraryControlTooltip>
            <ModLibraryControlTooltip content="机能数据面板视图" describeControl={false}>
              {() => (
                <button
                  type="button"
                  className={`toggle-btn${viewMode === "tech" ? " active" : ""}`}
                  aria-label="机能数据面板视图"
                  aria-pressed={viewMode === "tech"}
                  onClick={() => onViewModeChange("tech")}
                >
                  <TerminalSquare size={18} strokeWidth={2.5} aria-hidden="true" />
                </button>
              )}
            </ModLibraryControlTooltip>
          </div>
        </div>
      </div>

      <div className="library-toolbar__bottom-row">
        <div className="library-filters" role="group" aria-label="Mod 筛选">
          {filterChips.map((chip) => {
            const selected = isSameLibraryFilter(chip.filter, activeFilter);

            return (
              <ModLibraryControlTooltip key={chip.key} content={chip.disabledReason}>
                {(descriptionId) => (
                  <button
                    type="button"
                    className={`library-chip${selected ? " is-active" : ""}`}
                    aria-pressed={selected}
                    aria-disabled={chip.disabled || undefined}
                    aria-describedby={descriptionId}
                    onClick={(event) => {
                      if (chip.disabled) {
                        event.preventDefault();
                        event.stopPropagation();
                        return;
                      }
                      onFilterChange(chip.filter);
                    }}
                  >
                    {chip.color ? (
                      <span className="library-chip__swatch" style={{ backgroundColor: chip.color }} aria-hidden="true" />
                    ) : null}
                    <span>{chip.label}</span>
                  </button>
                )}
              </ModLibraryControlTooltip>
            );
          })}
        </div>
      </div>
    </div>
  );
}
