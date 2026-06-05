import { useCallback, useEffect, useState } from "react";
import { getGameSetupStatus, saveGameDirectory, scanGameCandidates } from "../../shared/api/tauri";
import type { GameId, GameSetupStatus } from "./gameSetupTypes";
import { mapCommandError, mapStatusDto, messageForError } from "./gameSetupViewModel";

type GameSetupState = {
  status: GameSetupStatus;
  isBusy: boolean;
  actionMessage: string | null;
};

const DEFAULT_GAME_ID: GameId = "mhw";

export function useGameSetup(gameId: GameId = DEFAULT_GAME_ID) {
  const [state, setState] = useState<GameSetupState>({
    status: { kind: "not_configured", gameId },
    isBusy: false,
    actionMessage: null,
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
        });
      } catch (error) {
        const mapped = mapCommandError(error);
        const message = messageForError(mapped.code);
        setState({
          status: {
            kind: "invalid",
            gameId,
            errorCode: mapped.code,
            message,
          },
          isBusy: false,
          actionMessage: message,
        });
      }
    },
    [gameId],
  );

  const scanSteam = useCallback(async () => {
    setState((current) => ({ ...current, isBusy: true, actionMessage: null }));

    try {
      await scanGameCandidates(gameId);
      setState((current) => ({
        ...current,
        isBusy: false,
        actionMessage: "自动扫描没有返回候选目录。",
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
    refresh,
    reportActionError,
    saveDirectory,
    scanSteam,
  };
}
