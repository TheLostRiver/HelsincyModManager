// #286 adopt: the confirmation before the only write in the external-state flow.
//
// An alertdialog on top of the detail dialog (the shared focus trap layers by
// z-index): backdrop clicks do not dismiss it, initial focus lands on Cancel.
// The body states exactly what will be recorded, what will be skipped and why,
// and the one consequence the player must accept — uninstalling adopted files
// deletes them with no automatic way back, because HMM never backed them up.

import { AlertTriangle, ShieldCheck } from "lucide-react";
import { useRef } from "react";
import { Dialog } from "../../shared/feedback";
import type { ExternalAdoptCounts } from "./externalAdoptView";
import type { ExternalAdoptCopy } from "./externalStateCopy";

type ExternalAdoptConfirmDialogProps = {
  open: boolean;
  modName: string;
  counts: ExternalAdoptCounts;
  copy: ExternalAdoptCopy;
  onCancel: () => void;
  onConfirm: () => void;
};

export function ExternalAdoptConfirmDialog({
  open,
  modName,
  counts,
  copy,
  onCancel,
  onConfirm,
}: ExternalAdoptConfirmDialogProps) {
  const cancelButtonRef = useRef<HTMLButtonElement | null>(null);
  if (!open) {
    return null;
  }
  const skippedTotal = counts.skippedChanged + counts.skippedMissing + counts.skippedClaimed;

  return (
    <Dialog
      open
      title={copy.confirm.title}
      description={modName}
      icon={<ShieldCheck size={20} />}
      onClose={onCancel}
      closeLabel={copy.confirm.closeAria}
      closeOnBackdrop={false}
      initialFocusRef={cancelButtonRef}
      role="alertdialog"
      footer={
        <>
          <button
            ref={cancelButtonRef}
            type="button"
            className="mod-detail-dialog__button is-secondary"
            onClick={onCancel}
          >
            {copy.confirm.cancel}
          </button>
          <button
            type="button"
            className="mod-detail-dialog__button is-primary"
            onClick={onConfirm}
          >
            {copy.confirm.confirm(counts.claimable)}
          </button>
        </>
      }
    >
      <div className="mod-detail-dialog__adopt-copy">
        <p>{copy.confirm.body(counts.claimable)}</p>
        {skippedTotal > 0 ? <p>{copy.confirm.skipped(counts)}</p> : null}
        <p className="mod-detail-dialog__adopt-warning" role="note">
          <AlertTriangle size={16} aria-hidden="true" />
          <span>{copy.confirm.uninstallWarning}</span>
        </p>
      </div>
    </Dialog>
  );
}
