import { CheckCircle2, Search, ShieldAlert, Tags } from "lucide-react";
import { useId } from "react";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { replacementCopy } from "./replacementCopy";
import { matchedHiddenReplacementTargetNames } from "./replacementTargetMatch";
import { resolveReplacementTargetAliases } from "./replacementTargetNames";
import type { ReplacementTargetOption } from "./replacementTargetOptions";
import type { OccupiedReplacementTarget } from "./replacementTypes";

type ReplacementTargetCatalogProps = {
  options: readonly ReplacementTargetOption[];
  selectedOption: ReplacementTargetOption | null;
  query: string;
  onQueryChange: (query: string) => void;
  onSelect: (targetId: string, alias: string | null) => void;
  disabled: boolean;
  searchDisabled?: boolean;
  installedTargetId?: string;
  disableInstalled?: boolean;
  occupancyByTarget?: ReadonlyMap<string, OccupiedReplacementTarget>;
};

/** 共用名称展开和目标行；安装门禁由单源／多源工作流传入。 */
export function ReplacementTargetCatalog({ options, selectedOption, query, onQueryChange, onSelect, disabled,
  searchDisabled = disabled, installedTargetId, disableInstalled = false, occupancyByTarget }: ReplacementTargetCatalogProps) {
  const { locale } = useI18n();
  const rCopy = resolveCopy(replacementCopy, locale);
  const titleId = useId();
  const radioName = useId();
  return <section className="replacement-panel__catalog" aria-labelledby={titleId}>
    <div className="replacement-panel__section-heading"><h3 id={titleId}>{rCopy.panel.targetsTitle}</h3><span>{rCopy.panel.targetCount(options.length)}</span></div>
    <label className="replacement-panel__search">
      <Search size={16} aria-hidden="true" />
      <input type="search" aria-label={rCopy.panel.searchAria} value={query} onChange={(event) => onQueryChange(event.target.value)} placeholder={rCopy.panel.searchPlaceholder} disabled={searchDisabled} />
    </label>
    <div className="replacement-panel__target-list" role="radiogroup" aria-label={rCopy.panel.targetsAria}>
      {options.map((option) => {
        const target = option.target;
        const currentInstalled = target.id === installedTargetId;
        const occupied = occupancyByTarget?.get(target.id) ?? null;
        const matchHint = matchedHiddenReplacementTargetNames(target, option, query);
        const aliasCount = resolveReplacementTargetAliases(target.aliasesByLocale, locale).length;
        return <label className="replacement-panel__target-row" data-installed={currentInstalled} data-occupied={occupied ? "true" : "false"}
          data-selected={option.key === selectedOption?.key} data-target-id={target.id} key={option.key}>
          <input type="radio" name={radioName} value={option.key} checked={option.key === selectedOption?.key}
            onChange={() => onSelect(target.id, option.alias)} disabled={disabled || (disableInstalled && currentInstalled)} />
          <span className="replacement-panel__target-name">
            <strong>{option.displayName}</strong>
            {option.secondaryName ? <small>{option.secondaryName}</small> : null}
            {matchHint ? <small className="replacement-panel__target-match">
              <Search size={11} aria-hidden="true" /><span>{rCopy.panel.matchedNames(matchHint.names)}</span>
              {matchHint.hiddenCount > 0 ? <em>{rCopy.panel.matchedNamesMore(matchHint.hiddenCount)}</em> : null}
            </small> : null}
          </span>
          <span className="replacement-panel__target-facts">
            {currentInstalled ? <span className="replacement-panel__target-status is-installed"><CheckCircle2 size={13} aria-hidden="true" />{rCopy.panel.currentInstalled}</span> : null}
            {occupied ? <span className="replacement-panel__target-status is-occupied"><ShieldAlert size={13} aria-hidden="true" />{rCopy.panel.targetOccupiedTag}</span> : null}
            {aliasCount > 0 ? <span className="replacement-panel__target-status is-aliases" title={rCopy.panel.aliasCountTitle}><Tags size={13} aria-hidden="true" />{rCopy.panel.aliasCount(aliasCount)}</span> : null}
            <code>{target.internalId}</code>
          </span>
        </label>;
      })}
      {options.length === 0 && <p className="replacement-panel__empty">{rCopy.panel.noMatches}</p>}
    </div>
  </section>;
}
