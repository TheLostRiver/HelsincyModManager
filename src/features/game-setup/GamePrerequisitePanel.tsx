import { AlertTriangle, CheckCircle2, CircleAlert, RefreshCw } from "lucide-react";
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
  const summary = summaryCopyForState(state);
  const SummaryIcon = summary.icon;
  const isEmbedded = variant === "embedded";

  return (
    <section
      className={`game-prerequisite-panel${isEmbedded ? " game-prerequisite-panel--embedded" : ""}`}
      data-tour-id={tourId}
      aria-label={isEmbedded ? "前置环境" : undefined}
      aria-labelledby={isEmbedded ? undefined : "game-prerequisite-title"}
    >
      <div className="game-prerequisite-panel__toolbar">
        <div className="game-prerequisite-panel__heading">
          <span className={`game-prerequisite-summary ${summary.tone}`}>
            <SummaryIcon size={14} aria-hidden="true" />
            {summary.label}
          </span>
          <div className="game-prerequisite-panel__summary-copy">
            {variant === "embedded" ? null : <h4 id="game-prerequisite-title">前置环境</h4>}
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
          重新检查
        </button>
      </div>

      <div className="game-prerequisite-panel__content">
        {state.status === "loading" ? (
          <div className={`game-prerequisite-note ${summary.tone}`} role="status">
            <span>正在检查前置环境…</span>
          </div>
        ) : null}

        {state.status === "not_configured" ? (
          <div className={`game-prerequisite-note ${summary.tone}`}>
            <span>配置游戏目录后即可检查前置环境。</span>
          </div>
        ) : null}

        {state.status === "game_directory_invalid" ||
        state.status === "game_directory_not_writable" ||
        state.status === "rules_unavailable" ? (
          <div className={`game-prerequisite-note ${summary.tone}`} role="status">
            <strong>{prerequisiteNoteHeading(state.status)}</strong>
            <span>{state.message}</span>
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
                      <p className="game-prerequisite-item__note">关键文件、配置和已知签名都已通过检查。</p>
                    ) : null}
                  </div>
                  <span className="game-prerequisite-item__status">{statusLabel(item.status)}</span>
                </div>
                {item.issues.length > 0 ? (
                  <ul className="game-prerequisite-issues">
                    {item.issues.map((issue) => (
                      <li key={`${item.id}-${issue.code}-${issue.path}`}>
                        <code>{issue.path}</code>
                        <span>{issueLabel(issue.code)}</span>
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

function summaryCopyForState(state: GamePrerequisiteLoadState) {
  if (state.status === "ready") {
    return summaryCopyForReadyState(state.summaryStatus);
  }

  if (state.status === "loading") {
    return {
      label: "检查中",
      description: "只读检查当前已配置游戏目录中的已知前置文件。",
      tone: "is-loading",
      icon: CircleAlert,
    };
  }

  if (state.status === "rules_unavailable") {
    return {
      label: "规则不可用",
      description: "无法完成签名校验，但不会写入游戏目录。",
      tone: "is-error",
      icon: AlertTriangle,
    };
  }

  if (state.status === "game_directory_invalid") {
    return {
      label: "目录失效",
      description: "请先修正当前保存的游戏目录，再重新检查前置环境。",
      tone: "is-error",
      icon: AlertTriangle,
    };
  }

  if (state.status === "game_directory_not_writable") {
    // 目录本身是对的，别复用"目录失效"让用户去重选目录。
    return {
      label: "目录不可写",
      description: "游戏目录存在但当前写不进去，安装会被阻止。请先关闭游戏再重试。",
      tone: "is-error",
      icon: AlertTriangle,
    };
  }

  return {
    label: "等待配置",
    description: "配置游戏目录后即可检查 Stracker's Loader 和 CRCBypass。",
    tone: "is-warning",
    icon: CircleAlert,
  };
}

function summaryCopyForReadyState(summaryStatus: GamePrerequisiteSummaryStatus) {
  switch (summaryStatus) {
    case "verified":
      return {
        label: "已验证",
        description: "两个已知前置都通过了文件、配置和签名检查。",
        tone: "is-success",
        icon: CheckCircle2,
      };
    case "warning":
      return {
        label: "存在警告",
        description: "已检测到前置文件，但至少有一个签名不在当前已知集合内。",
        tone: "is-warning",
        icon: CircleAlert,
      };
    case "error":
      return {
        label: "需要处理",
        description: "至少有一个前置缺失，或关键配置不正确。",
        tone: "is-error",
        icon: AlertTriangle,
      };
  }
}

/** 三种阻断状态的标题各不相同：目录坏了 / 目录写不进去 / 规则读不到，用户要做的事不一样。 */
function prerequisiteNoteHeading(
  status: "game_directory_invalid" | "game_directory_not_writable" | "rules_unavailable",
) {
  switch (status) {
    case "rules_unavailable":
      return "暂时无法读取前置规则。";
    case "game_directory_not_writable":
      return "游戏目录当前不可写。";
    case "game_directory_invalid":
      return "游戏目录当前不可用。";
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

function statusLabel(status: GamePrerequisiteItemStatus) {
  switch (status) {
    case "missing":
      return "缺少必需文件";
    case "misconfigured":
      return "配置不正确";
    case "installed_verified":
      return "已安装，版本已验证";
    case "installed_unverified":
      return "已安装，但版本未验证";
  }
}

function issueLabel(code: GamePrerequisiteIssueCode) {
  switch (code) {
    case "missing_required_file":
      return "缺少必需文件";
    case "signature_unverified":
      return "签名未命中当前已知集合";
    case "config_read_failed":
      return "配置文件无法读取";
    case "config_invalid_json":
      return "配置文件不是有效 JSON";
    case "config_field_mismatch":
      return "关键字段未满足 enablePluginLoader = true";
    case "rules_unavailable":
      return "规则暂不可用";
    case "rules_corrupted":
      return "规则文件已损坏";
  }
}
