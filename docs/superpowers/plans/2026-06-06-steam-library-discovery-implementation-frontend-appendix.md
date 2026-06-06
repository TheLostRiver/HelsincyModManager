# Steam Library Discovery Frontend Appendix

本附录服务于 [Steam Library Discovery Implementation Plan](2026-06-06-steam-library-discovery-implementation.md) 的前端任务，避免主计划文档过长。执行实现时，先完成主计划 Task 1-8，再按本附录执行前端任务。

## Task A: Frontend Candidate State and API

**Files:**

- Modify: `src/features/game-setup/gameSetupTypes.ts`
- Modify: `src/features/game-setup/gameSetupApi.ts`
- Modify: `src/features/game-setup/gameSetupViewModel.ts`
- Modify: `src/features/game-setup/useGameSetup.ts`
- Modify: `src/shared/api/tauri.ts`
- Test: `cmd /c corepack pnpm run typecheck`

- [ ] **Step 1: Add TypeScript scan DTOs**

In `gameSetupTypes.ts`, add:

```ts
export type GameCandidateSource = "steam";

export type GameDirectoryCandidateDto = {
  gameId: string;
  displayName: string;
  directory: string;
  pathLabel: string;
  source: GameCandidateSource;
  sourceLabel: string;
  isValid: boolean;
  confidence: number;
  evidence: GameDirectoryEvidenceDto[];
  errors: GameSetupErrorCode[];
};

export type GameCandidateScanDto = {
  gameId: string;
  candidates: GameDirectoryCandidateDto[];
};

export type GameDirectoryCandidate = GameDirectoryCandidateDto & {
  gameId: GameId;
};
```

- [ ] **Step 2: Update API return type**

In `gameSetupApi.ts`, change:

```ts
export async function scanGameCandidates(gameId: GameId): Promise<GameCandidateScanDto> {
  return invoke<GameCandidateScanDto>("scan_game_candidates", { gameId });
}
```

Ensure `src/shared/api/tauri.ts` exports the updated API.

- [ ] **Step 3: Add candidate mapper**

In `gameSetupViewModel.ts`, add:

```ts
export function mapCandidateScanDto(dto: GameCandidateScanDto): GameDirectoryCandidate[] {
  return dto.candidates.map((candidate) => ({
    ...candidate,
    gameId: normalizeGameId(candidate.gameId),
  }));
}
```

- [ ] **Step 4: Store candidates in hook state**

In `useGameSetup.ts`, extend state:

```ts
type GameSetupState = {
  status: GameSetupStatus;
  isBusy: boolean;
  actionMessage: string | null;
  candidates: GameDirectoryCandidate[];
};
```

Update initial state with `candidates: []`.

Change `scanSteam` so it stores mapped candidates:

```ts
const dto = await scanGameCandidates(gameId);
const candidates = mapCandidateScanDto(dto);
setState((current) => ({
  ...current,
  candidates,
  isBusy: false,
  actionMessage:
    candidates.length > 0 ? "已发现 Steam 候选目录。" : "未发现 Steam 候选目录，可手动选择游戏目录。",
}));
```

Return `candidates` from the hook.

- [ ] **Step 5: Run typecheck**

Run:

```powershell
cmd /c corepack pnpm run typecheck
```

Expected:

```text
No TypeScript errors
```

- [ ] **Step 6: Commit frontend state**

Run:

```powershell
git add src/features/game-setup/gameSetupTypes.ts src/features/game-setup/gameSetupApi.ts src/features/game-setup/gameSetupViewModel.ts src/features/game-setup/useGameSetup.ts src/shared/api/tauri.ts
git commit -m "feat: 接入前端候选扫描状态"
```

## Task B: Frontend Candidate List UI

**Files:**

