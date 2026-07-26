import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ChangeEvent,
  type FormEvent,
} from "react";
import { FilePenLine, ImageIcon, Info, Save, Tag, Target, X } from "lucide-react";
import { createPortal } from "react-dom";
import { useModalFocusTrap } from "../../shared/feedback/useModalFocusTrap";
import type { GameId } from "../game-setup/gameSetupTypes";
import { ReplacementTargetPanel } from "../replacements/ReplacementTargetPanel";
import { getModDetail } from "./modLibraryApi";
import type { ModDetail, ModLibraryItem } from "./modLibraryTypes";
import type { InstallManifestStatus } from "./modInstallPlanTypes";
import {
  getModCategories,
  listCategories,
  setModCategories,
  type CategoryItem,
} from "./modCategoryApi";
import { updateModMetadata } from "./modMetadataApi";
import {
  emptyForm,
  formFromDetail,
  parseNexusModId,
  saveModDetailChanges,
  selectedIdsFromCategories,
  type FormState,
} from "./modDetailDialogWorkflow";
import "./ModDetailDialog.css";

export type ModDetailDialogTab = "details" | "replacement";

type ModDetailDialogProps = {
  modId: string;
  fallbackItem?: ModLibraryItem | null;
  initialTab: ModDetailDialogTab;
  gameId: GameId;
  profileId: string | null;
  installStatus: InstallManifestStatus | undefined;
  onClose: () => void;
  onSaved: () => Promise<void> | void;
};

type CategoryLoadState = "idle" | "ready" | "unavailable";

/*
 * 退场动画时长。纯 CSS 无法为卸载中的节点播放动画（父级把状态置空后节点已不存在），
 * 因此先标记退场、等动画播完再调用真正的 onClose。该值必须与 ModDetailDialog.css 中
 * .mod-detail-dialog__backdrop.is-exiting 的动画时长保持一致。
 */
const DIALOG_EXIT_DURATION_MS = 160;

