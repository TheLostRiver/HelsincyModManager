import { Search, UserCheck, X } from "lucide-react";
import { useEffect, useState } from "react";
import type { ProfileSaveDirectoryNotice } from "./ProfileSaveDirectoryDiscoveryProvider";

const AUTO_DISMISS_TIMEOUT_MS = 6000;

type ProfileSaveDirectoryFloatingNoticeProps = {
  notice: ProfileSaveDirectoryNotice | null;
  isBusy: boolean;
  onReviewCandidates: () => void;
  onRetry: () => Promise<void>;
  onDismiss: () => void;
};

export function ProfileSaveDirectoryFloatingNotice({
  notice,
  isBusy,
  onReviewCandidates,
  onRetry,
  onDismiss,
}: ProfileSaveDirectoryFloatingNoticeProps) {
  const [isDismissPaused, setIsDismissPaused] = useState(false);

  useEffect(() => {
    if (!notice) {
      setIsDismissPaused(false);
    }
  }, [notice]);

  useEffect(() => {
    if (!notice || isDismissPaused) {
      return undefined;
    }

    const dismissTimer = window.setTimeout(() => onDismiss(), AUTO_DISMISS_TIMEOUT_MS);

    return () => window.clearTimeout(dismissTimer);
  }, [isDismissPaused, notice, onDismiss]);

  if (!notice) return null;

  return (
    <aside
      // positioned by CSS
      className={`profile-save-directory-floating-notice is-${notice.tone}`}
      role="status"
      aria-live="polite"
      onPointerEnter={() => setIsDismissPaused(true)}
      onPointerLeave={() => setIsDismissPaused(false)}
      onFocus={() => setIsDismissPaused(true)}
      onBlur={() => setIsDismissPaused(false)}
    >
      <div className="profile-save-directory-floating-notice__copy">
        <strong>{notice.title}</strong>
        <p>{notice.message}</p>
        <span>{notice.detail}</span>
      </div>

      <div className="profile-save-directory-floating-notice__actions">
        {notice.action === "candidates" ? (
          <button type="button" className="primary-action" disabled={isBusy} onClick={onReviewCandidates}>
            <UserCheck size={16} />
            查看候选
          </button>
        ) : null}
        {notice.action === "retry" ? (
          <button type="button" className="primary-action" disabled={isBusy} onClick={() => void onRetry()}>
            <Search size={16} />
            重新检测
          </button>
        ) : null}
        <button
          type="button"
          className="profile-save-directory-floating-notice__dismiss"
          aria-label="关闭存档目录提示"
          onClick={onDismiss}
        >
          <X size={16} />
        </button>
      </div>
    </aside>
  );
}
