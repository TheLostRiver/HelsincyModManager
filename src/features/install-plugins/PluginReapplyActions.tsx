import { resolveCopy, useI18n } from "../../shared/i18n";
import { modReinstallCopy } from "../mods/modReinstallCopy";
import { ReinstallPreviewSummary } from "../mods/ReinstallPlanPreviewPanel";
import { RetargetFileDetails } from "../replacements/RetargetFileDetails";
import { replacementCopy } from "../replacements/replacementCopy";
import { canCancelRetargetInstallTaskPhase } from "../replacements/replacementWorkflow";
import { pluginSelectionCopy } from "./pluginSelectionCopy";
import type { PluginReapplyWorkflow } from "./usePluginReapply";

export function PluginReapplyActions({ workflow }: { workflow: PluginReapplyWorkflow }) {
  const { locale } = useI18n();
  const copy = resolveCopy(pluginSelectionCopy, locale);
  if (!workflow.installed) return null;
  return <>
    <button type="button" className="install-config__button" disabled={!workflow.canPreview} onClick={() => void workflow.generatePreview()}>{copy.preview}</button>
    {workflow.preview && <button type="button" className="install-config__button is-primary" disabled={!workflow.canApply} onClick={() => void workflow.start()}>{copy.apply}</button>}
    {workflow.task.status === "running" && <button type="button" className="install-config__button" disabled={!canCancelRetargetInstallTaskPhase(workflow.task.phase)} onClick={() => void workflow.cancel()}>{copy.cancel}</button>}
  </>;
}

export function PluginReapplyResult({ workflow }: { workflow: PluginReapplyWorkflow }) {
  const { locale } = useI18n();
  const copy = resolveCopy(pluginSelectionCopy, locale);
  const reCopy = resolveCopy(modReinstallCopy, locale);
  return <>
    {workflow.loading && <p role="status">{reCopy.dialog.loadingPreview}</p>}
    {workflow.error && <p role="alert">{workflow.error}</p>}
    {workflow.preview && <><ReinstallPreviewSummary preview={workflow.preview} /><RetargetFileDetails files={workflow.preview.fileEffects} defaultOpen />
      {workflow.preview.status === "no_changes" && <p role="status">{copy.noChanges}</p>}
    </>}
    {!workflow.preview && !workflow.loading && !workflow.error && <PluginReapplyFeedback workflow={workflow} />}
  </>;
}

/** 关闭结果栏也不能隐藏写入进度、失败和事件监听故障。 */
export function PluginReapplyFeedback({ workflow }: { workflow: PluginReapplyWorkflow }) {
  const { locale } = useI18n();
  const reCopy = resolveCopy(modReinstallCopy, locale);
  const replacement = resolveCopy(replacementCopy, locale);
  const { task } = workflow;
  return <>
    {workflow.error && <p role="alert">{workflow.error}</p>}
    {workflow.installed && workflow.listener === "failed" && <p role="alert">{reCopy.dialog.listenerFailed} <button type="button" onClick={workflow.retryListener}>{reCopy.dialog.retryListener}</button></p>}
    {workflow.active && <p role="status">{task.status === "running" ? replacement.phases[task.phase] : reCopy.dialog.starting}</p>}
    {task.status === "completed" && <p role="status">{reCopy.dialog.completed}</p>}
    {task.status === "cancelled" && <p role="status">{reCopy.dialog.cancelled}</p>}
    {task.status === "failed" && <p role="alert">{task.message}</p>}
  </>;
}
