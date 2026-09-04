import { useCallback, useEffect, useRef, useState } from "react";
import { useFeedback } from "../../shared/feedback";
import { resolveCopy, useI18n } from "../../shared/i18n";
import {
  autoDetectGameDirectory,
  getGameSetupStatus,
  saveGameDirectory,
  scanGameCandidates,
} from "./gameSetupApi";
import { gameSetupCopy } from "./gameSetupCopy";
import type {
  GameAutoDetectionDto,
  GameDirectoryCandidate,
  GameId,
  GameSetupErrorCode,
  GameSetupStartupNotice,
  GameSetupStatus,
} from "./gameSetupTypes";
import {
  mapCandidateScanDto,
  mapCommandError,
  mapStatusDto,
  messageForError,
} from "./gameSetupViewModel";

type GameSetupState = {
  status: GameSetupStatus;
  /**
   * 上一次「手动保存游戏目录」的失败原因，与 status 正交。
   *
   * 曾经把它塞进 status（改成 invalid），后果是已配置时选错一次目录就把配置
   * 显示成丢失，并连带禁掉一键启动、恢复中心与安装健康检测（#333）——因为
   * status.kind 是这些能力的总闸门。可 status 描述的是「当前配置状态」，
   * 「上次保存失败」是另一个维度：后端拒绝时磁盘上的配置原封不动，
   * 前端没有任何理由假装它丢了。两者必须分开存。
   *
   * 只有手动保存会写它（自动检测的失败另有 startupNotice 承载），
   * 成功保存与 refresh 成功都会清掉。
   */
  lastSaveError: GameSetupErrorCode | null;
  isBusy: boolean;
  actionMessage: string | null;
  candidates: GameDirectoryCandidate[];
  startupNotice: GameSetupStartupNotice | null;
};

const DEFAULT_GAME_ID: GameId = "mhw";
const STARTUP_DETECTION_TIMEOUT_MS = 10000;

/**
 * 游戏目录配置的状态与操作。
 *
 * 只应由 GameSetupProvider 调用一次。它在挂载时会触发一次启动自检（含 Steam 库扫描
 * 与 10 秒超时），每多一个调用方就多跑一遍；而各调用方持有的又是彼此独立的副本，
 * 在一处配置完目录，另一处不会更新。组件请改用 useGameSetup()。
 */
