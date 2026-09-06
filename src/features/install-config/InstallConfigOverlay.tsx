import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { RotateCcw, SlidersHorizontal } from "lucide-react";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { Dialog } from "../../shared/feedback";
import { installConfigCopy, type InstallConfigCopy } from "./installConfigCopy";
import { getModPackageContents } from "./packageContentsApi";
import {
  buildPackageContentTree,
  flattenVisibleRows,
  indexNodesByPath,
  resolveInitialExpandedPaths,
  summarizeTree,
} from "./packageContentTree";
import { PackageContentTreeView } from "./PackageContentTreeView";
import { computeDirectorySelection, isSameSelection, toggleSelection } from "./packageContentSelection";
import { clearModPackageFileSelection, setModPackageFileSelection } from "./packageContentsApi";
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
  /*
   * 勾选是**草稿**：先在内存里改，点「保存」才落到后端。
   *
   * 事务型语义要求有「放弃」这一步——即时保存的话，玩家勾错一个文件就再也回不到原状了
   * （他不记得原来勾了哪些）。草稿不落盘，关掉面板就没了。
   */
  const [draftExcluded, setDraftExcluded] = useState<ReadonlySet<string>>(() => new Set<string>());
  const [saving, setSaving] = useState(false);
  const [saveFailed, setSaveFailed] = useState(false);
  const [confirmingClose, setConfirmingClose] = useState(false);

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
        // 草稿以后端记录为起点，否则重新打开会显示成「什么都没勾掉」。
        setDraftExcluded(new Set(contents.excludedFiles));
        setSaveFailed(false);
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

  const nodesByPath = useMemo(() => indexNodesByPath(tree), [tree]);
  const selectionStates = useMemo(
    () => computeDirectorySelection(tree, draftExcluded),
    [tree, draftExcluded],
  );

  const savedExcluded = state.status === "ready" ? state.contents.excludedFiles : [];
  const isDirty = !isSameSelection(draftExcluded, savedExcluded);

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

  const handleToggleSelection = useCallback(
    (path: string) => {
      const node = nodesByPath.get(path);
      if (!node) {
        return;
      }
      setSaveFailed(false);
      setDraftExcluded((current) => toggleSelection(node, current));
    },
    [nodesByPath],
  );

  /** 返回是否保存成功——「保存并关闭」要据此决定关不关，失败了关掉就把改动丢了。 */
  const handleSave = async (): Promise<boolean> => {
    setSaving(true);
    setSaveFailed(false);
    try {
      /*
       * 空排除集合走 `clear` 而不是提交一个空数组：清掉记录与「记录着一个空集合」在仓储
       * 层不是一回事，前者让这个包回到从没被干预过的状态。
       */
      const excludedFiles = [...draftExcluded];
      const contents =
        excludedFiles.length === 0
          ? await clearModPackageFileSelection({ gameId: "mhw", modId })
          : await setModPackageFileSelection({ gameId: "mhw", modId, excludedFiles });

      // 两条命令都**回读**并返回生效之后的结果，直接用它，不自己推演。
      setState({ status: "ready", contents });
      setDraftExcluded(new Set(contents.excludedFiles));
      return true;
    } catch {
      setSaveFailed(true);
      return false;
    } finally {
      setSaving(false);
    }
  };

  const handleSaveAndClose = async () => {
    if (await handleSave()) {
      onClose();
    }
    // 失败就留在面板里：确认条会换成失败文案，改动还在草稿里没丢。
  };

  const handleDiscard = () => {
    setDraftExcluded(new Set(savedExcluded));
    setSaveFailed(false);
    setConfirmingClose(false);
  };

  /*
   * 有未保存的改动时不直接关，先在页脚问一句。
   *
   * 用页脚而不是再套一层模态：嵌套模态会把焦点管理和 Esc 的语义搅成一团（Esc 该关哪个？），
   * 而这里要问的只是一个是非题。
   */
  const handleRequestClose = () => {
    if (isDirty) {
      setConfirmingClose(true);
      return;
    }
    onClose();
  };

  return (
    <Dialog
      open
      panelClassName="install-config-modal"
      icon={<SlidersHorizontal size={18} />}
      title={target.modName}
      description={copy.page.description}
      onClose={handleRequestClose}
      // 保存写盘期间不许关闭：关掉不会取消已经发出的写入，只会让玩家看不到结果。
      busy={saving}
      footer={
        state.status === "ready" ? (
          <SelectionActions
            copy={copy}
            isDirty={isDirty}
            saving={saving}
            saveFailed={saveFailed}
            onSave={handleSave}
            onDiscard={handleDiscard}
          />
        ) : undefined
      }
    >
      <div className="install-config">
        {confirmingClose ? (
          <CloseConfirmBanner
            copy={copy}
            saving={saving}
            saveFailed={saveFailed}
            onKeepEditing={() => setConfirmingClose(false)}
            onDiscardAndClose={onClose}
            onSaveAndClose={handleSaveAndClose}
          />
        ) : null}
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
              {/* 勾掉的计数跟着**草稿**走，不用后端返回的 excludedByPlayer——
                  否则勾了一下摘要不动，玩家会以为没生效。 */}
              {draftExcluded.size > 0 ? (
                <span className="install-config-fact install-config-fact--accent">
                  {copy.page.summaryExcluded(draftExcluded.size)}
                </span>
              ) : null}
            </div>

            {summary.fileCount === 0 ? (
              <p className="install-config__status">{copy.states.empty}</p>
            ) : (
              <PackageContentTreeView
                rows={rows}
                onToggle={handleToggle}
                selectionStates={selectionStates}
                excludedFiles={draftExcluded}
                onToggleSelection={handleToggleSelection}
                copy={copy}
              />
            )}
          </>
        )}
      </div>
    </Dialog>
  );
}

