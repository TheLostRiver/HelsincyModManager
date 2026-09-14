import { useEffect, useState } from "react";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { modLifecycleCopy } from "../mods/modLifecycleCopy";
import { modReinstallCopy } from "../mods/modReinstallCopy";
import { getReinstallBlockingReasonLabel } from "../mods/modReinstallTaskState";
import { getPrerequisiteDecisionCodeLabel, getPrerequisiteDecisionMessage } from "../mods/modPrerequisiteDecision";
import { ReplacementTargetPanel, type ReplacementTargetPanelProps } from "./ReplacementTargetPanel";
import { ReplacementContextPanel } from "./ReplacementContextPanel";
import { RetargetAttachmentNotice } from "./RetargetAttachmentNotice";
import { RetargetFileDetails } from "./RetargetFileDetails";
import { RetargetPluginSelection } from "./RetargetPluginSelection";
import { retargetFileCopy } from "./retargetFileCopy";
import { getEquipmentRetargetConfiguration } from "./equipmentRetargetApi";
import type { EquipmentRetargetConfiguration } from "./equipmentRetargetTypes";
import { replacementCopy } from "./replacementCopy";
import { replacementErrorMessage } from "./replacementErrorText";
import { replacementIdentityLabel } from "./replacementIdentityLabel";
import { canCancelRetargetInstallTaskPhase } from "./replacementWorkflow";
import { useEquipmentRetargetWorkflow } from "./useEquipmentRetargetWorkflow";
import { RetargetWorkspace } from "./RetargetWorkspace";
import { EquipmentSourceSelection } from "./EquipmentSourceSelection";
import "./EquipmentRetargetPanel.css";

type LoadState = { scope: string; data: EquipmentRetargetConfiguration } | { scope: string; error: unknown } | null;

/** Only mounted when the user opens the retargeting dialog. */
export function EquipmentRetargetPanel(props: ReplacementTargetPanelProps) {
  const { locale } = useI18n();
  const copy = resolveCopy(replacementCopy, locale);
  const { gameId, profileId, modId } = props;
  const scope = JSON.stringify([gameId, profileId, modId]);
  const [load, setLoad] = useState<LoadState>(null);
  const [retry, setRetry] = useState(0);
  useEffect(() => {
    let disposed = false;
    setLoad(null);
    void getEquipmentRetargetConfiguration({ gameId, profileId, modId }).then(
      (data) => { if (!disposed) setLoad({ scope, data }); },
      (error: unknown) => { if (!disposed) setLoad({ scope, error }); },
    );
    return () => { disposed = true; };
  }, [gameId, modId, profileId, scope, retry]);
  if (load?.scope !== scope) return <div className="replacement-panel__state" role="status">{copy.panel.analyzing}</div>;
  if ("error" in load) return <div className="replacement-panel">
    <ReplacementContextPanel gameId={gameId} modId={modId} profileId={profileId} installStatus={props.installStatus} reloadKey={retry} targets={[]}
      onRetry={() => setRetry((value) => value + 1)} />
    <div className="replacement-panel__state is-error" role="alert">
      {replacementErrorMessage(load.error, copy.events.analysisFallback, copy.errors)}
      <button type="button" onClick={() => setRetry((value) => value + 1)}>{copy.panel.retry}</button>
    </div>
  </div>;
  if (load.data.sources.length <= 1) return <ReplacementTargetPanel key={scope} {...props} />;
  return <EquipmentRetargetGroup key={scope} {...props} initialConfiguration={load.data} />;
}

