import { Search, Grid, LayoutGrid, List, Tags, TerminalSquare } from "lucide-react";
import type { CSSProperties } from "react";
import type { ModViewMode } from "./ModLibraryPage";
import { ModLibraryControlTooltip } from "./ModLibraryControlTooltip";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { modLibraryCopy } from "./modLibraryCopy";
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
  const { locale } = useI18n();
  const copy = resolveCopy(modLibraryCopy, locale).toolbar;
  const viewModeIndex = Math.max(0, viewModeOrder.indexOf(viewMode));
  const labelToggleTitle = showCardCategoryLabels ? copy.hideLabels : copy.showLabels;

  return (
    <div className="library-toolbar" data-tour-id="mods.toolbar">
      <div className="library-toolbar__top-row">
        <div className="library-search">
          <Search size={16} aria-hidden="true" />
          <input
            type="search"
            className="library-search__input"
            placeholder={copy.searchPlaceholder}
            value={query}
            onChange={(event) => onQueryChange(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && !event.nativeEvent.isComposing) {
                event.preventDefault();
                onQuerySubmit();
              }
            }}
            aria-label={copy.searchAria}
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
            aria-label={copy.viewSwitchAria}
            style={{ "--view-toggle-index": viewModeIndex } as CSSProperties}
          >
            <span className="library-view-toggle-indicator" aria-hidden="true" />
            <ModLibraryControlTooltip content={copy.classicView} describeControl={false}>
              {() => (
                <button
                  type="button"
                  className={`toggle-btn${viewMode === "classic" ? " active" : ""}`}
                  aria-label={copy.classicView}
                  aria-pressed={viewMode === "classic"}
                  onClick={() => onViewModeChange("classic")}
                >
                  <Grid size={18} strokeWidth={2.5} aria-hidden="true" />
                </button>
              )}
            </ModLibraryControlTooltip>
            <ModLibraryControlTooltip content={copy.gridView} describeControl={false}>
              {() => (
                <button
                  type="button"
                  className={`toggle-btn${viewMode === "grid" ? " active" : ""}`}
                  aria-label={copy.gridView}
                  aria-pressed={viewMode === "grid"}
                  onClick={() => onViewModeChange("grid")}
                >
                  <LayoutGrid size={18} strokeWidth={2.5} aria-hidden="true" />
                </button>
              )}
            </ModLibraryControlTooltip>
            <ModLibraryControlTooltip content={copy.listView} describeControl={false}>
              {() => (
                <button
                  type="button"
                  className={`toggle-btn${viewMode === "list" ? " active" : ""}`}
                  aria-label={copy.listView}
                  aria-pressed={viewMode === "list"}
                  onClick={() => onViewModeChange("list")}
                >
                  <List size={18} strokeWidth={2.5} aria-hidden="true" />
                </button>
              )}
            </ModLibraryControlTooltip>
            <ModLibraryControlTooltip content={copy.techView} describeControl={false}>
              {() => (
                <button
                  type="button"
                  className={`toggle-btn${viewMode === "tech" ? " active" : ""}`}
                  aria-label={copy.techView}
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
        <div className="library-filters" role="group" aria-label={copy.filtersAria}>
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