- Create: `src/features/game-setup/GameDirectoryCandidateList.tsx`
- Create: `src/features/game-setup/GameDirectoryCandidateList.css`
- Modify: `src/features/dashboard/DashboardHeroCard.tsx`
- Modify: `src/features/dashboard/DashboardPage.tsx`
- Test: `cmd /c corepack pnpm run typecheck`
- Test: `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-frontend-boundaries.ps1`

- [ ] **Step 1: Create candidate list component**

Create `GameDirectoryCandidateList.tsx`:

```tsx
import { CheckCircle2, CircleAlert, HardDrive } from "lucide-react";
import type { GameDirectoryCandidate } from "./gameSetupTypes";
import { messageForError } from "./gameSetupViewModel";
import "./GameDirectoryCandidateList.css";

type GameDirectoryCandidateListProps = {
  candidates: GameDirectoryCandidate[];
  isBusy: boolean;
  onCandidateSelected: (directory: string) => Promise<void>;
};

export function GameDirectoryCandidateList({
  candidates,
  isBusy,
  onCandidateSelected,
}: GameDirectoryCandidateListProps) {
  if (candidates.length === 0) {
    return null;
  }

  return (
    <section className="candidate-list" aria-label="Steam 候选目录">
      {candidates.map((candidate) => (
        <article className="candidate-item" key={candidate.directory}>
          <div className="candidate-icon" aria-hidden="true">
            {candidate.isValid ? <CheckCircle2 size={18} /> : <CircleAlert size={18} />}
          </div>
          <div className="candidate-content">
            <span className="candidate-source">
              <HardDrive size={14} />
              {candidate.sourceLabel}
            </span>
            <strong>{candidate.displayName}</strong>
            <p>{candidate.pathLabel}</p>
            {!candidate.isValid && candidate.errors.length > 0 ? (
              <small>{messageForError(candidate.errors[0])}</small>
            ) : null}
          </div>
          <button
            type="button"
            className="candidate-select"
            disabled={isBusy || !candidate.isValid}
            onClick={() => void onCandidateSelected(candidate.directory)}
          >
            使用此目录
          </button>
        </article>
      ))}
    </section>
  );
}
```

- [ ] **Step 2: Add candidate styles**

Create `GameDirectoryCandidateList.css` using existing tokens:

```css
.candidate-list {
  display: grid;
  gap: 10px;
  margin-top: 14px;
}

.candidate-item {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: 12px;
  padding: 12px;
  border: 1px solid var(--color-border);
  border-radius: 8px;
  background: var(--color-surface);
}

.candidate-icon {
  display: grid;
  place-items: center;
  width: 32px;
  height: 32px;
  border-radius: 8px;
  color: var(--color-accent);
  background: var(--color-accent-alpha-12);
}

.candidate-content {
  min-width: 0;
}

.candidate-content strong,
.candidate-content p,
.candidate-content small {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.candidate-source {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  color: var(--color-text-muted);
  font-size: 12px;
}

.candidate-select {
  min-height: 36px;
}
```

- [ ] **Step 3: Pass candidates through Dashboard**

In `DashboardPage.tsx`, pass `gameSetup.candidates` into `DashboardHeroCard`.

In `DashboardHeroCard.tsx`, render:

```tsx
<GameDirectoryCandidateList
  candidates={candidates}
  isBusy={isBusy}
  onCandidateSelected={onDirectorySelected}
/>
```

Do not make Dashboard inspect `candidate.source` or `candidate.errors`.

- [ ] **Step 4: Run frontend checks**

Run:

```powershell
cmd /c corepack pnpm run typecheck
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-frontend-boundaries.ps1
```

Expected:

```text
No TypeScript errors
Frontend boundary checks passed
```

- [ ] **Step 5: Commit candidate UI**

Run:

```powershell
git add src/features/game-setup/GameDirectoryCandidateList.tsx src/features/game-setup/GameDirectoryCandidateList.css src/features/dashboard/DashboardHeroCard.tsx src/features/dashboard/DashboardPage.tsx
git commit -m "feat: 展示 Steam 游戏目录候选列表"
```
