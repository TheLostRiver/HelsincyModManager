# Game Directory Settings Frontend Appendix

本附录服务于 `2026-06-04-game-directory-settings-implementation.md` 的 Task 8，提供前端 `features/game-setup` 的完整代码片段。执行 Task 8 时以本附录为准。

## Task 8: Frontend Game Setup Feature

**Files:**

- Create: `src/features/game-setup/gameSetupTypes.ts`
- Create: `src/features/game-setup/gameSetupApi.ts`
- Create: `src/features/game-setup/gameSetupViewModel.ts`
- Create: `src/features/game-setup/useGameSetup.ts`
- Create: `src/features/game-setup/GameDirectoryActions.tsx`
- Modify: `src/shared/api/tauri.ts`
- Test: `cmd /c corepack pnpm run typecheck`

- [ ] **Step 1: Add frontend types**

Create `src/features/game-setup/gameSetupTypes.ts`:

```ts
export type GameId = "mhw";

export type GameSetupErrorCode =
  | "unsupported_game"
  | "directory_not_found"
  | "missing_executable"
  | "storage_failed"
  | "storage_corrupted"
  | "scan_not_implemented"
  | "unknown";

export type GameSetupStatusDto = {
  gameId: string;
  kind: "not_configured" | "invalid" | "configured";
  displayName: string | null;
  pathLabel: string | null;
  errorCode: GameSetupErrorCode | null;
  message: string | null;
};

export type GameDirectoryEvidenceDto = {
  kind: string;
  label: string;
};

export type GameDirectoryValidationDto = {
  gameId: string;
  isValid: boolean;
  confidence: number;
  evidence: GameDirectoryEvidenceDto[];
  errors: GameSetupErrorCode[];
  pathLabel: string;
};

export type CommandErrorDto = {
  code: GameSetupErrorCode;
  message: string;
};

export type GameSetupStatus =
  | { kind: "not_configured"; gameId: GameId }
  | { kind: "validating"; gameId: GameId }
  | { kind: "invalid"; gameId: GameId; errorCode: GameSetupErrorCode; message: string }
  | { kind: "configured"; gameId: GameId; displayName: string; pathLabel: string };
```

- [ ] **Step 2: Add typed API wrapper**

Create `src/features/game-setup/gameSetupApi.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";
import type { GameDirectoryValidationDto, GameId, GameSetupStatusDto } from "./gameSetupTypes";

export async function getGameSetupStatus(gameId: GameId): Promise<GameSetupStatusDto> {
  return invoke<GameSetupStatusDto>("get_game_setup_status", { gameId });
}

export async function validateGameDirectory(gameId: GameId, directory: string): Promise<GameDirectoryValidationDto> {
  return invoke<GameDirectoryValidationDto>("validate_game_directory", { gameId, directory });
}

export async function saveGameDirectory(gameId: GameId, directory: string): Promise<GameSetupStatusDto> {
  return invoke<GameSetupStatusDto>("save_game_directory", { gameId, directory });
}

export async function scanGameCandidates(gameId: GameId): Promise<void> {
  return invoke<void>("scan_game_candidates", { gameId });
}
```

- [ ] **Step 3: Re-export API from shared tauri module**

Modify `src/shared/api/tauri.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";
import type { AppHealth } from "../types/app";

export async function getAppHealth(): Promise<AppHealth> {
  return invoke<AppHealth>("app_health");
}

export {
  getGameSetupStatus,
  saveGameDirectory,
  scanGameCandidates,
  validateGameDirectory,
} from "../../features/game-setup/gameSetupApi";
```

- [ ] **Step 4: Add view model helpers**

Create `src/features/game-setup/gameSetupViewModel.ts`:

