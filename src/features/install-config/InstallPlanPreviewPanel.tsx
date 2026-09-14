import { AlertTriangle, RotateCcw } from "lucide-react";
import { resolveCopy, useI18n } from "../../shared/i18n";
import type { InstallPlanPreview } from "../mods/modInstallPlanTypes";
import { getPrerequisiteDecisionCodeLabel, getPrerequisiteDecisionMessage } from "../mods/modPrerequisiteDecision";
import type { ModLifecycleCopy } from "../mods/modLifecycleCopy";
import { RetargetPopover } from "../replacements/RetargetPopover";
import type { InstallConfigCopy } from "./installConfigCopy";
import { installConfigLayoutCopy } from "./installConfigLayoutCopy";
import { summarizeInstallTargets, type InstallPlanPreviewFailure } from "./installPlanPreview";

export type InstallPlanPreviewState =
  | { status: "loading" }
  | { status: "ready"; preview: InstallPlanPreview }
  | { status: "failed"; failure: InstallPlanPreviewFailure };

type InstallPlanPreviewPanelProps = {
  state: InstallPlanPreviewState; copy: InstallConfigCopy; prerequisiteCopy: ModLifecycleCopy["prerequisite"];
  driftCount: number; saving: boolean; onSaveAndRefresh: () => void; onRetry: () => void;
};

/** 计数只来自后端已保存计划；草稿漂移与阻断常驻，安装位置按需查看。 */
export function InstallPlanPreviewPanel({ state, copy, prerequisiteCopy, driftCount, saving, onSaveAndRefresh, onRetry }: InstallPlanPreviewPanelProps) {
  const { locale } = useI18n();
  const layoutCopy = resolveCopy(installConfigLayoutCopy, locale);
  const targets = state.status === "ready" ? summarizeInstallTargets(state.preview.actions, 4) : [];
  return <section className="install-config__plan" aria-label={copy.plan.heading}>
    <div className="install-config__plan-head">
      {state.status === "loading" ? <span role="status">{copy.plan.loading}</span> : state.status === "failed" ? (
        state.failure === "needs-content-root" ? <span role="status">{copy.plan.needsContentRoot}</span> : <span role="alert">{copy.plan.failed} <button type="button" className="install-config__button is-compact" disabled={saving} onClick={onRetry}><RotateCcw size={13} aria-hidden="true" />{copy.plan.retry}</button></span>
      ) : <>
        <span className={`install-config__plan-count${driftCount > 0 ? " is-stale" : ""}`} role="status">{state.preview.actions.length === 0 ? copy.plan.empty : copy.plan.actionCount(state.preview.actions.length)}</span>
        {targets.length > 0 && <RetargetPopover title={copy.plan.heading} trigger={layoutCopy.planDetails}>
          <p>{copy.plan.targetsLabel}</p>
          <ul>{targets.map((group) => <li key={group.prefix}>{copy.plan.targetGroup(group)}</li>)}</ul>
        </RetargetPopover>}
      </>}
    </div>
    {driftCount > 0 && <div className="install-config__plan-stale" role="status"><AlertTriangle size={14} aria-hidden="true" />
      <span>{copy.plan.stale(driftCount)}</span><button type="button" className="install-config__button is-compact" disabled={saving} onClick={onSaveAndRefresh}>{copy.plan.staleAction}</button>
    </div>}
    {state.status === "ready" && <PlanPrerequisite preview={state.preview} prerequisiteCopy={prerequisiteCopy} />}
  </section>;
}

function PlanPrerequisite({ preview, prerequisiteCopy }: { preview: InstallPlanPreview; prerequisiteCopy: ModLifecycleCopy["prerequisite"] }) {
  const { prerequisiteDecision } = preview;
  if (prerequisiteDecision.status === "ready") return null;
  return <div className={`install-config__plan-prerequisite ${prerequisiteDecision.status === "blocked" ? "is-danger" : "is-warning"}`} role="alert">
    <AlertTriangle size={14} aria-hidden="true" />
    <span>{getPrerequisiteDecisionMessage(prerequisiteDecision, prerequisiteCopy)}
      {prerequisiteDecision.codes.length > 0 ? ` ${prerequisiteDecision.codes.map((code) => getPrerequisiteDecisionCodeLabel(code, prerequisiteCopy)).join("；")}` : ""}
    </span>
  </div>;
}
