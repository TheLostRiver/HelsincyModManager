import { Ban, CheckCircle2, Clock, FileArchive, LoaderCircle, Minus, X, XCircle } from "lucide-react";
import type { ModImportCopy } from "./modImportCopy";
import { getDropRowNote, isDropRowSelectable, type DropRow } from "./modImportDropState";

type ModImportDropRowProps = {
  archivePath: string;
  row: DropRow | null;
  copy: ModImportCopy;
  onSelect?: (selected: boolean) => void;
  onRemove?: () => void;
};

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GiB`;
}

function PhaseMark({ row, copy }: { row: DropRow | null; copy: ModImportCopy }) {
  if (!row) return <LoaderCircle className="mod-import-drop__spinner" size={15} aria-label={copy.drop.checking(1)} />;
  const phases = {
    queued: { Icon: Clock, label: copy.drop.phaseQueued },
    running: { Icon: LoaderCircle, label: copy.drop.phaseRunning },
    succeeded: { Icon: CheckCircle2, label: copy.drop.phaseSucceeded },
    failed: { Icon: XCircle, label: copy.drop.phaseFailed },
    cancelled: { Icon: Ban, label: copy.drop.phaseCancelled },
    skipped: { Icon: Minus, label: copy.drop.phaseSkipped },
  };
  if (row.phase === "pending") return null;
  const { Icon, label } = phases[row.phase];
  return (
    <span className="mod-import-drop__phase" data-phase={row.phase}>
      <Icon className={row.phase === "running" ? "mod-import-drop__spinner" : undefined} size={15} aria-hidden="true" />
      {label}
    </span>
  );
}

export function ModImportDropRow({ archivePath, row, copy, onSelect, onRemove }: ModImportDropRowProps) {
  const name = row?.fileName ?? archivePath.split(/[\\/]/).pop() ?? archivePath;
  const note = row ? getDropRowNote(row, copy) : null;
  const selectable = row !== null && isDropRowSelectable(row);
  const nameContent = <><FileArchive size={16} aria-hidden="true" /><span className="mod-import-drop__file-name" title={archivePath}>{name}</span></>;
  return (
    <li className="mod-import-drop__row" data-status={row?.status ?? "checking"} data-phase={row?.phase ?? "checking"}>
      <div className="mod-import-drop__row-head">
        {onSelect ? (
          <label className="mod-import-drop__row-main">
            <input type="checkbox" checked={row?.selected ?? false} disabled={!selectable} onChange={(event) => onSelect(event.target.checked)} />
            {nameContent}
          </label>
        ) : <div className="mod-import-drop__row-main">{nameContent}</div>}
        {row?.sizeBytes !== null && row?.sizeBytes !== undefined ? <span className="mod-import-drop__size">{formatBytes(row.sizeBytes)}</span> : null}
        <PhaseMark row={row} copy={copy} />
        {onRemove ? (
          <button type="button" className="mod-import-drop__remove" aria-label={copy.drop.removeFile(name)} title={copy.drop.removeFile(name)} onClick={onRemove}>
            <X size={15} aria-hidden="true" />
          </button>
        ) : null}
      </div>
      {note ? <p className="mod-import-drop__row-note">{note}</p> : null}
    </li>
  );
}
