import { useEffect, useRef, useState } from "react";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { PluginSelectionPanel } from "../install-plugins/PluginSelectionPanel";
import { pluginSelectionCopy } from "../install-plugins/pluginSelectionCopy";
import { usePluginSelection } from "../install-plugins/usePluginSelection";
import { InstallPlanDetailSheet, type InstallPlanDetailSheetState } from "./ModLifecycleFeedback";
import { previewInstallPlanForImportedMod } from "./modInstallPlanApi";
import type { GameId } from "../game-setup/gameSetupTypes";
import { modLibraryCopy, type ModLibraryCopy } from "./modLibraryCopy";

export type ModInstallPreviewTarget = {
  gameId: GameId; profileId: string; modId: string; modName: string; autoStartWithoutPlugins: boolean;
};

/** Mounted only after an install/preview action, never by cards or hover. */
export function ModInstallPreview({ target, onClose, onInstall }: {
  target: ModInstallPreviewTarget; onClose: () => void; onInstall: (revisionId?: string) => void;
}) {
  const { locale } = useI18n();
  const copy = resolveCopy(pluginSelectionCopy, locale);
  const planCopy = resolveCopy(modLibraryCopy, locale).page.planPreview;
  const [state, setState] = useState<InstallPlanDetailSheetState>({ status: "loading", modName: target.modName });
  const [refresh, setRefresh] = useState(0);
  const generation = useRef(0);
  const started = useRef(false);
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const plugins = usePluginSelection({ gameId: target.gameId, profileId: target.profileId, modId: target.modId }, {
    onInvalidated: () => { generation.current += 1; setState({ status: "loading", modName: target.modName }); },
    onSaved: () => setRefresh((value) => value + 1),
  });
  const latest = useRef({ target, onInstall, copy, planCopy }); latest.current = { target, onInstall, copy, planCopy };
  useEffect(() => {
    if (!plugins.ready || started.current) return;
    const current = ++generation.current;
    const { target: request, onInstall: install } = latest.current;
    if (request.autoStartWithoutPlugins && plugins.inventory === null && !started.current) {
      started.current = true;
      install();
      return;
    }
    setState({ status: "loading", modName: request.modName });
    void previewInstallPlanForImportedMod({ ...request, layerName: "base", layerPriority: 0 }).then((plan) => {
      if (generation.current === current) setState({ status: "ready", modName: request.modName, plan });
    }, (error: unknown) => {
      if (generation.current === current) setState({ status: "error", modName: request.modName, message: installPlanPreviewErrorMessage(error, latest.current.planCopy) });
    });
    return () => { generation.current += 1; };
  }, [plugins.ready, plugins.inventory, refresh]);
  const confirm = async () => {
    if (started.current || !plugins.ready || state.status !== "ready" || state.plan.actions.length === 0 || state.plan.hasBlockingConflicts || state.plan.prerequisiteDecision.status === "blocked") return;
    started.current = true;
    try {
      const inventory = await plugins.confirm();
      if (mounted.current) onInstall(inventory?.revisionId);
    } catch {
      started.current = false;
    }
  };
  return <InstallPlanDetailSheet state={state} onClose={() => { if (!plugins.saving) onClose(); }}>
    <PluginSelectionPanel controller={plugins} />
    <button type="button" className="install-config__button is-primary" onClick={() => void confirm()}
      disabled={!plugins.ready || state.status !== "ready" || state.plan.actions.length === 0 || state.plan.hasBlockingConflicts || state.plan.prerequisiteDecision.status === "blocked"}>
      {copy.confirm}
    </button>
  </InstallPlanDetailSheet>;
}

function installPlanPreviewErrorMessage(error: unknown, copy: ModLibraryCopy["page"]["planPreview"]) {
  const code = typeof error === "object" && error !== null && "code" in error ? error.code : null;
  switch (code) {
    case "install_planning_imported_mod_not_found": return copy.modNotFound;
    case "install_planning_imported_mod_analysis_unavailable": return copy.analysisUnavailable;
    case "install_planning_imported_mod_sandbox_unavailable":
    case "install_planning_imported_mod_file_scan_unavailable": return copy.archiveUnavailable;
    case "install_planning_imported_mod_ambiguous_content_root": return copy.ambiguousContentRoot;
    case "install_planning_game_adapter_not_found":
    case "game_id_invalid": return copy.unsupportedGame;
    default: return copy.failed;
  }
}
