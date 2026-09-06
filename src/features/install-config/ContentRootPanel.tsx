import { RotateCcw } from "lucide-react";
import { isActiveContentRoot, resolveContentRootDetailKind } from "./contentRootChoice";
import type { InstallConfigCopy } from "./installConfigCopy";
import type { PackageContents } from "./packageContentsTypes";

/*
 * 内容根面板（`#354` 切片 D4-3）。
 *
 * 「内容根」是算安装路径的起点。包里套着包装目录时它不等于沙箱根，包里有多个 `nativePC`
 * 时后端不敢替玩家挑——那正是这块 UI 存在的理由。
 *
 * 两处与勾选不同的设计：
 *
 * 1. **选中即提交，没有草稿。** 内容根一改，整棵树的 `targetPath` 与 `installable` 全部重算，
 *    而那是后端的判断（`InstallTargetPath::parse` 对着 game adapter 的允许根）。前端推演不出，
 *    只能提交后拿回读结果重绘。
 * 2. **候选清单与当前生效的根是两份数据。** `candidates` 恒为全集，玩家选定之后 `contentRoot`
 *    会收敛成 `single` 但候选**不会消失**——否则他就改不了主意了。
 */

type ContentRootPanelProps = {
  contents: PackageContents;
  copy: InstallConfigCopy;
  busy: boolean;
  failed: boolean;
  onChoose: (contentRoot: string) => void;
  onReset: () => void;
};

export function ContentRootPanel({
  contents,
  copy,
  busy,
  failed,
  onChoose,
  onReset,
}: ContentRootPanelProps) {
  const { contentRoot, candidates } = contents;
  // 分档与选中判断都在 `contentRootChoice` 里（那里钉住了「空串 ≠ null」这个陷阱）。
  const detailKind = resolveContentRootDetailKind(contentRoot, candidates);
  const isAmbiguous = detailKind === "ambiguous";

  return (
    <section
      className={`install-config__content-root${isAmbiguous ? " is-ambiguous" : ""}`}
      aria-label={copy.contentRoot.heading}
    >
      <div className="install-config__content-root-head">
        <span className="install-config__content-root-heading">{copy.contentRoot.heading}</span>
        <span
          className={`install-config-fact install-config-fact--${isAmbiguous ? "warning" : "neutral"}`}
        >
          {copy.contentRoot.kind[contentRoot.kind]}
        </span>
        {/* 恢复自动解析。放在这里而不是候选清单里当一个选项：DTO 分不出「自动解析到 X」
            与「玩家显式选了 X」，硬塞进单选组会出现两个都该高亮的项。 */}
        {!isAmbiguous ? (
          <button
            type="button"
            className="install-config__content-root-reset"
            disabled={busy}
            onClick={onReset}
          >
            <RotateCcw size={13} aria-hidden="true" />
            {copy.contentRoot.reset}
          </button>
        ) : null}
      </div>

      <p className="install-config__content-root-detail" role={isAmbiguous ? "alert" : undefined}>
        {detailKind === "ambiguous"
          ? copy.contentRoot.ambiguousDetail(candidates.length)
          : detailKind === "fallback"
            ? copy.contentRoot.fallbackDetail
            : detailKind === "rootByChoice"
              ? // 候选恒含根目录本身，所以「另有几层」要把它减掉。
                copy.contentRoot.rootByChoiceDetail(candidates.length - 1)
              : copy.contentRoot.path(contentRoot.path ?? "")}
      </p>

      {failed ? (
        <p className="install-config__content-root-failed" role="alert">
          {copy.contentRoot.chooseFailed}
        </p>
      ) : null}

      {candidates.length > 0 ? (
        <fieldset className="install-config__candidates" disabled={busy}>
          <legend className="install-config__candidates-legend">{copy.contentRoot.chooseLabel}</legend>
          <div className="install-config__candidates-list">
            {candidates.map((candidate) => {
              const label =
                candidate === "" ? copy.contentRoot.candidateRoot : candidate;
              return (
                <label key={candidate} className="install-config__candidate">
                  <input
                    type="radio"
                    name="install-config-content-root"
                    value={candidate}
                    checked={isActiveContentRoot(contentRoot, candidate)}
                    onChange={() => onChoose(candidate)}
                  />
                  <span className="install-config__candidate-label">{label}</span>
                </label>
              );
            })}
          </div>
        </fieldset>
      ) : null}
    </section>
  );
}