```ts
import type { CommandErrorDto, GameId, GameSetupErrorCode, GameSetupStatus, GameSetupStatusDto } from "./gameSetupTypes";

export function mapStatusDto(dto: GameSetupStatusDto): GameSetupStatus {
  const gameId = normalizeGameId(dto.gameId);

  if (dto.kind === "configured") {
    return {
      kind: "configured",
      gameId,
      displayName: dto.displayName ?? "Monster Hunter: World - Iceborne",
      pathLabel: dto.pathLabel ?? ".../Monster Hunter World",
    };
  }

  if (dto.kind === "invalid") {
    return {
      kind: "invalid",
      gameId,
      errorCode: dto.errorCode ?? "unknown",
      message: dto.message ?? messageForError(dto.errorCode ?? "unknown"),
    };
  }

  return { kind: "not_configured", gameId };
}

export function mapCommandError(error: unknown): CommandErrorDto {
  if (isCommandErrorDto(error)) {
    return error;
  }

  return {
    code: "unknown",
    message: "操作失败，请稍后重试。",
  };
}

export function messageForError(code: GameSetupErrorCode): string {
  switch (code) {
    case "unsupported_game":
      return "当前版本暂不支持该游戏。";
    case "directory_not_found":
      return "所选目录不存在。";
    case "missing_executable":
      return "所选目录缺少 MonsterHunterWorld.exe。";
    case "storage_failed":
      return "配置保存失败，请检查应用数据目录权限。";
    case "storage_corrupted":
      return "配置文件已损坏，请先处理应用数据目录中的 games.json。";
    case "scan_not_implemented":
      return "自动扫描 Steam 尚未启用，请先手动选择目录。";
    case "unknown":
      return "操作失败，请稍后重试。";
  }
}

function normalizeGameId(value: string): GameId {
  return value === "mhw" ? "mhw" : "mhw";
}

function isCommandErrorDto(value: unknown): value is CommandErrorDto {
  if (!value || typeof value !== "object") {
    return false;
  }

  return "code" in value && "message" in value;
}
```

- [ ] **Step 5: Add hook**

Create `src/features/game-setup/useGameSetup.ts`:

```ts
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
    const dto = await getGameSetupStatus(gameId);
    setState((current) => ({
      ...current,
      status: mapStatusDto(dto),
      actionMessage: null,
    }));
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
        setState({
          status: {
            kind: "invalid",
            gameId,
            errorCode: mapped.code,
            message: messageForError(mapped.code),
          },
          isBusy: false,
          actionMessage: mapped.message,
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

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return {
    status: state.status,
    isBusy: state.isBusy,
    actionMessage: state.actionMessage,
    refresh,
    saveDirectory,
    scanSteam,
  };
}
```

- [ ] **Step 6: Add directory action component**

Create `src/features/game-setup/GameDirectoryActions.tsx`:

```tsx
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, Search } from "lucide-react";

type GameDirectoryActionsProps = {
  isBusy: boolean;
  onDirectorySelected: (directory: string) => Promise<void>;
  onScanSteam: () => Promise<void>;
};

export function GameDirectoryActions({ isBusy, onDirectorySelected, onScanSteam }: GameDirectoryActionsProps) {
  async function handleManualSelect() {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "选择《怪物猎人：世界 冰原》游戏目录",
    });

    if (typeof selected === "string") {
      await onDirectorySelected(selected);
    }
  }

  return (
    <div className="setup-actions">
      <button type="button" className="primary-action" disabled={isBusy} onClick={() => void onScanSteam()}>
        <Search size={16} />
        自动扫描 Steam
      </button>
      <button type="button" className="secondary-action" disabled={isBusy} onClick={() => void handleManualSelect()}>
        <FolderOpen size={16} />
        手动选择游戏目录
      </button>
    </div>
  );
}
```

- [ ] **Step 7: Run frontend typecheck**

Run:

```powershell
cmd /c corepack pnpm run typecheck
```

Expected:

```text
No TypeScript errors
```

- [ ] **Step 8: Commit game setup frontend feature**

Run:

```powershell
git add src/features/game-setup src/shared/api/tauri.ts
git commit -m "feat: 添加前端游戏目录配置状态"
```
