import { useCallback, useEffect, useMemo, useState } from "react";
import { RotateCcw, SlidersHorizontal } from "lucide-react";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { Dialog } from "../../shared/feedback";
import { installConfigCopy, type InstallConfigCopy } from "./installConfigCopy";
import { getModPackageContents } from "./packageContentsApi";
import {
  buildPackageContentTree,
  flattenVisibleRows,
  resolveInitialExpandedPaths,
  summarizeTree,
} from "./packageContentTree";
import { PackageContentTreeView } from "./PackageContentTreeView";
import type { InstallConfigTarget } from "./InstallConfigTargetProvider";
import type { PackageContents } from "./packageContentsTypes";

/*
 * 「安装配置」的悬浮覆盖层（`#354` 切片 D4，第一片：只读的包内容树）。
 *
 * 为什么是覆盖层而不是整页路由：这是个**事务型**界面——打开、看、改、提交或放弃、关闭。
 * 覆盖层天然有这套退出语义，而整页路由没有「放弃」这个概念（离开就是离开）。它也因此
 * 不必进 `navItems`、不必占一个 `AppRouteId`、不必在引导教程里编造一条说明。
 *
 * 尺寸走 `panelClassName` 放大而不是新增一档 `kind`：交互形态仍是标准 dialog（焦点陷阱、
 * Esc、点遮罩关闭都由 `ModalSurface` 提供），变的只是它得装得下一棵树。
 */

type LoadState =
  | { status: "loading" }
  | { status: "ready"; contents: PackageContents }
  | { status: "failed" };

type InstallConfigOverlayProps = {
  target: InstallConfigTarget;
  onClose: () => void;
};

export function InstallConfigOverlay({ target, onClose }: InstallConfigOverlayProps) {
  const { locale } = useI18n();
  const copy = resolveCopy(installConfigCopy, locale);

  const [state, setState] = useState<LoadState>({ status: "loading" });
  const [expandedPaths, setExpandedPaths] = useState<ReadonlySet<string>>(() => new Set<string>());
  const [reloadToken, setReloadToken] = useState(0);

  const { modId } = target;

  useEffect(() => {
    let cancelled = false;
    setState({ status: "loading" });

    getModPackageContents({ gameId: "mhw", modId })
      .then((contents) => {
        if (cancelled) {
          return;
        }
        setState({ status: "ready", contents });
        // 展开状态跟着包走：换了 Mod 就重新算一次，别把上一个包的展开路径带过来。
        setExpandedPaths(resolveInitialExpandedPaths(buildPackageContentTree(contents.entries)));
      })
      .catch(() => {
        if (!cancelled) {
          setState({ status: "failed" });
        }
      });

    return () => {
      cancelled = true;
    };
  }, [modId, reloadToken]);

  const tree = useMemo(
    () => (state.status === "ready" ? buildPackageContentTree(state.contents.entries) : []),
    [state],
  );
  const rows = useMemo(() => flattenVisibleRows(tree, expandedPaths), [tree, expandedPaths]);
  const summary = useMemo(() => summarizeTree(tree), [tree]);

  const handleToggle = useCallback((path: string) => {
    setExpandedPaths((current) => {
      const next = new Set(current);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  }, []);

  return (
    <Dialog
      open
      panelClassName="install-config-modal"
      icon={<SlidersHorizontal size={18} />}
      title={target.modName}
      description={copy.page.description}
      onClose={onClose}
    >
      <div className="install-config">
        {state.status === "loading" ? (
          <p className="install-config__status" role="status">
            {copy.states.loading}
          </p>
        ) : state.status === "failed" ? (
          <div className="install-config__empty">
            <h3>{copy.states.failedTitle}</h3>
            <p>{copy.states.failedDetail}</p>
            <button
              type="button"
              className="install-config__retry"
              onClick={() => setReloadToken((token) => token + 1)}
            >
              <RotateCcw size={15} aria-hidden="true" />
              {copy.states.retry}
            </button>
          </div>
        ) : (
          <>
            <ContentRootPanel contents={state.contents} copy={copy} />

            <div className="install-config__summary" role="status">
              <span>
                {copy.page.summary({
                  fileCount: summary.fileCount,
                  installableCount: summary.installableCount,
                })}
              </span>
              {summary.rejectedByGameCount > 0 ? (
                <span className="install-config-fact install-config-fact--warning">
                  {copy.page.summaryRejected(summary.rejectedByGameCount)}
                </span>
              ) : null}
              {summary.excludedByPlayerCount > 0 ? (
                <span className="install-config-fact install-config-fact--accent">
                  {copy.page.summaryExcluded(summary.excludedByPlayerCount)}
                </span>
              ) : null}
            </div>

            {summary.fileCount === 0 ? (
              <p className="install-config__status">{copy.states.empty}</p>
            ) : (
              <PackageContentTreeView rows={rows} onToggle={handleToggle} copy={copy} />
            )}
          </>
        )}
      </div>
    </Dialog>
  );
}

function ContentRootPanel({
  contents,
  copy,
}: {
  contents: PackageContents;
  copy: InstallConfigCopy;
}) {
  const { contentRoot } = contents;

  return (
    <section className="install-config__content-root" aria-label={copy.contentRoot.heading}>
      <div className="install-config__content-root-head">
        <span className="install-config__content-root-heading">{copy.contentRoot.heading}</span>
        <span
          className={`install-config-fact install-config-fact--${
            contentRoot.kind === "ambiguous" ? "warning" : "neutral"
          }`}
        >
          {copy.contentRoot.kind[contentRoot.kind]}
        </span>
      </div>
      <p className="install-config__content-root-detail">
        {contentRoot.kind === "ambiguous"
          ? copy.contentRoot.ambiguousDetail(contents.candidates.length)
          : /*
             * `fallback` 的 path 是空串（内容根就是沙箱根本身），拿它去拼「从 X 开始」会得到
             * 一句没有主语的话，所以这一档走自己的说明句。空串与 null 不是一回事。
             */
            contentRoot.kind === "fallback" || !contentRoot.path
            ? copy.contentRoot.fallbackDetail
            : copy.contentRoot.path(contentRoot.path)}
      </p>
    </section>
  );
}
