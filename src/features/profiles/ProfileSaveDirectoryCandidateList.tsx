import { User, UserCheck } from "lucide-react";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { useProfileSaveDirectoryDiscovery } from "./ProfileSaveDirectoryDiscoveryProvider";
import { saveDirectoryCopy, type SaveDirectoryCopy } from "./saveDirectoryCopy";

export function ProfileSaveDirectoryCandidateList() {
  const { locale } = useI18n();
  const copy = resolveCopy(saveDirectoryCopy, locale).candidates;
  const { latestDiscovery, isDiscovering, confirmCandidate } = useProfileSaveDirectoryDiscovery();

  if (latestDiscovery?.outcome !== "confirmation_required") {
    return null;
  }

  return (
    <section
      id="profile-save-directory-candidates"
      className="profile-save-directory-candidates"
      aria-labelledby="profile-save-directory-candidates-title"
    >
      <div className="profile-save-directory-candidates__header">
        <div>
          <h3 id="profile-save-directory-candidates-title">{copy.title}</h3>
          <span>{copy.hint}</span>
        </div>
        <UserCheck size={18} aria-hidden="true" />
      </div>

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
    </section>
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