/**
 * 页脚：保存 / 放弃，以及带未保存改动关闭时的那一句确认。
 *
 * 两种状态互斥——确认关闭时不再显示保存按钮，免得玩家在「要不要关」和「要不要存」之间
 * 反复横跳。
 */
/**
 * 带未保存改动点关闭时的确认。
 *
 * 摆在 header 正下方——也就是关闭按钮的正下方。第一版放在页脚，实测**等于没反应**：
 * 面板一千多像素高，玩家点完右上角的 X 视线就停在那儿，根本不会往下扫到页脚，
 * 页脚静默换几个字他看不见。确认要出现在触发它的控件旁边。
 */
function CloseConfirmBanner({
  copy,
  saving,
  saveFailed,
  onKeepEditing,
  onDiscardAndClose,
  onSaveAndClose,
}: {
  copy: InstallConfigCopy;
  saving: boolean;
  saveFailed: boolean;
  onKeepEditing: () => void;
  onDiscardAndClose: () => void;
  onSaveAndClose: () => void;
}) {
  const saveAndCloseRef = useRef<HTMLButtonElement | null>(null);

  /*
   * 焦点跟过来：读屏要播报这条 alert，键盘用户也不该再去找按钮在哪。
   *
   * 落在「保存并关闭」——玩家点 X 就是想走，这是最可能的意图，而且它不破坏任何东西
   * （存错了还能再改）。「放弃并关闭」才是不可逆的那个，不该是顺手一敲回车的目标。
   */
  useEffect(() => {
    saveAndCloseRef.current?.focus();
  }, []);

  return (
    <div
      className={`install-config__close-confirm${saveFailed ? " is-failed" : ""}`}
      role="alert"
    >
      <span className="install-config__close-confirm-text">
        {/* 失败也报在这里而不是页脚：玩家的视线在这一条上，报去页脚等于不报。 */}
        {saveFailed ? copy.actions.saveFailed : copy.actions.confirmCloseDetail}
      </span>
      <button
        type="button"
        className="install-config__button"
        disabled={saving}
        onClick={onKeepEditing}
      >
        {copy.actions.keepEditing}
      </button>
      <button
        type="button"
        className="install-config__button is-danger"
        disabled={saving}
        onClick={onDiscardAndClose}
      >
        {copy.actions.discardAndClose}
      </button>
      <button
        ref={saveAndCloseRef}
        type="button"
        className="install-config__button is-primary"
        disabled={saving}
        onClick={onSaveAndClose}
      >
        {saving ? copy.actions.saving : copy.actions.saveAndClose}
      </button>
    </div>
  );
}

function SelectionActions({
  copy,
  isDirty,
  saving,
  saveFailed,
  onSave,
  onDiscard,
}: {
  copy: InstallConfigCopy;
  isDirty: boolean;
  saving: boolean;
  saveFailed: boolean;
  onSave: () => void;
  onDiscard: () => void;
}) {
  return (
    <div className="install-config__actions">
      <span className="install-config__actions-note" role={saveFailed ? "alert" : undefined}>
        {saveFailed
          ? copy.actions.saveFailed
          : isDirty
            ? copy.actions.unsaved
            : copy.actions.saved}
      </span>
      <button
        type="button"
        className="install-config__button"
        disabled={!isDirty || saving}
        onClick={onDiscard}
      >
        {copy.actions.discard}
      </button>
      <button
        type="button"
        className="install-config__button is-primary"
        disabled={!isDirty || saving}
        onClick={onSave}
      >
        {saving ? copy.actions.saving : copy.actions.save}
      </button>
    </div>
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
