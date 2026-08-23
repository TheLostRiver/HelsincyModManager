import { AlertTriangle, CheckCircle2, CircleAlert, RefreshCw } from "lucide-react";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { gamePrerequisiteCopy, type GamePrerequisiteCopy } from "./gamePrerequisiteCopy";
import type {
  GamePrerequisiteIssueCode,
  GamePrerequisiteItemStatus,
  GamePrerequisiteLoadState,
  GamePrerequisiteSummaryStatus,
} from "./gamePrerequisiteTypes";

type GamePrerequisitePanelProps = {
  state: GamePrerequisiteLoadState;
  onRefresh: () => Promise<void>;
  variant?: "default" | "embedded";
  tourId?: string;
};

export function GamePrerequisitePanel({
  state,
  onRefresh,
  variant = "default",
  tourId,
}: GamePrerequisitePanelProps) {
  const { locale } = useI18n();
  const copy = resolveCopy(gamePrerequisiteCopy, locale);
  const summary = summaryCopyForState(state, copy);
  const SummaryIcon = summary.icon;
  const isEmbedded = variant === "embedded";

  return (
    <section
      className={`game-prerequisite-panel${isEmbedded ? " game-prerequisite-panel--embedded" : ""}`}
      data-tour-id={tourId}
      aria-label={isEmbedded ? copy.panelTitle : undefined}
      aria-labelledby={isEmbedded ? undefined : "game-prerequisite-title"}
    >
      <div className="game-prerequisite-panel__toolbar">
        <div className="game-prerequisite-panel__heading">
          <span className={`game-prerequisite-summary ${summary.tone}`}>
            <SummaryIcon size={14} aria-hidden="true" />
            {summary.label}
          </span>
          <div className="game-prerequisite-panel__summary-copy">
            {variant === "embedded" ? null : <h4 id="game-prerequisite-title">{copy.panelTitle}</h4>}
            <p>{summary.description}</p>
          </div>
        </div>
        <button
          type="button"
          className="game-prerequisite-panel__refresh"
          disabled={state.status === "loading"}
          onClick={() => void onRefresh()}
        >
          <RefreshCw size={14} aria-hidden="true" />
          {copy.recheck}
        </button>
      </div>

      <div className="game-prerequisite-panel__content">
        {state.status === "loading" ? (
          <div className={`game-prerequisite-note ${summary.tone}`} role="status">
            <span>{copy.checking}</span>
          </div>
        ) : null}

        {state.status === "not_configured" ? (
          <div className={`game-prerequisite-note ${summary.tone}`}>
            <span>{copy.configureFirst}</span>
          </div>
        ) : null}

        {state.status === "game_directory_invalid" ||
        state.status === "game_directory_not_writable" ||
        state.status === "rules_unavailable" ? (
          <div className={`game-prerequisite-note ${summary.tone}`} role="status">
            <strong>{prerequisiteNoteHeading(state.status, copy)}</strong>
            {/* 后端给出的 message 原样透传；仅后端未给时才用当前语言的兜底文案。 */}
            <span>{state.message ?? fallbackMessageForStatus(state.status, copy)}</span>
          </div>
        ) : null}

        {state.status === "ready" ? (
          <div className="game-prerequisite-list">
            {state.items.map((item) => (
              <article key={item.id} className={`game-prerequisite-item ${statusClassName(item.status)}`}>
                <div className="game-prerequisite-item__top">
                  <div className="game-prerequisite-item__title">
                    <strong>{item.displayName}</strong>
                    {item.issues.length === 0 ? (
                      <p className="game-prerequisite-item__note">{copy.itemVerifiedNote}</p>
                    ) : null}
                  </div>
                  <span className="game-prerequisite-item__status">{statusLabel(item.status, copy)}</span>
                </div>
                {item.issues.length > 0 ? (
                  <ul className="game-prerequisite-issues">
                    {item.issues.map((issue) => (
                      <li key={`${item.id}-${issue.code}-${issue.path}`}>
                        <code>{issue.path}</code>
                        <span>{issueLabel(issue.code, copy)}</span>
                      </li>
                    ))}
                  </ul>
                ) : null}
              </article>
            ))}
          </div>
        ) : null}
      </div>
    </section>
  );
}

