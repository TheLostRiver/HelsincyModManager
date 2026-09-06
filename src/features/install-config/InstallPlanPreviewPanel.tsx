import { AlertTriangle, RotateCcw } from "lucide-react";
import type { InstallPlanPreview } from "../mods/modInstallPlanTypes";
import {
  getPrerequisiteDecisionCodeLabel,
  getPrerequisiteDecisionMessage,
} from "../mods/modPrerequisiteDecision";
import type { ModLifecycleCopy } from "../mods/modLifecycleCopy";
import type { InstallConfigCopy } from "./installConfigCopy";
import { summarizeInstallTargets, type InstallPlanPreviewFailure } from "./installPlanPreview";

/*
 * 安装计划预览（`#354` 切片 D4-4b）。
 *
 * 回答树回答不了的三个问题：实际会装几个文件、装到游戏目录的哪些位置、前置条件过不过。
 *
 * ⚠️ **草稿与预览之间有时间差，这里正面处理它。**
 *
 * 预览命令读的是**后端持久化**的状态（排除集合与内容根都在扫描器内部读取），而 D4-2 的
 * 勾选是草稿——点「保存」之前后端并不知道。于是玩家勾掉几个文件、还没保存时，预览显示的
 * 是旧状态，与他眼前的树不一致。
 *
 * 处置是**如实标记 ＋ 给一步出路**，而不是前端按草稿本地过滤 actions 伪造一个新计划。
 * 后者今天恰好等价（普通安装链路无重定向，排除只是按 `packageFileId` 做 retain，动作与
 * 包内文件逐条一一对应），但这个等价是**偶然的，不是契约**：`#336` 正在把重定向往安装
 * 链路里做，一旦接上，本地推演立刻开始说谎，而且是「装完不报错、文件落在别处」那一类。
 *
 * 宁可让玩家多点一下保存，也不要让预览与树**静默地各说各话**。
 */

export type InstallPlanPreviewState =
  | { status: "loading" }
  | { status: "ready"; preview: InstallPlanPreview }
  | { status: "failed"; failure: InstallPlanPreviewFailure };

/** 落点最多显示几组；超了就整体退一层聚合（见 `summarizeInstallTargets`）。 */
const MAX_TARGET_GROUPS = 4;

type InstallPlanPreviewPanelProps = {
  state: InstallPlanPreviewState;
  copy: InstallConfigCopy;
  prerequisiteCopy: ModLifecycleCopy["prerequisite"];
  /** 草稿与已保存记录差了几处；0 表示预览与眼前的树一致。 */
  driftCount: number;
  saving: boolean;
  onSaveAndRefresh: () => void;
  onRetry: () => void;
};

export function InstallPlanPreviewPanel({
  state,
  copy,
  prerequisiteCopy,
  driftCount,
  saving,
  onSaveAndRefresh,
  onRetry,
}: InstallPlanPreviewPanelProps) {
  return (
    <section className="install-config__plan" aria-label={copy.plan.heading}>
      <div className="install-config__plan-head">
        <span className="install-config__plan-heading">{copy.plan.heading}</span>
        {/*
         * 陈旧标记贴着预览本身，不放页脚。
         *
         * D4-2 踩过这个坑：面板一千多像素高，玩家视线停在哪儿，提示就得出现在哪儿，
         * 报去页脚等于没报。
         */}
        {driftCount > 0 ? (
          <div className="install-config__plan-stale" role="status">
            <AlertTriangle size={14} aria-hidden="true" />
            <span>{copy.plan.stale(driftCount)}</span>
            <button
              type="button"
              className="install-config__button is-primary is-compact"
              disabled={saving}
              onClick={onSaveAndRefresh}
            >
              {saving ? copy.actions.saving : copy.plan.staleAction}
            </button>
          </div>
        ) : null}
      </div>

      {state.status === "loading" ? (
        <p className="install-config__plan-status" role="status">
          {copy.plan.loading}
        </p>
      ) : state.status === "failed" ? (
        <PlanFailure failure={state.failure} copy={copy} onRetry={onRetry} />
      ) : (
        <PlanFacts preview={state.preview} copy={copy} prerequisiteCopy={prerequisiteCopy} />
      )}
    </section>
  );
}

/**
 * 失败分两档。
 *
 * 内容根未定**不当错误报**：玩家在同一个面板里已经看到内容根待指定了，再报一次
 * 「算不出计划」是噪音，还会把一个正常的待决状态说成出了问题。它也不给「重试」——
 * 重试一万次都是同一个结果，出路在上面的内容根面板里。
 */
function PlanFailure({
  failure,
  copy,
  onRetry,
}: {
  failure: InstallPlanPreviewFailure;
  copy: InstallConfigCopy;
  onRetry: () => void;
}) {
  if (failure === "needs-content-root") {
    return (
      <p className="install-config__plan-status is-pending" role="status">
        {copy.plan.needsContentRoot}
      </p>
    );
  }

  return (
    <div className="install-config__plan-status is-failed" role="alert">
      <span>{copy.plan.failed}</span>
      <button type="button" className="install-config__button is-compact" onClick={onRetry}>
        <RotateCcw size={13} aria-hidden="true" />
        {copy.plan.retry}
      </button>
    </div>
  );
}

function PlanFacts({
  preview,
  copy,
  prerequisiteCopy,
}: {
  preview: InstallPlanPreview;
  copy: InstallConfigCopy;
  prerequisiteCopy: ModLifecycleCopy["prerequisite"];
}) {
  const targets = summarizeInstallTargets(preview.actions, MAX_TARGET_GROUPS);
  const { prerequisiteDecision } = preview;

  return (
    <div className="install-config__plan-body">
      <p className="install-config__plan-count" role="status">
        {preview.actions.length === 0
          ? copy.plan.empty
          : copy.plan.actionCount(preview.actions.length)}
      </p>

      {targets.length > 0 ? (
        <div className="install-config__plan-targets">
          <span className="install-config__plan-targets-label">{copy.plan.targetsLabel}</span>
          {targets.map((group) => (
            <span key={group.prefix} className="install-config-fact install-config-fact--neutral">
              {copy.plan.targetGroup(group)}
            </span>
          ))}
        </div>
      ) : null}

      {/*
       * 前置条件只在**不 ready** 时出声。
       *
       * 一切正常时再挂一条「一切正常」只是噪音——玩家打开这个面板是来看包内容的，
       * 不是来看体检报告的。文案与码表直接复用 Mod 生命周期那一套（三语已穷尽 14 个码），
       * 不在这个 feature 里再造一份必然漂移的副本。
       */}
      {prerequisiteDecision.status !== "ready" ? (
        <div
          className={`install-config__plan-prerequisite ${
            prerequisiteDecision.status === "blocked" ? "is-danger" : "is-warning"
          }`}
          role="alert"
        >
          <AlertTriangle size={14} aria-hidden="true" />
          <span>
            {getPrerequisiteDecisionMessage(prerequisiteDecision, prerequisiteCopy)}
            {prerequisiteDecision.codes.length > 0
              ? ` ${prerequisiteDecision.codes
                  .map((code) => getPrerequisiteDecisionCodeLabel(code, prerequisiteCopy))
                  .join("；")}`
              : ""}
          </span>
        </div>
      ) : null}
    </div>
  );
}
