export const MAX_MOD_SELECTION_COUNT = 100;

export type ModSelectionSetMode = "replace" | "toggle";
export type ModSelectionMode = "single" | "batch";

export type ModCardSelectionIntent =
  | { kind: "primary"; modId: string; source: "pointer" | "keyboard" }
  | {
      kind: "toggle";
      modId: string;
      source: "ctrl-pointer" | "ctrl-keyboard";
    };

export type ModSelectionNoticeCode =
  | "mod_selection_limit_reached"
  | "mod_selection_page_limit_exceeded"
  | "mod_selection_cleared"
  | "mod_selection_context_reset";

// I18N-02 起 notice 只携带语义码与参数，不携带渲染文本：文本在 UI 层按当前界面语言
// 从 modLibraryCopy.selection 取词（renderModSelectionNotice），reducer 保持 locale 无关。
export type ModSelectionResetReason =
  | "query-changed"
  | "filters-changed"
  | "search-changed"
  | "query-reset"
  | "library-refreshed"
  | "profile-changed"
  | "batch-completed";

export type ModSelectionNotice =
  | { code: "mod_selection_limit_reached"; maxCount: number }
  | {
      code: "mod_selection_page_limit_exceeded";
      variant: "select-page";
      newCount: number;
      remainingSlots: number;
    }
  | {
      code: "mod_selection_page_limit_exceeded";
      variant: "invert-page";
      resultCount: number;
      maxCount: number;
    }
  | { code: "mod_selection_cleared"; clearedCount: number; exitedBatch: boolean }
  | { code: "mod_selection_context_reset"; reason: ModSelectionResetReason; clearedCount: number };

export type ModLibrarySelectionState = {
  mode: ModSelectionMode;
  selectedIds: ReadonlySet<string>;
  notice: ModSelectionNotice | null;
};

export type ModSelectionAction =
  | { type: "apply-intent"; intent: ModCardSelectionIntent }
  | { type: "enter-batch" }
  | { type: "exit-batch" }
  | { type: "clear-selection" }
  | { type: "dismiss-notice" }
  | { type: "reset-context"; reason: ModSelectionResetReason }
  | { type: "select-page"; modIds: readonly string[] }
  | { type: "invert-page"; modIds: readonly string[] };

export function createInitialModSelectionState(): ModLibrarySelectionState {
  return {
    mode: "single",
    selectedIds: new Set<string>(),
    notice: null,
  };
}

export function applyModSelection(
  previousSelection: ReadonlySet<string>,
  selectedId: string,
  mode: ModSelectionSetMode,
) {
  if (mode === "toggle") {
    const nextSelection = new Set(previousSelection);
    if (nextSelection.has(selectedId)) {
      nextSelection.delete(selectedId);
    } else {
      nextSelection.add(selectedId);
    }
    return nextSelection;
  }

  if (previousSelection.size === 1 && previousSelection.has(selectedId)) {
    return new Set<string>();
  }

  return new Set([selectedId]);
}

function uniqueIds(modIds: readonly string[]) {
  return [...new Set(modIds)];
}

function selectionLimitNotice(): ModSelectionNotice {
  return {
    code: "mod_selection_limit_reached",
    maxCount: MAX_MOD_SELECTION_COUNT,
  };
}

function toggleSelectedId(
  state: ModLibrarySelectionState,
  modId: string,
  mode: ModSelectionMode,
): ModLibrarySelectionState {
  if (!state.selectedIds.has(modId) && state.selectedIds.size >= MAX_MOD_SELECTION_COUNT) {
    return { ...state, mode, notice: selectionLimitNotice() };
  }

  return {
    mode,
    selectedIds: applyModSelection(state.selectedIds, modId, "toggle"),
    notice: null,
  };
}

export function reduceModSelection(
  state: ModLibrarySelectionState,
  action: ModSelectionAction,
): ModLibrarySelectionState {
  switch (action.type) {
    case "apply-intent": {
      const toggle = state.mode === "batch" || action.intent.kind === "toggle";
      if (toggle) {
        return toggleSelectedId(state, action.intent.modId, "batch");
      }

      return {
        mode: "single",
        selectedIds: applyModSelection(state.selectedIds, action.intent.modId, "replace"),
        notice: null,
      };
    }
    case "enter-batch":
      return state.mode === "batch" && state.notice === null
        ? state
        : { ...state, mode: "batch", notice: null };
    case "exit-batch": {
      const selectedCount = state.selectedIds.size;
      return {
        mode: "single",
        selectedIds: new Set<string>(),
        notice: {
          code: "mod_selection_cleared",
          clearedCount: selectedCount,
          exitedBatch: true,
        },
      };
    }
    case "clear-selection": {
      if (state.selectedIds.size === 0) {
        return state;
      }
      return {
        ...state,
        selectedIds: new Set<string>(),
        notice: {
          code: "mod_selection_cleared",
          clearedCount: state.selectedIds.size,
          exitedBatch: false,
        },
      };
    }
    case "dismiss-notice":
      return state.notice === null ? state : { ...state, notice: null };
    case "reset-context": {
      const selectedCount = state.selectedIds.size;
      if (selectedCount === 0 && state.mode === "single") {
        return state;
      }

      return {
        mode: "single",
        selectedIds: new Set<string>(),
        notice: {
          code: "mod_selection_context_reset",
          reason: action.reason,
          clearedCount: selectedCount,
        },
      };
    }
    case "select-page": {
      if (state.mode !== "batch") {
        return state;
      }

      const newIds = uniqueIds(action.modIds).filter((modId) => !state.selectedIds.has(modId));
      const remainingSlots = MAX_MOD_SELECTION_COUNT - state.selectedIds.size;
      if (newIds.length > remainingSlots) {
        return {
          ...state,
          notice: {
            code: "mod_selection_page_limit_exceeded",
            variant: "select-page",
            newCount: newIds.length,
            remainingSlots,
          },
        };
      }

      const selectedIds = new Set(state.selectedIds);
      for (const modId of newIds) {
        selectedIds.add(modId);
      }
      return { ...state, selectedIds, notice: null };
    }
    case "invert-page": {
      if (state.mode !== "batch") {
        return state;
      }

      const selectedIds = new Set(state.selectedIds);
      for (const modId of uniqueIds(action.modIds)) {
        if (selectedIds.has(modId)) {
          selectedIds.delete(modId);
        } else {
          selectedIds.add(modId);
        }
      }

      if (selectedIds.size > MAX_MOD_SELECTION_COUNT) {
        return {
          ...state,
          notice: {
            code: "mod_selection_page_limit_exceeded",
            variant: "invert-page",
            resultCount: selectedIds.size,
            maxCount: MAX_MOD_SELECTION_COUNT,
          },
        };
      }

      return { ...state, selectedIds, notice: null };
    }
  }
}

export function countSelectedOnPage(
  selectedIds: ReadonlySet<string>,
  pageIds: readonly string[],
) {
  let selectedCount = 0;
  for (const modId of new Set(pageIds)) {
    if (selectedIds.has(modId)) {
      selectedCount += 1;
    }
  }
  return selectedCount;
}