export function ModDetailDialog({
  modId,
  fallbackItem,
  initialTab,
  gameId,
  profileId,
  installStatus,
  onClose,
  onSaved,
}: ModDetailDialogProps) {
  const fallbackSnapshotRef = useRef<{ modId: string; item: ModLibraryItem | null | undefined }>({
    modId,
    item: fallbackItem,
  });
  if (fallbackSnapshotRef.current.modId !== modId) {
    fallbackSnapshotRef.current = { modId, item: fallbackItem };
  }

  const fallbackSnapshotItem = fallbackSnapshotRef.current.item;
  const panelRef = useRef<HTMLFormElement | null>(null);
  const [detail, setDetail] = useState<ModDetail | null>(null);
  const [form, setForm] = useState<FormState>(emptyForm);
  const [categories, setCategories] = useState<CategoryItem[]>([]);
  const [selectedCategoryIds, setSelectedCategoryIds] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [categoryLoadState, setCategoryLoadState] = useState<CategoryLoadState>("idle");
  const [message, setMessage] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<ModDetailDialogTab>(initialTab);
  const [replacementBusy, setReplacementBusy] = useState(false);
  const [replacementCompletedLocally, setReplacementCompletedLocally] = useState(false);
  const [replacementInstallStatus, setReplacementInstallStatus] =
    useState<InstallManifestStatus | undefined>(installStatus);
  const dialogBusy = saving || replacementBusy;
  const [exiting, setExiting] = useState(false);
  const exitTimerRef = useRef<number | null>(null);

  const clearExitTimer = useCallback(() => {
    if (exitTimerRef.current !== null) {
      window.clearTimeout(exitTimerRef.current);
      exitTimerRef.current = null;
    }
  }, []);

  /*
   * 所有关闭入口（ESC、点遮罩、关闭按钮、取消、保存成功）统一走这里：
   * 先播退场动画，播完再通知父级卸载。重入保护避免连点或多入口同时触发时重复计时。
   */
  const requestClose = useCallback(() => {
    if (exitTimerRef.current !== null) {
      return;
    }
    setExiting(true);
    exitTimerRef.current = window.setTimeout(() => {
      exitTimerRef.current = null;
      onClose();
    }, DIALOG_EXIT_DURATION_MS);
  }, [onClose]);

  useEffect(() => clearExitTimer, [clearExitTimer]);

  /*
   * 切换到另一个 Mod 时组件不卸载、只换 props，因此必须取消进行中的退场，
   * 否则新打开的对话框会带着上一个的退场状态、并在动画结束后被误关闭。
   */
  useEffect(() => {
    setExiting(false);
    clearExitTimer();
  }, [clearExitTimer, modId]);

  useModalFocusTrap({
    active: true,
    containerRef: panelRef,
    closeOnEscape: !dialogBusy && !exiting,
    onRequestClose: requestClose,
    focusKey: modId,
  });

  useEffect(() => {
    setActiveTab(initialTab);
    setReplacementBusy(false);
    setReplacementCompletedLocally(false);
  }, [initialTab, modId]);

  useEffect(() => {
    setReplacementInstallStatus(installStatus);
  }, [installStatus, modId]);

  const handleReplacementInstallCompleted = useCallback(async () => {
    setReplacementCompletedLocally(true);
    await onSaved();
    setReplacementInstallStatus("installed");
    setReplacementCompletedLocally(false);
  }, [onSaved]);

  useEffect(() => {
    let cancelled = false;

    setLoading(true);
    setMessage(null);
    setCategoryLoadState("idle");

    async function loadDialogData() {
      const [detailResult, categoryResult, assignedResult] = await Promise.allSettled([
        getModDetail({ modId }),
        listCategories(),
        getModCategories(modId),
      ]);

      if (cancelled) {
        return;
      }

      const loadedDetail = detailResult.status === "fulfilled" ? detailResult.value : null;
      const loadedCategories = categoryResult.status === "fulfilled" ? categoryResult.value : [];
      const assignedCategories = assignedResult.status === "fulfilled" ? assignedResult.value : [];
      const assignmentLoaded = assignedResult.status === "fulfilled";
      const categoriesReady = categoryResult.status === "fulfilled" && assignmentLoaded;

      setDetail(loadedDetail);
      setForm(formFromDetail(loadedDetail, fallbackSnapshotItem));
      setCategories(loadedCategories);
      setSelectedCategoryIds(
        selectedIdsFromCategories(loadedCategories, assignedCategories, fallbackSnapshotItem, assignmentLoaded),
      );
      setCategoryLoadState(categoriesReady ? "ready" : "unavailable");
      setLoading(false);

      if (detailResult.status === "rejected") {
        setMessage("详情读取失败，已使用列表中的基础信息。");
      } else if (!categoriesReady) {
        setMessage("分类读取失败，本次保存不会改动分类关联。");
      }
    }

    void loadDialogData();

    return () => {
      cancelled = true;
    };
  }, [fallbackSnapshotItem, modId]);

  const previewImage = detail?.previewImage ?? fallbackSnapshotItem?.previewImage ?? null;
  const previewThumbnail = previewImage?.kind === "thumbnail" ? previewImage : null;
  /*
   * 标题优先用已加载的权威 detail.name。fallbackSnapshotItem 只是打开对话框时的列表快照，
   * 可能已过期或缺失；modId 是最后兜底。名称提升为主标题后，这个优先级尤其重要——
   * 主标签显示过期名称会直接误导用户在编辑哪个 Mod。
   * 优先级与上方 previewImage 保持一致。
   */
  const displayModName = detail?.name ?? fallbackSnapshotItem?.name ?? modId;
  const selectedCategoryNames = useMemo(() => {
    const selected = new Set(selectedCategoryIds);
    return categories.filter((category) => selected.has(category.id)).map((category) => category.name);
  }, [categories, selectedCategoryIds]);

  const updateField = (event: ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => {
    const { name, value } = event.target;
    setForm((current) => ({ ...current, [name]: value }));
  };

  const toggleCategory = (categoryId: string) => {
    setSelectedCategoryIds((current) => {
      const next = new Set(current);
      if (next.has(categoryId)) {
        next.delete(categoryId);
      } else {
        next.add(categoryId);
      }
      return next;
    });
  };

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();

    if (activeTab !== "details") {
      return;
    }

    const nexusModId = parseNexusModId(form.nexusModId);
    if (nexusModId === null) {
      setMessage("NexusMods ID 只能填写正整数。");
      return;
    }

    setSaving(true);
    setMessage(null);

    try {
      const saveResult = await saveModDetailChanges({
        modId,
        metadata: {
          displayName: form.displayName,
          author: form.author,
          version: form.version,
          description: form.description,
          nexusModId,
        },
        categoryIds: Array.from(selectedCategoryIds),
        categoriesReady: categoryLoadState === "ready",
        updateModMetadata,
        setModCategories,
        onSaved,
      });

      switch (saveResult.status) {
        case "saved":
          requestClose();
          break;
        case "metadata-failure":
          setMessage("信息保存失败，请稍后重试。");
          break;
        case "partial-category-failure":
          setMessage("信息已保存，但分类关联保存失败，请稍后重试。");
          break;
        case "refresh-failure":
          setMessage("保存成功，但列表刷新失败，请稍后手动刷新。");
          break;
      }
    } finally {
      setSaving(false);
    }
  };

  return createPortal(
    <div
      className={`mod-detail-dialog__backdrop${exiting ? " is-exiting" : ""}`}
      role="presentation"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !dialogBusy && !exiting) {
          requestClose();
        }
      }}
    >
      <form
        ref={panelRef}
        className="mod-detail-dialog__panel"
        role="dialog"
        aria-modal="true"
        tabIndex={-1}
        onSubmit={handleSubmit}
      >
        <header className="mod-detail-dialog__header">
          <div className="mod-detail-dialog__title-block">
            <span className="mod-detail-dialog__icon" aria-hidden="true">
              {activeTab === "replacement" ? <Target size={18} /> : <Info size={18} />}
            </span>
            {/*
             * 层级以用户关心的信息为主：Mod 名称作为标题，"Mod 详情" 降为上方小字说明。
             * 原实现把通用词当标题、把 Mod 名称当副标题，层级是反的。
             */}
            <div className="mod-detail-dialog__heading">
              <span className="mod-detail-dialog__eyebrow">Mod 详情</span>
              <h2 title={displayModName}>{displayModName}</h2>
            </div>
          </div>
          <button className="mod-detail-dialog__close" type="button" onClick={requestClose} disabled={dialogBusy || exiting} aria-label="关闭">
            <X size={18} />
          </button>
        </header>

        <div className="mod-detail-dialog__tabs" role="tablist" aria-label="Mod 详情视图">
          <button
            type="button"
            role="tab"
            aria-selected={activeTab === "details"}
            className={activeTab === "details" ? "is-active" : undefined}
            onClick={() => setActiveTab("details")}
            disabled={dialogBusy}
          >
            <Info size={15} aria-hidden="true" />
            基本信息
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={activeTab === "replacement"}
            className={activeTab === "replacement" ? "is-active" : undefined}
            onClick={() => setActiveTab("replacement")}
            disabled={dialogBusy}
          >
            <Target size={15} aria-hidden="true" />
            替换目标
          </button>
        </div>

        <div
          className={`mod-detail-dialog__body${activeTab === "replacement" ? " is-replacement" : ""}`}
        >
          <aside className="mod-detail-dialog__preview" aria-label="Mod 预览图">
            {previewThumbnail ? (
              <img src={previewThumbnail.thumbnailUrl} alt="" />
            ) : (
              <div className="mod-detail-dialog__preview-fallback">
                <ImageIcon size={34} />
                <span>暂无预览图</span>
              </div>
            )}
            <dl className="mod-detail-dialog__summary">
              <div>
                <dt>Package ID</dt>
                <dd>{detail?.packageId ?? fallbackSnapshotItem?.id ?? modId}</dd>
              </div>
              <div>
                <dt>已选分类</dt>
                <dd>{selectedCategoryNames.length > 0 ? selectedCategoryNames.join(" / ") : "未关联"}</dd>
              </div>
            </dl>
          </aside>

          <main className="mod-detail-dialog__content">
            {activeTab === "details" ? (
              <>
            <section className="mod-detail-dialog__section">
              <div className="mod-detail-dialog__section-title">
                <FilePenLine size={16} aria-hidden="true" />
                <span>信息编辑</span>
              </div>
              <div className="mod-detail-dialog__form-grid" aria-busy={loading}>
                <label>
                  <span>名称</span>
                  <input name="displayName" value={form.displayName} onChange={updateField} disabled={loading || saving} />
                </label>
                <label>
                  <span>作者</span>
                  <input name="author" value={form.author} onChange={updateField} disabled={loading || saving} />
                </label>
                <label>
                  <span>版本</span>
                  <input name="version" value={form.version} onChange={updateField} disabled={loading || saving} />
                </label>
                <label>
                  <span>NexusMods ID</span>
                  <input
                    name="nexusModId"
                    inputMode="numeric"
                    value={form.nexusModId}
                    onChange={updateField}
                    disabled={loading || saving}
                  />
                </label>
                <label className="mod-detail-dialog__wide-field">
                  <span>备注</span>
                  <textarea
                    name="description"
                    value={form.description}
                    onChange={updateField}
                    disabled={loading || saving}
                    rows={5}
                  />
                </label>
              </div>
            </section>

            <section className="mod-detail-dialog__section">
              <div className="mod-detail-dialog__section-title">
                <Tag size={16} aria-hidden="true" />
                <span>分类关联</span>
              </div>
              {categories.length > 0 ? (
                <div className="mod-detail-dialog__category-grid" aria-disabled={categoryLoadState !== "ready"}>
                  {categories.map((category) => {
                    const checked = selectedCategoryIds.has(category.id);
                    return (
                      <label className="mod-detail-dialog__category-chip" key={category.id}>
                        <input
                          type="checkbox"
                          checked={checked}
                          onChange={() => toggleCategory(category.id)}
                          disabled={loading || saving || categoryLoadState !== "ready"}
                        />
                        <span
                          className="mod-detail-dialog__category-dot"
                          style={{ backgroundColor: category.color ?? "var(--color-border)" }}
                          aria-hidden="true"
                        />
                        <span>{category.name}</span>
                      </label>
                    );
                  })}
                </div>
              ) : (
                <p className="mod-detail-dialog__empty">还没有可关联的分类。</p>
              )}
            </section>
              </>
            ) : (
              <section className="mod-detail-dialog__section is-replacement">
                <ReplacementTargetPanel
                  gameId={gameId}
                  modId={modId}
                  profileId={profileId}
                  installStatus={replacementInstallStatus}
                  completedLocally={replacementCompletedLocally}
                  onBusyChange={setReplacementBusy}
                  onInstallCompleted={handleReplacementInstallCompleted}
                />
              </section>
            )}
          </main>
        </div>

        {activeTab === "details" && message ? <div className="mod-detail-dialog__message" role="status">{message}</div> : null}

        <footer className="mod-detail-dialog__footer">
          {activeTab === "details" ? (
            <>
              <button className="mod-detail-dialog__button is-secondary" type="button" onClick={requestClose} disabled={dialogBusy || exiting}>
                取消
              </button>
              <button className="mod-detail-dialog__button is-primary" type="submit" disabled={loading || dialogBusy}>
                <Save size={16} aria-hidden="true" />
                {saving ? "保存中" : "保存"}
              </button>
            </>
          ) : (
            <button className="mod-detail-dialog__button is-secondary" type="button" onClick={requestClose} disabled={dialogBusy || exiting}>
              关闭
            </button>
          )}
        </footer>
      </form>
    </div>,
    document.body,
  );
}
