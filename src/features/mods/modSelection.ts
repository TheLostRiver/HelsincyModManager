export type ModSelectionMode = "replace" | "toggle";

export function applyModSelection(
  previousSelection: ReadonlySet<string>,
  selectedId: string,
  mode: ModSelectionMode,
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
