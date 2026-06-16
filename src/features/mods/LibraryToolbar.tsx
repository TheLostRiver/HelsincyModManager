import { Search } from "lucide-react";
import { libraryFilterChips } from "./modsLibraryData";

type LibraryToolbarProps = {
  query: string;
  activeFilter: string;
  onQueryChange: (value: string) => void;
  onFilterChange: (value: string) => void;
};

export function LibraryToolbar({ query, activeFilter, onQueryChange, onFilterChange }: LibraryToolbarProps) {
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
    </div>
  );
}
