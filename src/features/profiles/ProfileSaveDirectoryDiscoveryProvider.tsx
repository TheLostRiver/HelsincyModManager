import { isTauri } from "@tauri-apps/api/core";
import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import {
  confirmProfileSaveDirectoryCandidate,
  discoverProfileSaveDirectories,
} from "./profileSaveDirectoryDiscoveryApi";
import type { SaveDirectoryDiscoveryDto } from "./profileSaveDirectoryDiscoveryTypes";
import {
  createPreviewSaveDirectoryConfirmation,
  createPreviewSaveDirectoryDiscovery,
} from "./profilesPreviewData";
import { useFeedback } from "../../shared/feedback";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { useActiveProfile } from "./ActiveProfileProvider";
import { saveDirectoryCopy } from "./saveDirectoryCopy";

type DiscoveryReason = "startup" | "manual";

// notice 只存语义 kind 与参数，文本在 toast 组装时经 saveDirectoryCopy 取
// （语义/文本分离，语言切换不影响已入队的通知语义）。
export type ProfileSaveDirectoryNoticeKind =
  | "preview_manual_only"
  | "detect_failed"
  | "confirm_failed"
  | "auto_saved_startup"
  | "auto_saved_manual"
  | "confirmation_required"
  | "not_found"
  | "scan_failed"
  | "existing_invalid"
  | "reconfirm_required";

export type ProfileSaveDirectoryNotice = {
  id: string;
  tone: "success" | "attention" | "warning";
  kind: ProfileSaveDirectoryNoticeKind;
  action: "candidates" | "retry" | null;
  gameId: string;
  profileId: string;
};

type ProfileSaveDirectoryDiscoveryContextValue = {
  latestDiscovery: SaveDirectoryDiscoveryDto | null;
  isDiscovering: boolean;
  discoveringTarget: DiscoveryTarget | null;
  notice: ProfileSaveDirectoryNotice | null;
  /** 候选选择浮层是否应打开：需要确认且未被用户暂时关闭。 */
  isCandidateSelectionOpen: boolean;
  runDiscovery: (input: { gameId: string; profileId: string; reason: DiscoveryReason }) => Promise<void>;
  confirmCandidate: (candidateId: string) => Promise<void>;
  /** 暂时关闭候选浮层（不做选择）；重新检测或 toast 的"查看候选"可再次打开。 */
  dismissCandidateSelection: () => void;
  dismissNotice: () => void;
};

type DiscoveryTarget = {
  gameId: string;
  profileId: string;
};

