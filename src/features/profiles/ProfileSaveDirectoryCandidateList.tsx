import { User, UserCheck } from "lucide-react";
import { Dialog } from "../../shared/feedback";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { useProfileSaveDirectoryDiscovery } from "./ProfileSaveDirectoryDiscoveryProvider";
import { saveDirectoryCopy, type SaveDirectoryCopy } from "./saveDirectoryCopy";

export function ProfileSaveDirectoryCandidateList() {
  const { locale } = useI18n();
  const copy = resolveCopy(saveDirectoryCopy, locale).candidates;
  const {
    latestDiscovery,
    isDiscovering,
    confirmCandidate,
    isCandidateSelectionOpen,
    dismissCandidateSelection,
  } = useProfileSaveDirectoryDiscovery();

  // 确认成功后 outcome 变为 auto_saved：保持挂载让浮层播完退场动画，
  // 由 open=false 驱动 ModalSurface 的两段式关闭。
  if (!latestDiscovery || latestDiscovery.candidates.length === 0) {
    return null;
  }

  return (
    <Dialog
      open={isCandidateSelectionOpen}
      title={copy.title}
      description={copy.hint}
      icon={<UserCheck size={18} />}
      busy={isDiscovering}
      onClose={dismissCandidateSelection}
    >
      <div className="profile-save-directory-candidates__list">
        {latestDiscovery.candidates.map((candidate) => (
          <article
            key={candidate.candidateId}
            className={`profile-save-directory-candidate ${candidate.recommended ? "is-recommended" : ""}`}
          >
            <div className="profile-save-directory-candidate__avatar" aria-hidden="true">
              {candidate.avatarUrl ? (
                <img src={candidate.avatarUrl} alt="" />
              ) : (
                <User size={22} />
              )}
            </div>
            <div className="profile-save-directory-candidate__body">
              <div className="profile-save-directory-candidate__title">
                <strong>{candidate.accountName ?? copy.accountUnavailable}</strong>
                {candidate.recommended ? <span>{copy.recommended}</span> : null}
              </div>
              <div className="profile-save-directory-candidate__meta">
                <span>{candidate.accountLabel}</span>
                <span>{candidate.pathLabel}</span>
                <span>{formatModified(candidate.lastModifiedAt, copy)}</span>
              </div>
              {candidate.evidence.length > 0 ? (
                <ul className="profile-save-directory-candidate__evidence">
                  {candidate.evidence.map((item) => (
                    <li key={item}>{item}</li>
                  ))}
                </ul>
              ) : null}
            </div>
            <button
              type="button"
              className="profile-action-button is-primary"
              disabled={isDiscovering}
              onClick={() => void confirmCandidate(candidate.candidateId)}
            >
              {copy.choose}
            </button>
          </article>
        ))}
      </div>
    </Dialog>
  );
}

function formatModified(value: number | null, copy: SaveDirectoryCopy["candidates"]) {
  if (value === null) return copy.modifiedUnavailable;

  const diffMs = Date.now() - value;
  if (!Number.isFinite(diffMs) || diffMs < 0) return copy.modifiedUnavailable;
  const minutes = Math.max(1, Math.round(diffMs / 60_000));
  if (minutes < 60) return copy.modifiedMinutesAgo(minutes);
  const hours = Math.round(minutes / 60);
  if (hours < 48) return copy.modifiedHoursAgo(hours);
  return copy.modifiedDaysAgo(Math.round(hours / 24));
}
