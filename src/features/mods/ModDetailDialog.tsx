import { useEffect, useMemo, useState, type ChangeEvent, type FormEvent } from "react";
import { FilePenLine, ImageIcon, Info, Save, Tag, X } from "lucide-react";
import { getModDetail } from "./modLibraryApi";
import type { ModDetail, ModLibraryItem } from "./modLibraryTypes";
import {
  getModCategories,
  listCategories,
  setModCategories,
  type CategoryItem,
  type CategoryRef,
} from "./modCategoryApi";
import { updateModMetadata } from "./modMetadataApi";
import "./ModDetailDialog.css";

type ModDetailDialogProps = {
  modId: string;
  fallbackItem?: ModLibraryItem | null;
  onClose: () => void;
  onSaved: () => Promise<void> | void;
};

type FormState = {
  displayName: string;
  author: string;
  version: string;
  description: string;
  nexusModId: string;
};

type CategoryLoadState = "idle" | "ready" | "unavailable";

const emptyForm: FormState = {
  displayName: "",
  author: "",
  version: "",
  description: "",
  nexusModId: "",
};

function versionFromLabel(versionLabel: string | undefined) {
  return versionLabel?.replace(/^v/i, "") ?? "";
}

function formFromDetail(detail: ModDetail | null, fallbackItem: ModLibraryItem | null | undefined): FormState {
  return {
    displayName: detail?.name ?? fallbackItem?.name ?? "",
    author: detail?.metadata.author ?? fallbackItem?.author ?? "",
    version: detail?.metadata.version ?? versionFromLabel(fallbackItem?.versionLabel),
    description: detail?.description ?? "",
    nexusModId: detail?.nexusModId === undefined ? "" : String(detail.nexusModId),
  };
}

function selectedIdsFromCategories(
  allCategories: CategoryItem[],
  assignedCategories: CategoryRef[],
  fallbackItem: ModLibraryItem | null | undefined,
  assignmentLoaded: boolean,
) {
  if (assignmentLoaded) {
    return new Set(assignedCategories.map((category) => category.id));
  }

  const fallbackNames = new Set((fallbackItem?.categoryLabels ?? []).map((label) => label.name));
  return new Set(allCategories.filter((category) => fallbackNames.has(category.name)).map((category) => category.id));
}

function parseNexusModId(value: string) {
  const trimmed = value.trim();
  if (!trimmed) {
    return undefined;
  }
  if (!/^\d+$/.test(trimmed)) {
    return null;
  }

  const parsed = Number.parseInt(trimmed, 10);
  return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : null;
}

export function ModDetailDialog({ modId, fallbackItem, onClose, onSaved }: ModDetailDialogProps) {
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
      setForm(formFromDetail(loadedDetail, fallbackItem));
      setCategories(loadedCategories);
      setSelectedCategoryIds(
        selectedIdsFromCategories(loadedCategories, assignedCategories, fallbackItem, assignmentLoaded),
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
  }, [fallbackItem, modId]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !saving) {
        onClose();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose, saving]);

  const previewImage = detail?.previewImage ?? fallbackItem?.previewImage ?? null;
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
      await updateModMetadata({
        modId,
        displayName: form.displayName,
        author: form.author,
        version: form.version,
        description: form.description,
        nexusModId,
      });

      if (categoryLoadState === "ready") {
        await setModCategories(modId, Array.from(selectedCategoryIds));
      }

      await onSaved();
      onClose();
    } catch {
      setMessage("保存失败，请稍后重试。");
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
      <form className="mod-detail-dialog__panel" role="dialog" aria-modal="true" onSubmit={handleSubmit}>
        <header className="mod-detail-dialog__header">
          <div className="mod-detail-dialog__title-block">
            <span className="mod-detail-dialog__icon" aria-hidden="true">
              <Info size={18} />
            </span>
            <div>
              <h2>MOD 信息设置</h2>
              <p>{fallbackItem?.name ?? modId}</p>
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
                <dd>{detail?.packageId ?? fallbackItem?.id ?? modId}</dd>
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
