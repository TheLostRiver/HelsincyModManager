import { useCallback, useEffect, useState } from "react";
import {
  autoDetectGameDirectory,
  getGameSetupStatus,
  saveGameDirectory,
  scanGameCandidates,
} from "./gameSetupApi";
import type {
  GameAutoDetectionDto,
  GameDirectoryCandidate,
  GameId,
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
  isBusy: boolean;
  actionMessage: string | null;
  candidates: GameDirectoryCandidate[];
  startupNotice: GameSetupStartupNotice | null;
};

const DEFAULT_GAME_ID: GameId = "mhw";
const STARTUP_DETECTION_TIMEOUT_MS = 10000;

export function useGameSetup(gameId: GameId = DEFAULT_GAME_ID) {
  const [state, setState] = useState<GameSetupState>({
    status: { kind: "not_configured", gameId },
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
        actionMessage: null,
        startupNotice: dto.kind === "configured" ? null : current.startupNotice,
      }));
    } catch (error) {
      const mapped = mapCommandError(error);
      setState((current) => {
        if (mapped.code === "unknown") {
          return {
            ...current,
            actionMessage: mapped.message,
          };
        }

        return {
          ...current,
          status: {
            kind: "invalid",
            gameId,
            errorCode: mapped.code,
            message: messageForError(mapped.code),
          },
          actionMessage: mapped.message,
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
        "启动自检超时，请重试或手动选择游戏目录。",
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
                message: messageForError(mapped.code),
              },
        startupNotice: {
          title: "需要配置游戏目录",
          message: messageForError(mapped.code),
          detail: mapped.message,
          errorCode: mapped.code,
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
          isBusy: false,
          actionMessage: "游戏目录已保存。",
          candidates: [],
          startupNotice: null,
        });
      } catch (error) {
        const mapped = mapCommandError(error);
        const message = messageForError(mapped.code);
        setState((current) => ({
          status: {
            kind: "invalid",
            gameId,
            errorCode: mapped.code,
            message,
          },
          isBusy: false,
          actionMessage: message,
          candidates: current.candidates,
          startupNotice: current.startupNotice,
        }));
      }
    },
    [gameId],
  );

  const scanSteam = useCallback(async () => {
    setState((current) => ({ ...current, isBusy: true, actionMessage: null }));

    try {
      const detection = await withTimeout(
        autoDetectGameDirectory(gameId),
        STARTUP_DETECTION_TIMEOUT_MS,
        "自动扫描超时，请重试或手动选择游戏目录。",
      );

      if (isDetectionReady(detection)) {
        setState((current) => ({
          ...current,
          status: mapStatusDto(detection.status),
          candidates: [],
          isBusy: false,
          startupNotice: null,
          actionMessage: messageForReadyDetection(detection),
        }));
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
        actionMessage:
          candidates.length > 0 ? "已发现 Steam 候选目录。" : "未发现 Steam 候选目录，可手动选择游戏目录。",
      }));
    } catch (error) {
      const mapped = mapCommandError(error);
      setState((current) => ({
        ...current,
        isBusy: false,
        actionMessage: messageForError(mapped.code),
      }));
    }
  }, [gameId]);

  const reportActionError = useCallback((message: string) => {
    setState((current) => ({
      ...current,
      isBusy: false,
      actionMessage: message,
    }));
  }, []);

  const dismissStartupNotice = useCallback(() => {
    setState((current) => ({
      ...current,
      startupNotice: null,
    }));
  }, []);

  useEffect(() => {
    void runStartupDetection();
  }, [runStartupDetection]);

  return {
    status: state.status,
    isBusy: state.isBusy,
    actionMessage: state.actionMessage,
    candidates: state.candidates,
    startupNotice: state.startupNotice,
    dismissStartupNotice,
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
  const detail =
    detection.outcome === "invalid_candidate" && detection.candidateCount > 0
      ? "Steam 返回了候选目录，但校验未通过。"
      : "没有找到可直接保存的 Steam 安装目录。";

  return {
    title: "需要配置游戏目录",
    message: messageForError(errorCode),
    detail,
    errorCode,
  };
}

function isDetectionReady(detection: GameAutoDetectionDto): boolean {
  return detection.outcome === "already_configured" || detection.outcome === "detected_and_saved";
}

function messageForReadyDetection(detection: GameAutoDetectionDto): string {
  return detection.outcome === "detected_and_saved" ? "已自动识别并保存 Steam 游戏目录。" : "游戏目录已准备就绪。";
}

function withTimeout<T>(promise: Promise<T>, timeoutMs: number, timeoutMessage: string): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timeoutId = window.setTimeout(() => {
      reject({
        code: "scan_failed",
        message: timeoutMessage,
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