export function EquipmentRetargetGroup({ initialConfiguration, ...props }: ReplacementTargetPanelProps & { initialConfiguration: EquipmentRetargetConfiguration }) {
  const { locale } = useI18n();
  const copy = resolveCopy(replacementCopy, locale);
  const fileCopy = resolveCopy(retargetFileCopy, locale);
  const prerequisiteCopy = resolveCopy(modLifecycleCopy, locale).prerequisite;
  const reinstallCopy = resolveCopy(modReinstallCopy, locale).task;
  const workflow = useEquipmentRetargetWorkflow(props, initialConfiguration, copy);
  const { configuration, preview, task } = workflow;
  const switching = props.installStatus === "installed";
  const prerequisite = preview.status === "ready" ? preview.value.prerequisiteDecision : null;
  const warnings = preview.status === "ready" && preview.mode === "initial" ? [...new Set(preview.value.warnings)] : [];
  const failedSource = preview.status === "error" ? configuration.sources.find(({ source }) => source.id === preview.sourceId) : undefined;
  return <RetargetWorkspace previewStatus={preview.status} selection={<EquipmentSourceSelection configuration={configuration} choices={workflow.choices}
    installStatus={props.installStatus} disabled={workflow.busy || task.status === "completed"} onChoose={workflow.choose}
    tools={<RetargetPluginSelection controller={workflow.plugins} disabled={workflow.busy || task.status === "completed"} />} />} preview={<>
    {preview.status === "loading" && <p role="status">{copy.panel.previewLoading}</p>}
    {preview.status === "error" && <p className="replacement-panel__notice" role="alert">
      <span>{failedSource && <strong>{replacementIdentityLabel(failedSource.source, locale)}: </strong>}{preview.message}</span>
    </p>}
    {preview.status === "ready" && <div className="replacement-panel__preview" aria-live="polite">
      <div className="replacement-panel__section-heading"><h3>{preview.mode === "reapply" ? fileCopy.reapplyTitle : preview.mode === "initial" ? copy.panel.initialPreviewTitle : copy.panel.switchPreviewTitle}</h3></div>
      {preview.mode === "reapply" && <p className="retarget-reapply-hint">{fileCopy.reapplyHint}</p>}
      {preview.mode === "initial" ? <>
        <p>{copy.panel.actionCount(preview.value.installPlan.actions.length)}</p>
        <p>{preview.value.installPlan.hasBlockingConflicts
          ? copy.panel.blockingConflicts(preview.value.installPlan.conflicts.length) : copy.panel.noBlockingConflicts}</p>
        {preview.value.installPlan.hasBlockingConflicts && <p>{copy.panel.blockingConflictHint}</p>}
      </> : <>
        {preview.value.status === "no_changes" && <div className="replacement-panel__inline-state is-success" role="status">{fileCopy.noChanges}</div>}
        <RetargetAttachmentNotice counts={preview.value.attachmentCounts} />
        <dl className="replacement-panel__counts">
          <div><dt>{copy.panel.countRetained}</dt><dd>{preview.value.counts.retained}</dd></div>
          <div><dt>{copy.panel.countReplaced}</dt><dd>{preview.value.counts.replaced}</dd></div>
          <div><dt>{copy.panel.countAdded}</dt><dd>{preview.value.counts.added}</dd></div>
          <div><dt>{copy.panel.countStale}</dt><dd>{preview.value.counts.stale}</dd></div>
        </dl>
        {preview.value.blockingReasons.map((reason) => <p key={reason.code}>{getReinstallBlockingReasonLabel(reason.code, reinstallCopy)}</p>)}
      </>}
      {prerequisite && <div aria-label={copy.panel.prerequisiteResultsAria}>
        <p>{getPrerequisiteDecisionMessage(prerequisite, prerequisiteCopy)}</p>
        {prerequisite.codes.map((code) => <p key={code}>{getPrerequisiteDecisionCodeLabel(code, prerequisiteCopy)}</p>)}
      </div>}
      {warnings.length > 0 && <ul aria-label={copy.panel.warningsAria}>{warnings.map((warning) => <li key={warning}>{copy.warnings[warning]}</li>)}</ul>}
      <RetargetFileDetails defaultOpen files={preview.value.fileEffects} sourceLabels={Object.fromEntries(configuration.sources.map(({ source }) => [source.id, replacementIdentityLabel(source, locale)]))} />
    </div>}
    </>} feedback={<>
    {switching && configuration.installedTargets !== null && Object.keys(configuration.installedTargets).length === 0
      && <p className="replacement-panel__notice" role="status">{copy.panel.originRecoveryHint}</p>}
    {workflow.block && <p className="replacement-panel__notice is-blocked" role="status">{workflow.block}</p>}
    {workflow.listener === "failed" && <div className="replacement-panel__notice" role="alert">
      {copy.panel.listenerUnavailable}<button type="button" onClick={workflow.retryListener}>{copy.panel.retryListener}</button>
    </div>}
    {task.status !== "idle" && <p role="status">{task.status === "starting" ? copy.panel.startingInstall
      : task.status === "failed" ? task.message : copy.phases[task.phase]}</p>}
    {workflow.refresh === "refreshing" && <p role="status">{copy.panel.refreshing}</p>}
    {workflow.refresh === "failed" && <div className="replacement-panel__notice" role="alert">
      {copy.events.refreshFailed}<button type="button" onClick={() => void workflow.refreshCompleted()}>{copy.panel.retryRefresh}</button>
    </div>}
    {workflow.cancelError && <p role="alert">{workflow.cancelError}</p>}
    </>} actions={<>
      <button type="button" className="is-secondary" disabled={!workflow.canPreview || preview.status === "loading"} onClick={() => void workflow.createPreview()}>
        {switching ? copy.panel.previewSwitch : copy.panel.generatePreview}
      </button>
      <button type="button" className="is-primary" disabled={!workflow.canStart} onClick={() => void workflow.start()}>
        {preview.status === "ready" && preview.mode === "reapply" ? fileCopy.confirmReapply : switching ? copy.panel.confirmSwitch : copy.panel.installToTarget}
      </button>
      {switching && <button type="button" className="is-secondary" disabled={!workflow.canReapply || preview.status === "loading"} onClick={() => void workflow.createReapplyPreview()}>{fileCopy.previewReapply}</button>}
      {task.status === "running" && canCancelRetargetInstallTaskPhase(task.phase) && <button type="button" className="is-secondary"
        disabled={workflow.cancel === "requesting"} onClick={() => void workflow.cancelTask()}>
        {workflow.cancel === "requesting" ? copy.panel.cancelling : copy.panel.cancelTask}
      </button>}
    </>} />;
}
