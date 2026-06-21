import { Search, Grid, LayoutGrid, List, TerminalSquare } from "lucide-react";
import type { ModViewMode } from "./ModLibraryPage";
import { libraryFilterChips } from "./modsLibraryData";

type LibraryToolbarProps = {
  query: string;
  activeFilter: string;
  viewMode: ModViewMode;
  onQueryChange: (value: string) => void;
  onFilterChange: (value: string) => void;
  onViewModeChange: (mode: ModViewMode) => void;
};

export function LibraryToolbar({
  query,
  activeFilter,
  viewMode,
  onQueryChange,
  onFilterChange,
  onViewModeChange,
}: LibraryToolbarProps) {
  return (
    <div className="library-toolbar">
      <div className="library-search">
        <Search size={16} aria-hidden="true" />
        <input
          type="search"
          className="library-search__input"
          placeholder="搜索 Mod 名称、作者或标签…"
          value={query}
          onChange={(event) => onQueryChange(event.target.value)}
          aria-label="搜索 Mod"
        />
      </div>

      <div className="library-filters" role="group" aria-label="Mod 筛选">
        {libraryFilterChips.map((chip) => (
          <button
            key={chip}
            type="button"
            className={`library-chip${chip === activeFilter ? " is-active" : ""}`}
            aria-pressed={chip === activeFilter}
            onClick={() => onFilterChange(chip)}
          >
            {chip}
          </button>
        ))}
      </div>

      <div className="library-view-toggles" role="group" aria-label="排版视图切换">
        <button
          type="button"
          className={`toggle-btn${viewMode === "classic" ? " active" : ""}`}
          title="经典简约视图"
          aria-pressed={viewMode === "classic"}
          onClick={() => onViewModeChange("classic")}
        >
          <Grid size={18} strokeWidth={2.5} aria-hidden="true" />
        </button>
        <button
          type="button"
          className={`toggle-btn${viewMode === "grid" ? " active" : ""}`}
          title="增强网格视图"
          aria-pressed={viewMode === "grid"}
          onClick={() => onViewModeChange("grid")}
        >
          <LayoutGrid size={18} strokeWidth={2.5} aria-hidden="true" />
        </button>
        <button
          type="button"
          className={`toggle-btn${viewMode === "list" ? " active" : ""}`}
          title="紧凑列表视图"
          aria-pressed={viewMode === "list"}
          onClick={() => onViewModeChange("list")}
        >
          <List size={18} strokeWidth={2.5} aria-hidden="true" />
        </button>
        <button
          type="button"
          className={`toggle-btn${viewMode === "tech" ? " active" : ""}`}
          title="机能数据面板视图"
          aria-pressed={viewMode === "tech"}
          onClick={() => onViewModeChange("tech")}
        >
          <TerminalSquare size={18} strokeWidth={2.5} aria-hidden="true" />
        </button>
      </div>
    </div>
  );
}