type DiscoveryRequestSnapshot = DiscoveryTarget & {
  requestSeq: number;
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
  const { pushToast } = useFeedback();
  const { locale } = useI18n();
  const copy = resolveCopy(saveDirectoryCopy, locale);
  const checkedProfileIdsRef = useRef<Set<string>>(new Set());
  const discoveryRequestSeqRef = useRef(0);
  const activeDiscoveryRequestRef = useRef<DiscoveryRequestSnapshot | null>(null);
  const [latestDiscovery, setLatestDiscovery] = useState<SaveDirectoryDiscoveryDto | null>(null);
  const [notice, setNotice] = useState<ProfileSaveDirectoryNotice | null>(null);
  const [isDiscovering, setIsDiscovering] = useState(false);
  const [discoveringTarget, setDiscoveringTarget] = useState<DiscoveryTarget | null>(null);
  // 记录被用户暂时关闭的 discoveryId：新一轮检测产生新 ID，浮层自然重新打开。
  const [dismissedDiscoveryId, setDismissedDiscoveryId] = useState<string | null>(null);

  const dismissNotice = useCallback(() => setNotice(null), []);

  const dismissCandidateSelection = useCallback(() => {
    setDismissedDiscoveryId((current) => latestDiscovery?.discoveryId ?? current);
  }, [latestDiscovery]);

  const isCandidateSelectionOpen =
    latestDiscovery?.outcome === "confirmation_required" &&
    latestDiscovery.discoveryId !== dismissedDiscoveryId;

  const runDiscovery = useCallback(
    async (input: { gameId: string; profileId: string; reason: DiscoveryReason }) => {
      if (!isTauriRuntime()) {
        // 预览环境不访问真实 Steam 目录；手动检测返回模拟多账号候选，
        // 让候选选择 UI 在纯浏览器下可见、可调整（toast 文案继续如实说明）。
        if (input.reason === "manual") {
          setLatestDiscovery(createPreviewSaveDirectoryDiscovery(input.gameId, input.profileId));
          setNotice({
            id: `preview-${input.profileId}`,
            tone: "attention",
            kind: "preview_manual_only",
            action: null,
            gameId: input.gameId,
            profileId: input.profileId,
          });
        }
        return;
      }

      const requestSeq = discoveryRequestSeqRef.current + 1;
      discoveryRequestSeqRef.current = requestSeq;
      const requestSnapshot = {
        gameId: input.gameId,
        profileId: input.profileId,
        requestSeq,
      };
      activeDiscoveryRequestRef.current = requestSnapshot;
      setDiscoveringTarget({ gameId: input.gameId, profileId: input.profileId });
      setIsDiscovering(true);
      try {
        const discovery = await discoverProfileSaveDirectories({
          gameId: input.gameId,
          profileId: input.profileId,
        });
        if (!isCurrentDiscoveryRequest(activeDiscoveryRequestRef.current, requestSnapshot)) return;
        setLatestDiscovery(discovery);
        setNotice(noticeForDiscovery(discovery, input.reason));
      } catch {
        if (!isCurrentDiscoveryRequest(activeDiscoveryRequestRef.current, requestSnapshot)) return;
        setNotice({
          id: `failed-${input.profileId}-${Date.now()}`,
          tone: "warning",
          kind: "detect_failed",
          action: "retry",
          gameId: input.gameId,
          profileId: input.profileId,
        });
      } finally {
        if (isCurrentDiscoveryRequest(activeDiscoveryRequestRef.current, requestSnapshot)) {
          activeDiscoveryRequestRef.current = null;
          setDiscoveringTarget(null);
          setIsDiscovering(false);
        }
      }
    },
    [],
  );

  const confirmCandidate = useCallback(
    async (candidateId: string) => {
      if (!latestDiscovery) return;

      if (!isTauriRuntime()) {
        // 预览环境本地推进为 auto_saved，仿真真实确认流（与备份中心 preview 仿真同先例）。
        const confirmed = createPreviewSaveDirectoryConfirmation(latestDiscovery, candidateId);
        setLatestDiscovery(confirmed);
        setNotice(noticeForDiscovery(confirmed, "manual"));
        return;
      }

      const requestSeq = discoveryRequestSeqRef.current + 1;
      discoveryRequestSeqRef.current = requestSeq;
      const requestSnapshot = {
        gameId: latestDiscovery.gameId,
        profileId: latestDiscovery.profileId,
        requestSeq,
      };
      activeDiscoveryRequestRef.current = requestSnapshot;
      setIsDiscovering(true);
      setDiscoveringTarget({ gameId: latestDiscovery.gameId, profileId: latestDiscovery.profileId });
      try {
        const discovery = await confirmProfileSaveDirectoryCandidate({
          discoveryId: latestDiscovery.discoveryId,
          candidateId,
        });
        if (!isCurrentDiscoveryRequest(activeDiscoveryRequestRef.current, requestSnapshot)) return;
        setLatestDiscovery(discovery);
        setNotice(noticeForDiscovery(discovery, "manual"));
      } catch {
        if (!isCurrentDiscoveryRequest(activeDiscoveryRequestRef.current, requestSnapshot)) return;
        setNotice({
          id: `confirm-failed-${latestDiscovery.discoveryId}-${Date.now()}`,
          tone: "warning",
          kind: "confirm_failed",
          action: "retry",
          gameId: latestDiscovery.gameId,
          profileId: latestDiscovery.profileId,
        });
      } finally {
        if (isCurrentDiscoveryRequest(activeDiscoveryRequestRef.current, requestSnapshot)) {
          activeDiscoveryRequestRef.current = null;
          setDiscoveringTarget(null);
          setIsDiscovering(false);
        }
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

  // 候选选择已是悬浮层："查看候选"重新打开被关闭的浮层，而不是滚动定位。
  const reviewCandidates = useCallback(() => {
    setDismissedDiscoveryId(null);
  }, []);

  useEffect(() => {
    if (!notice) return;

    const noticeCopy = copy.notices[notice.kind];
    const action = notice.action === "candidates"
      ? { label: copy.noticeActions.reviewCandidates, onSelect: reviewCandidates }
      : notice.action === "retry"
        ? { label: copy.noticeActions.retryDetection, onSelect: () => void retryNotice() }
        : undefined;
    pushToast({
      eventKey: `profile.save-directory.${notice.profileId}.${notice.action ?? notice.tone}`,
      title: noticeCopy.title,
      message: `${noticeCopy.message} ${noticeCopy.detail}`,
      tone: notice.tone === "attention" ? "neutral" : notice.tone,
      action,
    });
    setNotice(null);
  }, [copy, notice, pushToast, retryNotice, reviewCandidates]);

  const value = useMemo<ProfileSaveDirectoryDiscoveryContextValue>(
    () => ({
      latestDiscovery,
      isDiscovering,
      discoveringTarget,
      notice,
      isCandidateSelectionOpen,
      runDiscovery,
      confirmCandidate,
      dismissCandidateSelection,
      dismissNotice,
    }),
    [
      confirmCandidate,
      dismissCandidateSelection,
      dismissNotice,
      discoveringTarget,
      isCandidateSelectionOpen,
      isDiscovering,
      latestDiscovery,
      notice,
      runDiscovery,
    ],
  );

  return (
    <ProfileSaveDirectoryDiscoveryContext.Provider value={value}>
      {children}
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
  return typeof window !== "undefined" && isTauri();
}

function isCurrentDiscoveryRequest(
  current: DiscoveryRequestSnapshot | null,
  request: DiscoveryRequestSnapshot,
) {
  return (
    current?.requestSeq === request.requestSeq &&
    current.gameId === request.gameId &&
    current.profileId === request.profileId
  );
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
      kind: reason === "startup" ? "auto_saved_startup" : "auto_saved_manual",
      action: null,
    };
  }

  if (discovery.outcome === "confirmation_required") {
    return {
      ...base,
      tone: "attention",
      kind: "confirmation_required",
      action: "candidates",
    };
  }

  if (discovery.outcome === "not_found") {
    return {
      ...base,
      tone: "warning",
      kind: "not_found",
      action: "retry",
    };
  }

  if (discovery.outcome === "scan_failed") {
    return {
      ...base,
      tone: "warning",
      kind: "scan_failed",
      action: "retry",
    };
  }

  return {
    ...base,
    tone: "warning",
    kind: discovery.outcome === "existing_invalid" ? "existing_invalid" : "reconfirm_required",
    action: "retry",
  };
}
