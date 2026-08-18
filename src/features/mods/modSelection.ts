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

export type ModSelectionNotice = {
  code: ModSelectionNoticeCode;
  message: string;
};

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
  | { type: "reset-context"; reason: string }
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
    message: `每批最多选择 ${MAX_MOD_SELECTION_COUNT} 个 Mod，取消一项后可继续添加。`,
  };
}

function pageLimitNotice(message: string): ModSelectionNotice {
  return {
    code: "mod_selection_page_limit_exceeded",
    message,
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
          message: selectedCount > 0
            ? `已退出批量选择，并清空 ${selectedCount} 项选择。`
            : "已退出批量选择。",
        },
      };
    }
    case "clear-selection": {
      if (state.selectedIds.size === 0) {
        return state;
      }
      const selectedCount = state.selectedIds.size;
      return {
        ...state,
        selectedIds: new Set<string>(),
        notice: {
          code: "mod_selection_cleared",
          message: `已清空 ${selectedCount} 项选择。`,
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
          message: selectedCount > 0
            ? `${action.reason}，已清空 ${selectedCount} 项选择。`
            : `${action.reason}，已退出批量选择。`,
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
          notice: pageLimitNotice(
            `选择本页需要新增 ${newIds.length} 项，当前仅剩 ${remainingSlots} 个名额。`,
          ),
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
          notice: pageLimitNotice(
            `反选本页后将选择 ${selectedIds.size} 项，超过每批 ${MAX_MOD_SELECTION_COUNT} 项上限。`,
          ),
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
