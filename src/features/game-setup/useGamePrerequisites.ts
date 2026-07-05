import { useCallback, useEffect, useState } from "react";
import { getGamePrerequisiteStatus } from "./gamePrerequisiteApi";
import type { GameId } from "./gameSetupTypes";
import type { GamePrerequisiteLoadState } from "./gamePrerequisiteTypes";
import { mapCommandError } from "./gameSetupViewModel";
import { mapPrerequisiteReportDto } from "./gamePrerequisiteViewModel";

export function useGamePrerequisites(gameId: GameId) {
  const [state, setState] = useState<GamePrerequisiteLoadState>({ status: "loading" });

  const refresh = useCallback(async () => {
    setState({ status: "loading" });

    try {
      const dto = await getGamePrerequisiteStatus(gameId);
      setState(mapPrerequisiteReportDto(dto));
    } catch (error) {
      const mapped = mapCommandError(error);
      setState({
        status: "rules_unavailable",
        errorCode: mapped.code,
        message: mapped.message,
      });
    }
  }, [gameId]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return {
    state,
    refresh,
  };
}
