import { User, UserCheck } from "lucide-react";
import { useProfileSaveDirectoryDiscovery } from "./ProfileSaveDirectoryDiscoveryProvider";

export function ProfileSaveDirectoryCandidateList() {
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
          <h3 id="profile-save-directory-candidates-title">选择 Steam 存档账户</h3>
          <span>按最近修改时间推荐，确认后写入当前配置档</span>
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
                <strong>{candidate.accountName ?? "Steam 资料不可用"}</strong>
                {candidate.recommended ? <span>推荐</span> : null}
              </div>
              <div className="profile-save-directory-candidate__meta">
                <span>{candidate.accountLabel}</span>
                <span>{candidate.pathLabel}</span>
                <span>{formatModified(candidate.lastModifiedAt)}</span>
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
              选择此账户
            </button>
          </article>
        ))}
      </div>
    </section>
  );
}

function formatModified(value: number | null) {
  if (value === null) return "最近修改时间不可用";

  const diffMs = Date.now() - value;
  if (!Number.isFinite(diffMs) || diffMs < 0) return "最近修改时间不可用";
  const minutes = Math.max(1, Math.round(diffMs / 60_000));
  if (minutes < 60) return `${minutes} 分钟前修改`;
  const hours = Math.round(minutes / 60);
  if (hours < 48) return `${hours} 小时前修改`;
  return `${Math.round(hours / 24)} 天前修改`;
}
