// #286 detail-dialog section: on-demand external install state check + adopt.
//
// Rendered only for mods HMM's manifest does NOT claim (gate lives in the
// dialog): files put into the game directory by other tools never show up in
// HMM's records, so the judgement here comes from the game directory itself.
// The scan is read-only; results arrive via the store getter, never via
// progress events (contract: no target paths in event payloads).
//
// Adopt is the one write: it records the scanned, matched, unclaimed files as
// manifest entries and touches no game file. The button only lights up when
// the backend's own pre-checks would pass (`projectExternalAdoptAvailability`),
// and it always goes through an explicit confirmation.

import { resolveCopy, useI18n } from "../../shared/i18n";
import { ExternalAdoptConfirmDialog } from "./ExternalAdoptConfirmDialog";
import { projectExternalAdoptAvailability } from "./externalAdoptView";
import {
  externalStatusAriaLabel,
  fileClaimantDisplayName,
  occupierDisplayName,
  projectExternalStatusBadge,
} from "./externalInstallStatusView";
import type { ExternalModStateDto } from "./externalStateApi";
import {
  externalAdoptErrorMessage,
  externalStateCopy,
  externalStateErrorMessage,
} from "./externalStateCopy";
import { useExternalModState, type ExternalModAdoptCompletion } from "./useExternalModState";
import { FolderSearch } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

export type ExternalAdoptCompletedResult = {
  /** Ready-to-show status line (section owns the copy; the dialog owns the slot). */
  notice: string;
};

type ExternalStateSectionProps = {
  gameId: string;
  profileId: string | null;
  modId: string;
  /** Shown as the confirmation's subtitle so the player sees what they adopt. */
  modName: string;
  /** The details tab is visible; gates the initial cached-state query. */
  active: boolean;
  /** Mirrors every getter result to the page-level session store (#286 3b-2). */
  onResult?: (modId: string, state: ExternalModStateDto) => void;
  /** An adopt is running: the dialog must not close or switch tabs meanwhile. */
  onBusyChange?: (busy: boolean) => void;
  /** The manifest now claims this mod; the dialog refreshes into the installed state. */
  onAdoptCompleted?: (result: ExternalAdoptCompletedResult) => void | Promise<void>;
};

