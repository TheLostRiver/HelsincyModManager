import { ListChecks, LoaderCircle } from "lucide-react";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { useModImportDrop } from "./ModImportDropProvider";
import { modImportCopy } from "./modImportCopy";

export function ModImportDropAction() {
  const { locale } = useI18n();
  const copy = resolveCopy(modImportCopy, locale).drop;
  const { openDropList, activeImportCount } = useModImportDrop();
  return (
    <button type="button" className="compact-action mod-import-drop__entry" onClick={openDropList} title={activeImportCount > 0 ? copy.activeCount(activeImportCount) : copy.openTasks}>
      <span className="compact-action__left">
        {activeImportCount > 0 ? <LoaderCircle className="mod-import-drop__spinner" size={14} aria-hidden="true" /> : <ListChecks size={14} aria-hidden="true" />}
        <span className="compact-action__label">{copy.openTasks}</span>
      </span>
      {activeImportCount > 0 ? <span className="mod-import-drop__entry-count" aria-label={copy.activeCount(activeImportCount)}>{activeImportCount}</span> : null}
    </button>
  );
}
