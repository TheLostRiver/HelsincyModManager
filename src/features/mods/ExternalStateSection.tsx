// #286 detail-dialog section: on-demand external install state check.
//
// Rendered only for mods HMM's manifest does NOT claim (gate lives in the
// dialog): files put into the game directory by other tools never show up in
// HMM's records, so the judgement here comes from the game directory itself.
// The scan is read-only; results arrive via the store getter, never via
// progress events (contract: no target paths in event payloads).

import { resolveCopy, useI18n } from "../../shared/i18n";
import {
  externalStatusAriaLabel,
  projectExternalStatusBadge,
} from "./externalInstallStatusView";
import { externalStateCopy, externalStateErrorMessage } from "./externalStateCopy";
import { useExternalModState } from "./useExternalModState";
import { FolderSearch } from "lucide-react";

type ExternalStateSectionProps = {
  gameId: string;
  profileId: string | null;
  modId: string;
  /** The details tab is visible; gates the initial cached-state query. */
  active: boolean;
};

export function ExternalStateSection({
  gameId,
  profileId,
  modId,
  active,
}: ExternalStateSectionProps) {
  const { locale } = useI18n();
  const copy = resolveCopy(externalStateCopy, locale);
  const workflow = useExternalModState({ gameId, profileId, modId, active });

  const summary = workflow.state?.summary ?? null;
  // The dialog is a wide surface: always project the full tier ("tech" view).
  const badge = summary
    ? projectExternalStatusBadge(summary, "tech", copy.badge)
    : null;
  const pillLabel = badge ? externalStatusAriaLabel(badge, copy.badge) : null;
  // A fresh attempt's failure wins; otherwise surface the stored last error.
  const errorCode = workflow.scanErrorCode ?? workflow.state?.lastError ?? null;

  return (
    <section className="mod-detail-dialog__section">
      <div className="mod-detail-dialog__section-title">
        <FolderSearch size={16} aria-hidden="true" />
        <span>{copy.title}</span>
      </div>
      <p className="mod-detail-dialog__external-intro">{copy.intro}</p>
      {profileId === null ? (
        <p className="mod-detail-dialog__empty">{copy.profileRequired}</p>
      ) : (
        <>
          <div className="mod-detail-dialog__external-actions">
            <button
              type="button"
              className="mod-detail-dialog__button is-secondary"
              onClick={workflow.startScan}
              disabled={workflow.scanning || !workflow.listenerReady}
            >
              {summary ? copy.rescanAction : copy.checkAction}
            </button>
            {workflow.scanning ? (
              <span className="mod-detail-dialog__external-status" role="status">
                {copy.scanning}
              </span>
            ) : null}
          </div>
          {!workflow.scanning && errorCode ? (
            <p className="mod-detail-dialog__external-notice is-error" role="alert">
              {externalStateErrorMessage(errorCode, copy)}
            </p>
          ) : null}
          {!workflow.scanning && summary && workflow.state?.stale ? (
            <p className="mod-detail-dialog__external-notice is-stale">
              {copy.staleNotice}
            </p>
          ) : null}
          {summary && badge && pillLabel ? (
            <>
              <p className="mod-detail-dialog__external-badge">
                <span
                  className="mod-detail-dialog__external-pill"
                  data-case={badge.case}
                  title={pillLabel}
                  aria-label={pillLabel}
                >
                  <span
                    className="mod-detail-dialog__external-pill-origin"
                    aria-hidden="true"
                  >
                    {copy.badge.externalOrigin}
                  </span>
                  <span aria-hidden="true">{badge.text}</span>
                </span>
              </p>
              {badge.case === "unknown" ? (
                <p className="mod-detail-dialog__external-notice is-stale">
                  {copy.unknownHint}
                </p>
              ) : null}
              {summary.files.length > 0 ? (
                <div className="mod-detail-dialog__external-files">
                  <table>
                    <caption>{copy.fileListCaption}</caption>
                    <thead>
                      <tr>
                        <th scope="col">{copy.fileHeaderPath}</th>
                        <th scope="col">{copy.fileHeaderState}</th>
                      </tr>
                    </thead>
                    <tbody>
                      {summary.files.map((file) => (
                        <tr key={file.targetPath} data-state={file.state}>
                          <td>{file.targetPath}</td>
                          <td>{copy.fileState[file.state]}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              ) : null}
            </>
          ) : null}
          {workflow.loaded && !summary && !workflow.scanning && !errorCode ? (
            <p className="mod-detail-dialog__empty">{copy.neverScanned}</p>
          ) : null}
        </>
      )}
    </section>
  );
}