export function ExternalStateSection({
  gameId,
  profileId,
  modId,
  modName,
  active,
  onResult,
  onBusyChange,
  onAdoptCompleted,
}: ExternalStateSectionProps) {
  const { locale } = useI18n();
  const copy = resolveCopy(externalStateCopy, locale);
  const [confirmOpen, setConfirmOpen] = useState(false);
  // Set the moment the adopt completes and kept until this section unmounts: the
  // backend dropped the scan record, so the re-query comes back empty and the
  // body would otherwise flash "never scanned" while the dialog is still refreshing.
  const [completedNotice, setCompletedNotice] = useState<string | null>(null);
  useEffect(() => {
    setCompletedNotice(null);
    setConfirmOpen(false);
  }, [modId, profileId]);
  // The completed event carries no counts (contract); the notice quotes the
  // preview the player confirmed — which is exactly what the backend wrote,
  // or it would have failed as stale.
  const confirmedClaimableRef = useRef(0);
  const copyRef = useRef(copy);
  copyRef.current = copy;
  const onAdoptCompletedRef = useRef(onAdoptCompleted);
  onAdoptCompletedRef.current = onAdoptCompleted;

  const handleAdoptCompleted = useCallback((completion: ExternalModAdoptCompletion) => {
    const adoptCopy = copyRef.current.adopt;
    const notice = completion.auditDegraded
      ? `${adoptCopy.completed(confirmedClaimableRef.current)} ${adoptCopy.completedAuditDegraded}`
      : adoptCopy.completed(confirmedClaimableRef.current);
    setCompletedNotice(notice);
    return onAdoptCompletedRef.current?.({ notice });
  }, []);

  const workflow = useExternalModState({
    gameId,
    profileId,
    modId,
    active,
    onResult,
    onAdoptCompleted: handleAdoptCompleted,
  });

  const onBusyChangeRef = useRef(onBusyChange);
  onBusyChangeRef.current = onBusyChange;
  useEffect(() => {
    onBusyChangeRef.current?.(workflow.adopting);
  }, [workflow.adopting]);
  useEffect(() => () => onBusyChangeRef.current?.(false), []);

  const summary = workflow.state?.summary ?? null;
  // The dialog is a wide surface: always project the full tier ("tech" view).
  const badge = summary
    ? projectExternalStatusBadge(summary, "tech", copy.badge)
    : null;
  const pillLabel = badge ? externalStatusAriaLabel(badge, copy.badge) : null;
  // A fresh attempt's failure wins; otherwise surface the stored last error.
  const errorCode = workflow.scanErrorCode ?? workflow.state?.lastError ?? null;
  const busy = workflow.scanning || workflow.adopting;
  const availability = projectExternalAdoptAvailability(workflow.state);
  const adoptCounts = availability.status === "available" ? availability.counts : null;
  // No hint before the first scan: `neverScanned` already tells the player what to do.
  const adoptBlockedHint =
    summary && availability.status === "blocked" && availability.reason !== "no_summary"
      ? copy.adopt.blocked[availability.reason]
      : null;
  // The stale notice right above already says "check again"; do not say it twice.
  const adoptBlockedHintLine =
    availability.status === "blocked" && availability.reason === "stale" ? null : adoptBlockedHint;

  const requestAdopt = () => {
    if (adoptCounts === null || busy) {
      return;
    }
    setConfirmOpen(true);
  };
  const confirmAdopt = () => {
    if (adoptCounts === null) {
      setConfirmOpen(false);
      return;
    }
    confirmedClaimableRef.current = adoptCounts.claimable;
    setConfirmOpen(false);
    workflow.startAdopt();
  };

  return (
    <section className="mod-detail-dialog__section">
      <div className="mod-detail-dialog__section-title">
        <FolderSearch size={16} aria-hidden="true" />
        <span>{copy.title}</span>
      </div>
      <p className="mod-detail-dialog__external-intro">{copy.intro}</p>
      {profileId === null ? (
        <p className="mod-detail-dialog__empty">{copy.profileRequired}</p>
      ) : completedNotice !== null ? (
        <p className="mod-detail-dialog__external-notice is-occupied" role="status">
          {completedNotice}
        </p>
      ) : (
        <>
          <div className="mod-detail-dialog__external-actions">
            <button
              type="button"
              className="mod-detail-dialog__button is-secondary"
              onClick={workflow.startScan}
              disabled={busy || !workflow.listenerReady}
            >
              {summary ? copy.rescanAction : copy.checkAction}
            </button>
            <button
              type="button"
              className="mod-detail-dialog__button is-primary"
              onClick={requestAdopt}
              disabled={adoptCounts === null || busy || !workflow.listenerReady}
              title={adoptBlockedHint ?? undefined}
            >
              {adoptCounts ? copy.adopt.action(adoptCounts.claimable) : copy.adopt.actionIdle}
            </button>
            {workflow.scanning ? (
              <span className="mod-detail-dialog__external-status" role="status">
                {copy.scanning}
              </span>
            ) : null}
            {workflow.adopting ? (
              <span className="mod-detail-dialog__external-status" role="status">
                {copy.adopt.adopting}
              </span>
            ) : null}
          </div>
          {!busy && errorCode ? (
            <p className="mod-detail-dialog__external-notice is-error" role="alert">
              {externalStateErrorMessage(errorCode, copy)}
            </p>
          ) : null}
          {!busy && workflow.adoptErrorCode ? (
            <p className="mod-detail-dialog__external-notice is-error" role="alert">
              {externalAdoptErrorMessage(workflow.adoptErrorCode, copy)}
            </p>
          ) : null}
          {!busy && summary && workflow.state?.stale ? (
            <p className="mod-detail-dialog__external-notice is-stale">
              {copy.staleNotice}
            </p>
          ) : null}
          {!busy && adoptBlockedHintLine ? (
            <p className="mod-detail-dialog__external-notice">{adoptBlockedHintLine}</p>
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
              {summary.occupiedBy.length > 0 ? (
                // #286 attribution: these paths belong to HMM-managed mods, so
                // "externally installed" would be misleading — say who owns them.
                <p className="mod-detail-dialog__external-notice is-occupied">
                  {copy.occupiedNotice(
                    summary.occupiedBy.map(occupierDisplayName),
                    summary.files.filter((file) => file.claimedByModId !== undefined).length,
                  )}
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
                      {summary.files.map((file) => {
                        const claimant = fileClaimantDisplayName(file);
                        return (
                          <tr key={file.targetPath} data-state={file.state}>
                            <td>{file.targetPath}</td>
                            <td>
                              {copy.fileState[file.state]}
                              {claimant !== null ? (
                                <span className="mod-detail-dialog__external-claimed">
                                  {copy.fileClaimedBy(claimant)}
                                </span>
                              ) : null}
                            </td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                </div>
              ) : null}
            </>
          ) : null}
          {workflow.loaded && !summary && !busy && !errorCode ? (
            <p className="mod-detail-dialog__empty">{copy.neverScanned}</p>
          ) : null}
          {adoptCounts ? (
            <ExternalAdoptConfirmDialog
              open={confirmOpen}
              modName={modName}
              counts={adoptCounts}
              copy={copy.adopt}
              onCancel={() => setConfirmOpen(false)}
              onConfirm={confirmAdopt}
            />
          ) : null}
        </>
      )}
    </section>
  );
}
