import { useEffect, useMemo, useRef, useState, type ChangeEvent, type FormEvent } from "react";
import { FilePenLine, ImageIcon, Info, Save, Tag, X } from "lucide-react";
import { getModDetail } from "./modLibraryApi";
import type { ModDetail, ModLibraryItem } from "./modLibraryTypes";
import { getTrappedFocusIndex } from "./modalFocusTrap";
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

type ModDetailDialogProps = {
  modId: string;
  fallbackItem?: ModLibraryItem | null;
  onClose: () => void;
  onSaved: () => Promise<void> | void;
};

type CategoryLoadState = "idle" | "ready" | "unavailable";

const focusableSelector = [
  "a[href]",
  "button:not([disabled])",
  "input:not([disabled])",
  "textarea:not([disabled])",
  "select:not([disabled])",
  "[tabindex]:not([tabindex='-1'])",
].join(",");

function getFocusableElements(container: HTMLElement) {
  return Array.from(container.querySelectorAll<HTMLElement>(focusableSelector)).filter(
    (element) => element.tabIndex >= 0 && !element.hasAttribute("aria-hidden"),
  );
}

export function ModDetailDialog({ modId, fallbackItem, onClose, onSaved }: ModDetailDialogProps) {
  const fallbackSnapshotRef = useRef<{ modId: string; item: ModLibraryItem | null | undefined }>({
    modId,
    item: fallbackItem,
  });
  if (fallbackSnapshotRef.current.modId !== modId) {
    fallbackSnapshotRef.current = { modId, item: fallbackItem };
  }

  const fallbackSnapshotItem = fallbackSnapshotRef.current.item;
  const panelRef = useRef<HTMLFormElement | null>(null);
  const restoreFocusRef = useRef<HTMLElement | null>(null);
  const [detail, setDetail] = useState<ModDetail | null>(null);
  const [form, setForm] = useState<FormState>(emptyForm);
  const [categories, setCategories] = useState<CategoryItem[]>([]);
  const [selectedCategoryIds, setSelectedCategoryIds] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [categoryLoadState, setCategoryLoadState] = useState<CategoryLoadState>("idle");
  const [message, setMessage] = useState<string | null>(null);

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

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !saving) {
        onClose();
        return;
      }

      if (event.key !== "Tab") {
        return;
      }

      const panel = panelRef.current;
      if (!panel) {
        return;
      }

      const focusableElements = getFocusableElements(panel);
      const activeElement = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      const currentIndex = activeElement ? focusableElements.indexOf(activeElement) : -1;
      const nextIndex = getTrappedFocusIndex({
        currentIndex,
        focusableCount: focusableElements.length,
        backwards: event.shiftKey,
      });

      if (nextIndex !== null) {
        event.preventDefault();
        const focusTarget = nextIndex === -1 ? panel : focusableElements[nextIndex];
        focusTarget.focus();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose, saving]);

  useEffect(() => {
    if (typeof document === "undefined") {
      return undefined;
    }

    restoreFocusRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const frameId = window.requestAnimationFrame(() => {
      const panel = panelRef.current;
      if (!panel) {
        return;
      }

      const firstFocusable = getFocusableElements(panel)[0] ?? panel;
      firstFocusable.focus();
    });

    return () => {
      window.cancelAnimationFrame(frameId);
      restoreFocusRef.current?.focus();
      restoreFocusRef.current = null;
    };
  }, [modId]);

  const previewImage = detail?.previewImage ?? fallbackSnapshotItem?.previewImage ?? null;
  const previewThumbnail = previewImage?.kind === "thumbnail" ? previewImage : null;
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
          onClose();
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

  return (
    <div
      className="mod-detail-dialog__backdrop"
      role="presentation"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !saving) {
          onClose();
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
              <Info size={18} />
            </span>
            <div>
              <h2>MOD 信息设置</h2>
              <p>{fallbackSnapshotItem?.name ?? modId}</p>
            </div>
          </div>
          <button className="mod-detail-dialog__close" type="button" onClick={onClose} disabled={saving} aria-label="关闭">
            <X size={18} />
          </button>
        </header>

        <div className="mod-detail-dialog__body">
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
          </main>
        </div>

        {message ? <div className="mod-detail-dialog__message" role="status">{message}</div> : null}

        <footer className="mod-detail-dialog__footer">
          <button className="mod-detail-dialog__button is-secondary" type="button" onClick={onClose} disabled={saving}>
            取消
          </button>
          <button className="mod-detail-dialog__button is-primary" type="submit" disabled={loading || saving}>
            <Save size={16} aria-hidden="true" />
            {saving ? "保存中" : "保存"}
          </button>
        </footer>
      </form>
    </div>
  );
}
