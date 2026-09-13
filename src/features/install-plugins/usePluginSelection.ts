import { useCallback, useEffect, useRef, useState } from "react";
import { getModPluginSelection, selectionInput, setModPluginSelection } from "./pluginSelectionApi";
import type { PluginInventory, PluginSelectionScope } from "./pluginSelectionTypes";

type State = { scope: string; status: "loading" | "ready" | "failed"; inventory: PluginInventory | null; error: unknown; saving: boolean; dirty: boolean };
export type PluginSelectionController = ReturnType<typeof usePluginSelection>;

export function usePluginSelection(scope: PluginSelectionScope | null, options: {
  draft?: boolean; onInvalidated?: () => void; onSaved?: () => void;
} = {}) {
  const key = JSON.stringify(scope);
  const latest = useRef({ scope, options }); latest.current = { scope, options };
  const generation = useRef(0);
  const active = useRef(false);
  const savingRef = useRef(false);
  const baseline = useRef<PluginInventory | null>(null);
  const [reloadKey, setReloadKey] = useState(0);
  const [state, setState] = useState<State>({ scope: key, status: "loading", inventory: null, error: null, saving: false, dirty: false });
  const stateRef = useRef(state); stateRef.current = state;
  useEffect(() => {
    active.current = true;
    const current = ++generation.current;
    savingRef.current = false;
    baseline.current = null;
    const currentScope = latest.current.scope;
    setState({ scope: key, status: currentScope ? "loading" : "ready", inventory: null, error: null, saving: false, dirty: false });
    if (currentScope) void getModPluginSelection(currentScope).then((inventory) => {
      if (generation.current === current) {
        baseline.current = inventory;
        setState({ scope: key, status: "ready", inventory, error: null, saving: false, dirty: false });
      }
    }, (error: unknown) => {
      if (generation.current === current) setState({ scope: key, status: "failed", inventory: null, error, saving: false, dirty: false });
    });
    return () => { active.current = false; generation.current += 1; };
  }, [key, reloadKey]);

  const persist = useCallback(async (inventory: PluginInventory, invalidate: boolean) => {
    const current = generation.current;
    const scopeKey = JSON.stringify(latest.current.scope);
    if (!active.current || savingRef.current) throw { code: "plugin_selection_unavailable" };
    savingRef.current = true;
    if (invalidate) latest.current.options.onInvalidated?.();
    setState((value) => ({ ...value, saving: true, error: null }));
    try {
      const saved = await setModPluginSelection(selectionInput(inventory));
      if (generation.current !== current || scopeKey !== JSON.stringify(latest.current.scope)) throw { code: "plugin_inventory_changed" };
      baseline.current = saved;
      const next: State = { scope: scopeKey, status: "ready", inventory: saved, error: null, saving: false, dirty: false };
      stateRef.current = next;
      setState(next);
      if (invalidate) latest.current.options.onSaved?.();
      return saved;
    } catch (error) {
      if (generation.current === current) setState((value) => ({ ...value, error, saving: false }));
      throw error;
    } finally {
      if (generation.current === current) savingRef.current = false;
    }
  }, []);

  const choose = async (fileId: string, selected: boolean) => {
    const current = stateRef.current;
    if (!active.current || savingRef.current || current.saving || current.status !== "ready" || current.scope !== key || !current.inventory) return;
    const candidate = current.inventory.files.find((file) => file.fileId === fileId);
    if (!candidate?.selectable) return;
    const inventory = { ...current.inventory, files: current.inventory.files.map((file) => file.fileId === fileId ? { ...file, selected } : file) };
    if (latest.current.options.draft) {
      const dirty = inventory.files.some((file) => baseline.current?.files.find((previous) => previous.fileId === file.fileId)?.selected !== file.selected);
      const next = { ...current, inventory, dirty, error: null };
      stateRef.current = next;
      setState(next);
      return;
    }
    await persist(inventory, true);
  };

  const confirm = async () => {
    const current = stateRef.current;
    if (!active.current || current.scope !== key || current.status !== "ready" || savingRef.current || current.saving || current.error) throw { code: "plugin_selection_unavailable" };
    if (!current.inventory) return null;
    if (current.dirty || current.inventory.confirmationRequired) return persist(current.inventory, false);
    return current.inventory;
  };
  const ready = state.scope === key && state.status === "ready" && !state.saving && !state.error;
  const visible: State = state.scope === key ? state : { scope: key, status: scope ? "loading" : "ready", inventory: null, error: null, saving: false, dirty: false };
  const dirtyCount = visible.dirty ? visible.inventory?.files.filter((file) => baseline.current?.files.find((previous) => previous.fileId === file.fileId)?.selected !== file.selected).length ?? 0 : 0;
  const discard = () => {
    if (savingRef.current || stateRef.current.scope !== key) return;
    const next = { ...stateRef.current, inventory: baseline.current, dirty: false, error: null };
    stateRef.current = next; setState(next);
  };
  const reload = useCallback(() => {
    if (!active.current || savingRef.current) return;
    generation.current += 1;
    latest.current.options.onInvalidated?.();
    const next: State = { ...stateRef.current, status: "loading", error: null };
    stateRef.current = next; setState(next);
    setReloadKey((value) => value + 1);
  }, []);
  return { ...visible, ready, choose, confirm, dirtyCount, discard, draft: options.draft ?? false, reload };
}
