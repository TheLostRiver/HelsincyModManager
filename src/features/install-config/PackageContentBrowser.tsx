import { useMemo, useState, type ComponentProps } from "react";
import { Search, X } from "lucide-react";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { PackageContentTreeView } from "./PackageContentTreeView";
import { flattenVisibleRows, type PackageTreeNode } from "./packageContentTree";
import { collectDirectoryPaths, projectPackageContentTree, type PackageFileFilter } from "./packageContentDisplay";
import { installConfigLayoutCopy } from "./installConfigLayoutCopy";

type BrowserProps = Omit<ComponentProps<typeof PackageContentTreeView>, "rows" | "viewKey"> & {
  tree: readonly PackageTreeNode[]; expandedPaths: ReadonlySet<string>;
};

export function PackageContentBrowser({ tree, expandedPaths, onToggle, ...props }: BrowserProps) {
  const { locale } = useI18n();
  const copy = resolveCopy(installConfigLayoutCopy, locale);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<PackageFileFilter>("all");
  const [compact, setCompact] = useState(true);
  const [filterCollapsed, setFilterCollapsed] = useState<ReadonlySet<string>>(new Set());
  const filtering = query.trim() !== "" || filter !== "all";
  const displayTree = useMemo(() => projectPackageContentTree(tree, { compact, query, filter, excludedFiles: props.excludedFiles }), [tree, compact, query, filter, props.excludedFiles]);
  const displayExpanded = useMemo(() => {
    if (!filtering) return expandedPaths;
    const expanded = collectDirectoryPaths(displayTree);
    for (const path of filterCollapsed) expanded.delete(path);
    return expanded;
  }, [displayTree, expandedPaths, filtering, filterCollapsed]);
  const rows = useMemo(() => flattenVisibleRows(displayTree, displayExpanded), [displayTree, displayExpanded]);
  const resetFilterExpansion = () => setFilterCollapsed(new Set());
  const toggle = (path: string) => {
    if (!filtering) { onToggle(path); return; }
    setFilterCollapsed((current) => {
      const next = new Set(current);
      if (next.has(path)) next.delete(path); else next.add(path);
      return next;
    });
  };
  return <div className="install-config-browser">
    <div className="install-config-browser__toolbar">
      <label className="install-config-browser__search"><Search size={15} aria-hidden="true" />
        <input type="search" aria-label={copy.search} placeholder={copy.search} value={query} onChange={(event) => { setQuery(event.target.value); resetFilterExpansion(); }} />
        {query && <button type="button" aria-label={copy.clearSearch} onClick={() => { setQuery(""); resetFilterExpansion(); }}><X size={14} aria-hidden="true" /></button>}
      </label>
      <div className="install-config-browser__filters" role="group" aria-label={copy.workspace.selection}>
        {(["all", "excluded"] as const).map((value) => <button type="button" key={value} aria-pressed={filter === value} onClick={() => { setFilter(value); resetFilterExpansion(); }}>{copy[value]}</button>)}
      </div>
      <button className="install-config__button is-compact" type="button" aria-pressed={!compact} onClick={() => setCompact((value) => !value)}>{compact ? copy.original : copy.compact}</button>
    </div>
    {filtering && <p className="install-config-browser__hint">{copy.filterHint}</p>}
    <PackageContentTreeView {...props} rows={rows} onToggle={toggle} viewKey={`${query}\u0000${filter}\u0000${compact}`} />
    {rows.length === 0 && <p className="install-config-browser__empty" role="status">{copy.noMatches}</p>}
  </div>;
}