function summaryCopyForState(state: GamePrerequisiteLoadState, copy: GamePrerequisiteCopy) {
  if (state.status === "ready") {
    return summaryCopyForReadyState(state.summaryStatus, copy);
  }

  if (state.status === "loading") {
    return { ...copy.summary.loading, tone: "is-loading", icon: CircleAlert };
  }

  if (state.status === "rules_unavailable") {
    return { ...copy.summary.rulesUnavailable, tone: "is-error", icon: AlertTriangle };
  }

  if (state.status === "game_directory_invalid") {
    return { ...copy.summary.directoryInvalid, tone: "is-error", icon: AlertTriangle };
  }

  if (state.status === "game_directory_not_writable") {
    // 目录本身是对的，别复用"目录失效"让用户去重选目录。
    return { ...copy.summary.directoryNotWritable, tone: "is-error", icon: AlertTriangle };
  }

  return { ...copy.summary.notConfigured, tone: "is-warning", icon: CircleAlert };
}

function summaryCopyForReadyState(
  summaryStatus: GamePrerequisiteSummaryStatus,
  copy: GamePrerequisiteCopy,
) {
  switch (summaryStatus) {
    case "verified":
      return { ...copy.summary.verified, tone: "is-success", icon: CheckCircle2 };
    case "warning":
      return { ...copy.summary.warning, tone: "is-warning", icon: CircleAlert };
    case "error":
      return { ...copy.summary.error, tone: "is-error", icon: AlertTriangle };
  }
}

/** 三种阻断状态的标题各不相同：目录坏了 / 目录写不进去 / 规则读不到，用户要做的事不一样。 */
function prerequisiteNoteHeading(
  status: "game_directory_invalid" | "game_directory_not_writable" | "rules_unavailable",
  copy: GamePrerequisiteCopy,
) {
  switch (status) {
    case "rules_unavailable":
      return copy.noteHeading.rulesUnavailable;
    case "game_directory_not_writable":
      return copy.noteHeading.directoryNotWritable;
    case "game_directory_invalid":
      return copy.noteHeading.directoryInvalid;
  }
}

function fallbackMessageForStatus(
  status: "game_directory_invalid" | "game_directory_not_writable" | "rules_unavailable",
  copy: GamePrerequisiteCopy,
) {
  switch (status) {
    case "rules_unavailable":
      return copy.fallbackMessage.rulesUnavailable;
    case "game_directory_not_writable":
      return copy.fallbackMessage.directoryNotWritable;
    case "game_directory_invalid":
      return copy.fallbackMessage.directoryInvalid;
  }
}

function statusClassName(status: GamePrerequisiteItemStatus) {
  switch (status) {
    case "missing":
    case "misconfigured":
      return "is-error";
    case "installed_unverified":
      return "is-warning";
    case "installed_verified":
      return "is-success";
  }
}

function statusLabel(status: GamePrerequisiteItemStatus, copy: GamePrerequisiteCopy) {
  switch (status) {
    case "missing":
      return copy.itemStatus.missing;
    case "misconfigured":
      return copy.itemStatus.misconfigured;
    case "installed_verified":
      return copy.itemStatus.installedVerified;
    case "installed_unverified":
      return copy.itemStatus.installedUnverified;
  }
}

function issueLabel(code: GamePrerequisiteIssueCode, copy: GamePrerequisiteCopy) {
  switch (code) {
    case "missing_required_file":
      return copy.issue.missingRequiredFile;
    case "signature_unverified":
      return copy.issue.signatureUnverified;
    case "config_read_failed":
      return copy.issue.configReadFailed;
    case "config_invalid_json":
      return copy.issue.configInvalidJson;
    case "config_field_mismatch":
      return copy.issue.configFieldMismatch;
    case "rules_unavailable":
      return copy.issue.rulesUnavailable;
    case "rules_corrupted":
      return copy.issue.rulesCorrupted;
  }
}
