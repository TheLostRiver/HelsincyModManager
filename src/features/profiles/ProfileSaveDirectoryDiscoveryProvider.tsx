import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import {
  confirmProfileSaveDirectoryCandidate,
  discoverProfileSaveDirectories,
} from "./profileSaveDirectoryDiscoveryApi";
import type { SaveDirectoryDiscoveryDto } from "./profileSaveDirectoryDiscoveryTypes";
import { ProfileSaveDirectoryFloatingNotice } from "./ProfileSaveDirectoryFloatingNotice";
import { useActiveProfile } from "./ActiveProfileProvider";

type DiscoveryReason = "startup" | "manual";

export type ProfileSaveDirectoryNotice = {
  id: string;
  tone: "success" | "attention" | "warning";
  title: string;
  message: string;
  detail: string;
  action: "candidates" | "retry" | null;
  gameId: string;
  profileId: string;
};

type ProfileSaveDirectoryDiscoveryContextValue = {
  latestDiscovery: SaveDirectoryDiscoveryDto | null;
  isDiscovering: boolean;
  notice: ProfileSaveDirectoryNotice | null;
  runDiscovery: (input: { gameId: string; profileId: string; reason: DiscoveryReason }) => Promise<void>;
  confirmCandidate: (candidateId: string) => Promise<void>;
  dismissNotice: () => void;
};

const ProfileSaveDirectoryDiscoveryContext =
  createContext<ProfileSaveDirectoryDiscoveryContextValue | null>(null);

type ProfileSaveDirectoryDiscoveryProviderProps = {
  children: ReactNode;
};

export function ProfileSaveDirectoryDiscoveryProvider({
  children,
}: ProfileSaveDirectoryDiscoveryProviderProps) {
  const { activeProfile } = useActiveProfile();
  const checkedProfileIdsRef = useRef<Set<string>>(new Set());
  const [latestDiscovery, setLatestDiscovery] = useState<SaveDirectoryDiscoveryDto | null>(null);
  const [notice, setNotice] = useState<ProfileSaveDirectoryNotice | null>(null);
  const [isDiscovering, setIsDiscovering] = useState(false);

  const dismissNotice = useCallback(() => setNotice(null), []);

  const runDiscovery = useCallback(
    async (input: { gameId: string; profileId: string; reason: DiscoveryReason }) => {
      if (!isTauriRuntime()) {
        if (input.reason === "manual") {
          setNotice({
            id: `preview-${input.profileId}`,
            tone: "attention",
            title: "自动检测仅在桌面端可用",
            message: "当前预览环境不会访问本地 Steam 存档目录。",
            detail: "可以继续使用手动选择入口调整界面状态。",
            action: null,
            gameId: input.gameId,
            profileId: input.profileId,
          });
        }
        return;
      }

      setIsDiscovering(true);
      try {
        const discovery = await discoverProfileSaveDirectories({
          gameId: input.gameId,
          profileId: input.profileId,
        });
        setLatestDiscovery(discovery);
        setNotice(noticeForDiscovery(discovery, input.reason));
      } catch {
        setNotice({
          id: `failed-${input.profileId}-${Date.now()}`,
          tone: "warning",
          title: "存档目录检测失败",
          message: "没有完成本次自动检测。",
          detail: "可以稍后重试，或继续手动选择存档目录。",
          action: "retry",
          gameId: input.gameId,
          profileId: input.profileId,
        });
      } finally {
        setIsDiscovering(false);
      }
    },
    [],
  );

  const confirmCandidate = useCallback(
    async (candidateId: string) => {
      if (!latestDiscovery) return;

      setIsDiscovering(true);
      try {
        const discovery = await confirmProfileSaveDirectoryCandidate({
          discoveryId: latestDiscovery.discoveryId,
          candidateId,
        });
        setLatestDiscovery(discovery);
        setNotice(noticeForDiscovery(discovery, "manual"));
      } catch {
        setNotice({
          id: `confirm-failed-${latestDiscovery.discoveryId}-${Date.now()}`,
          tone: "warning",
          title: "候选确认失败",
          message: "所选 Steam 存档目录未能通过重新验证。",
          detail: "请重新检测，或使用手动选择入口。",
          action: "retry",
          gameId: latestDiscovery.gameId,
          profileId: latestDiscovery.profileId,
        });
      } finally {
        setIsDiscovering(false);
      }
    },
    [latestDiscovery],
  );

  useEffect(() => {
    if (activeProfile.status !== "ready" || !isTauriRuntime()) return;

    const profileId = activeProfile.profile.id;
    if (checkedProfileIdsRef.current.has(profileId)) return;
    checkedProfileIdsRef.current.add(profileId);

    void runDiscovery({ gameId: "mhw", profileId, reason: "startup" });
  }, [activeProfile, runDiscovery]);

  const retryNotice = useCallback(async () => {
    if (!notice) return;
    await runDiscovery({ gameId: notice.gameId, profileId: notice.profileId, reason: "manual" });
  }, [notice, runDiscovery]);

  const reviewCandidates = useCallback(() => {
    document
      .getElementById("profile-save-directory-candidates")
      ?.scrollIntoView({ behavior: "smooth", block: "start" });
  }, []);

  const value = useMemo<ProfileSaveDirectoryDiscoveryContextValue>(
    () => ({
      latestDiscovery,
      isDiscovering,
      notice,
      runDiscovery,
      confirmCandidate,
      dismissNotice,
    }),
    [confirmCandidate, dismissNotice, isDiscovering, latestDiscovery, notice, runDiscovery],
  );

  return (
    <ProfileSaveDirectoryDiscoveryContext.Provider value={value}>
      {children}
      <ProfileSaveDirectoryFloatingNotice
        notice={notice}
        isBusy={isDiscovering}
        onReviewCandidates={reviewCandidates}
        onRetry={retryNotice}
        onDismiss={dismissNotice}
      />
    </ProfileSaveDirectoryDiscoveryContext.Provider>
  );
}

export function useProfileSaveDirectoryDiscovery() {
  const context = useContext(ProfileSaveDirectoryDiscoveryContext);

  if (!context) {
    throw new Error("useProfileSaveDirectoryDiscovery must be used inside ProfileSaveDirectoryDiscoveryProvider.");
  }

  return context;
}

function isTauriRuntime() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function noticeForDiscovery(
  discovery: SaveDirectoryDiscoveryDto,
  reason: DiscoveryReason,
): ProfileSaveDirectoryNotice | null {
  if (discovery.outcome === "existing_valid") return null;

  const base = {
    id: discovery.discoveryId,
    gameId: discovery.gameId,
    profileId: discovery.profileId,
  };

  if (discovery.outcome === "auto_saved") {
    return {
      ...base,
      tone: "success",
      title: "已自动关联存档目录",
      message: reason === "startup" ? "启动自检已完成，当前配置档可直接备份。" : "存档目录已写入当前配置档。",
      detail: "备份前仍会再次验证目录状态。",
      action: null,
    };
  }

  if (discovery.outcome === "confirmation_required") {
    return {
      ...base,
      tone: "attention",
      title: "发现多个 Steam 存档账户",
      message: "请选择要绑定到当前配置档的账户。",
      detail: "已按最近修改时间推荐候选，但仍需要你确认。",
      action: "candidates",
    };
  }

  return {
    ...base,
    tone: "warning",
    title: "未能自动关联存档目录",
    message: discovery.outcome === "not_found" ? "没有发现可用的 MHW:I Steam 存档目录。" : "当前存档目录需要重新确认。",
    detail: "可以重新检测，或继续使用手动选择入口。",
    action: "retry",
  };
}
