import { useCallback, useEffect, useState } from "react";
import { getGameSetupStatus, saveGameDirectory, scanGameCandidates } from "../../shared/api/tauri";
import type { GameDirectoryCandidate, GameId, GameSetupStatus } from "./gameSetupTypes";
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
};

const DEFAULT_GAME_ID: GameId = "mhw";

export function useGameSetup(gameId: GameId = DEFAULT_GAME_ID) {
  const [state, setState] = useState<GameSetupState>({
    status: { kind: "not_configured", gameId },
    isBusy: false,
    actionMessage: null,
    candidates: [],
  });

  const refresh = useCallback(async () => {
    try {
      const dto = await getGameSetupStatus(gameId);
      setState((current) => ({
        ...current,
        status: mapStatusDto(dto),
        actionMessage: null,
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
        }));
      }
    },
    [gameId],
  );

  const scanSteam = useCallback(async () => {
    setState((current) => ({ ...current, isBusy: true, actionMessage: null }));

    try {
      const dto = await scanGameCandidates(gameId);
      const candidates = mapCandidateScanDto(dto);
      setState((current) => ({
        ...current,
        candidates,
        isBusy: false,
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

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return {
    status: state.status,
    isBusy: state.isBusy,
    actionMessage: state.actionMessage,
    candidates: state.candidates,
    refresh,
    reportActionError,
    saveDirectory,
    scanSteam,
  };
}
