import { Ban, CheckCircle2, ChevronDown, Clock, LoaderCircle, RotateCcw, XCircle } from "lucide-react";
import { useState } from "react";
import type { Locale } from "../../shared/i18n";
import type { ModImportCopy } from "./modImportCopy";
import { getDropQueueStatus } from "./modImportDropState";
import { dropBatchSummary, type DropImportBatch } from "./modImportDropSession";
import { ModImportDropRow } from "./ModImportDropRow";

type ModImportDropBatchProps = {
  batch: DropImportBatch;
  copy: ModImportCopy;
  locale: Locale;
  initiallyExpanded: boolean;
  onRetry: () => void;
};

export function ModImportDropBatch({ batch, copy, locale, initiallyExpanded, onRetry }: ModImportDropBatchProps) {
  const [expanded, setExpanded] = useState(initiallyExpanded);
  const summary = dropBatchSummary(batch);
  const Icon = summary.running > 0 ? LoaderCircle : summary.active ? Clock
    : summary.failed > 0 ? XCircle : summary.cancelled > 0 ? Ban : CheckCircle2;
  const tag = { zh_cn: "zh-CN", en: "en", ja: "ja" }[locale];
  const timestamp = new Intl.DateTimeFormat(tag, { hour: "2-digit", minute: "2-digit", second: "2-digit" }).format(batch.createdAt);
  const result = copy.drop.batchResult(summary.succeeded, summary.failed, summary.cancelled, summary.skipped);
  return (
    <details className="mod-import-drop__batch" open={expanded} onToggle={(event) => setExpanded(event.currentTarget.open)}>
      <summary className="mod-import-drop__batch-heading">
        <Icon size={19} className={summary.running > 0 ? "mod-import-drop__spinner" : undefined} aria-hidden="true" />
        <span className="mod-import-drop__batch-copy">
          <span className="mod-import-drop__batch-title"><strong>{copy.drop.batchTitle(batch.number)}</strong><time dateTime={new Date(batch.createdAt).toISOString()}>{timestamp}</time></span>
          <span className="mod-import-drop__batch-status">{getDropQueueStatus(summary, copy)}</span>
        </span>
        <ChevronDown size={16} className="mod-import-drop__chevron" aria-hidden="true" />
      </summary>
      {expanded ? <>
      {summary.active ? (
        <progress className="mod-import-drop__batch-progress" value={summary.succeeded + summary.failed + summary.cancelled} max={summary.submitted} aria-label={copy.drop.batchTitle(batch.number)} />
      ) : null}
      <ul className="mod-import-drop__rows">
        {batch.items.map((item) => <ModImportDropRow key={item.id} archivePath={item.row.archivePath} row={item.row} copy={copy} />)}
      </ul>
      {summary.active && result ? <p className="mod-import-drop__batch-result">{result}</p> : null}
      {!summary.active && summary.failed > 0 ? (
        <div className="mod-import-drop__batch-actions">
          <button type="button" className="mod-import-drop__button" onClick={onRetry}><RotateCcw size={14} aria-hidden="true" />{copy.drop.retryFailed(summary.failed)}</button>
        </div>
      ) : null}
      </> : null}
    </details>
  );
}