export function useGameSetupState(gameId: GameId = DEFAULT_GAME_ID) {
  const { pushToast } = useFeedback();
  const { locale } = useI18n();
  const copy = resolveCopy(gameSetupCopy, locale);
  // 回调经 ref 取词：启动自检 effect 依赖 runStartupDetection，copy 一旦进入
  // 依赖链，切换语言就会重跑 Steam 扫描。状态本身只存语义码，文本在渲染时取。
  const copyRef = useRef(copy);
  copyRef.current = copy;
  const [state, setState] = useState<GameSetupState>({
    status: { kind: "not_configured", gameId },
    lastSaveError: null,
    isBusy: false,
    actionMessage: null,
    candidates: [],
    startupNotice: null,
  });

  const refresh = useCallback(async () => {
    try {
      const dto = await getGameSetupStatus(gameId);
      setState((current) => ({
        ...current,
        status: mapStatusDto(dto),
        /* 读到配置即证明上次保存的失败已经不再是现状，清掉残留提示。 */
        lastSaveError: dto.kind === "configured" ? null : current.lastSaveError,
        actionMessage: null,
        startupNotice: dto.kind === "configured" ? null : current.startupNotice,
      }));
    } catch (error) {
      const mapped = mapCommandError(error);
      setState((current) => {
        if (mapped.code === "unknown") {
          return {
            ...current,
            actionMessage: mapped.backendMessage,
          };
        }

        return {
          ...current,
          status: {
            kind: "invalid",
            gameId,
            errorCode: mapped.code,
            backendMessage: mapped.backendMessage,
          },
          actionMessage: mapped.backendMessage,
        };
      });
    }
  }, [gameId]);

  const runStartupDetection = useCallback(async () => {
    setState((current) => ({
      ...current,
      isBusy: true,
      actionMessage: null,
    }));

    try {
      const detection = await withTimeout(
        autoDetectGameDirectory(gameId),
        STARTUP_DETECTION_TIMEOUT_MS,
      );
      setState((current) => ({
        ...current,
        status: mapStatusDto(detection.status),
        isBusy: false,
        actionMessage: null,
        candidates: isDetectionReady(detection) ? [] : current.candidates,
        startupNotice: setStartupNoticeForDetection(detection),
      }));
    } catch (error) {
      const timedOut = isTimeoutError(error);
      const mapped = mapCommandError(error);
      setState((current) => ({
        ...current,
        isBusy: false,
        actionMessage: null,
        status:
          mapped.code === "unknown"
            ? current.status
            : {
                kind: "invalid",
                gameId,
                errorCode: mapped.code,
                backendMessage: mapped.backendMessage,
              },
        startupNotice: {
          errorCode: mapped.code,
          detailKind: timedOut ? "startup_timeout" : "command_error",
          backendDetail: timedOut ? null : mapped.backendMessage,
        },
      }));
    }
  }, [gameId]);

  const saveDirectory = useCallback(
    async (directory: string) => {
      setState((current) => ({
        ...current,
        status: { kind: "validating", gameId },
        isBusy: true,
        actionMessage: null,
      }));

      try {
        const dto = await saveGameDirectory(gameId, directory);
        setState({
          status: mapStatusDto(dto),
          lastSaveError: null,
          isBusy: false,
          actionMessage: null,
          candidates: [],
          startupNotice: null,
        });
        pushToast({
          eventKey: `game-setup.directory.saved.${gameId}`,
          title: copyRef.current.toasts.directorySavedTitle,
          message: copyRef.current.toasts.directorySavedMessage,
          tone: "success",
        });
      } catch (error) {
        const mapped = mapCommandError(error);
        setState((current) => ({
          ...current,
          /*
           * 保存失败不改写 status：后端在写盘之前就拒绝了（game_setup.rs 的
           * save_game_directory 所有校验都先于 repository.save_game_instance），
           * 磁盘上的配置原封不动，UI 也就不能假装它丢了。已配置时把 status 拍成
           * invalid 会连带禁掉一键启动、恢复中心与安装健康检测——它们都以
           * status.kind === "configured" 为闸门（#333）。失败原因改由
           * lastSaveError 承载，在配置面板常驻显示。
           *
           * 未配置时没有既有配置可保护，此时转 invalid 仍然是对的。
           */
          status:
            current.status.kind === "configured"
              ? current.status
              : {
                  kind: "invalid",
                  gameId,
                  errorCode: mapped.code,
                  backendMessage: mapped.backendMessage,
                },
          lastSaveError: mapped.code,
          isBusy: false,
        }));
        pushToast({
          eventKey: `game-setup.directory.save-failed.${gameId}.${mapped.code}`,
          title: copyRef.current.toasts.directorySaveFailedTitle,
          message: messageForError(mapped.code, copyRef.current.errors),
          tone: "danger",
        });
      }
    },
    [gameId, pushToast],
  );

  const scanSteam = useCallback(async () => {
    setState((current) => ({ ...current, isBusy: true, actionMessage: null }));

    try {
      const detection = await withTimeout(
        autoDetectGameDirectory(gameId),
        STARTUP_DETECTION_TIMEOUT_MS,
      );

      if (isDetectionReady(detection)) {
        setState((current) => ({
          ...current,
          status: mapStatusDto(detection.status),
          candidates: [],
          isBusy: false,
          startupNotice: null,
          actionMessage: null,
        }));
        pushToast({
          eventKey: `game-setup.scan.ready.${gameId}.${detection.outcome}`,
          title: copyRef.current.toasts.scanReadyTitle,
          message:
            detection.outcome === "detected_and_saved"
              ? copyRef.current.toasts.scanDetectedSaved
              : copyRef.current.toasts.scanAlreadyReady,
          tone: "success",
        });
        return;
      }

      const dto = await scanGameCandidates(gameId);
      const candidates = mapCandidateScanDto(dto);
      setState((current) => ({
        ...current,
        status: mapStatusDto(detection.status),
        candidates,
        isBusy: false,
        startupNotice: setStartupNoticeForDetection(detection),
        actionMessage: null,
      }));
      pushToast({
        eventKey: `game-setup.scan.candidates.${gameId}.${candidates.length > 0 ? "found" : "empty"}`,
        title: candidates.length > 0
          ? copyRef.current.toasts.candidatesFoundTitle
          : copyRef.current.toasts.candidatesEmptyTitle,
        message: candidates.length > 0
          ? copyRef.current.toasts.candidatesFoundMessage
          : copyRef.current.toasts.candidatesEmptyMessage,
        tone: candidates.length > 0 ? "success" : "warning",
      });
    } catch (error) {
      const mapped = mapCommandError(error);
      setState((current) => ({
        ...current,
        isBusy: false,
        actionMessage: null,
      }));
      pushToast({
        eventKey: `game-setup.scan.failed.${gameId}.${mapped.code}`,
        title: copyRef.current.toasts.scanFailedTitle,
        message: messageForError(mapped.code, copyRef.current.errors),
        tone: "danger",
      });
    }
  }, [gameId, pushToast]);

  const reportActionError = useCallback((message: string) => {
    setState((current) => ({
      ...current,
      isBusy: false,
      actionMessage: null,
    }));
    pushToast({
      eventKey: `game-setup.action.failed.${gameId}`,
      title: copyRef.current.toasts.actionFailedTitle,
      message,
      tone: "danger",
    });
  }, [gameId, pushToast]);

  useEffect(() => {
    void runStartupDetection();
  }, [runStartupDetection]);

  return {
    status: state.status,
    lastSaveError: state.lastSaveError,
    isBusy: state.isBusy,
    actionMessage: state.actionMessage,
    candidates: state.candidates,
    startupNotice: state.startupNotice,
    refresh,
    reportActionError,
    retryStartupDetection: runStartupDetection,
    saveDirectory,
    scanSteam,
  };
}

function setStartupNoticeForDetection(detection: GameAutoDetectionDto): GameSetupStartupNotice | null {
  if (isDetectionReady(detection)) {
    return null;
  }

  const errorCode =
    detection.errorCode ?? (detection.outcome === "scan_failed" ? "scan_failed" : "directory_not_found");

  return {
    errorCode,
    detailKind:
      detection.outcome === "invalid_candidate" && detection.candidateCount > 0
        ? "invalid_candidate"
        : "not_found",
    backendDetail: null,
  };
}

function isDetectionReady(detection: GameAutoDetectionDto): boolean {
  return detection.outcome === "already_configured" || detection.outcome === "detected_and_saved";
}

const TIMEOUT_MARKER = Symbol("gameSetupTimeout");

function isTimeoutError(error: unknown): boolean {
  return typeof error === "object" && error !== null && TIMEOUT_MARKER in error;
}

function withTimeout<T>(promise: Promise<T>, timeoutMs: number): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timeoutId = window.setTimeout(() => {
      reject({
        code: "scan_failed",
        message: "",
        [TIMEOUT_MARKER]: true,
      });
    }, timeoutMs);

    promise.then(
      (value) => {
        window.clearTimeout(timeoutId);
        resolve(value);
      },
      (error: unknown) => {
        window.clearTimeout(timeoutId);
        reject(error);
      },
    );
  });
}
