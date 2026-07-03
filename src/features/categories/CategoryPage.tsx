import {
  AlertTriangle,
  ListChecks,
  Loader2,
  Palette,
  Plus,
  Search,
  SearchX,
  Tags,
  Trash2,
  X,
} from "lucide-react";
import { useEffect, useMemo, useState, type FormEvent } from "react";
import { createCategory, updateCategory, deleteCategory, type CategoryItem } from "./categoryApi";
import { CategoryColorPicker } from "./CategoryColorPicker";
import { CategoryList } from "./CategoryList";
import { CategorySortMenu, type CategorySortOption } from "./CategorySortMenu";
import {
  buildSortOrderUpdates,
  canReorderCategories,
  filterCategories,
  findDuplicateCategoryName,
  getCategoryMetrics,
  getCategoryMutationErrorMessage,
  moveCategoryByOffset,
  nextAppendSortOrder,
  pruneCategorySelection,
  reorderCategoryList,
  sortCategoriesForView,
  summarizeBatchTargets,
  toggleCategorySelection,
  type CategorySortMode,
} from "./categoryWorkflow";
import { useCategoryList } from "./useCategoryList";

const emptyCategoryItems: CategoryItem[] = [];

const SORT_MODE_OPTIONS: CategorySortOption[] = [
  { value: "custom", label: "自定义排序" },
  { value: "name", label: "按名称" },
  { value: "modCount", label: "按关联数" },
];

export function CategoryPage() {
  const { state, refresh } = useCategoryList();
  const [query, setQuery] = useState("");
  const [sortMode, setSortMode] = useState<CategorySortMode>("custom");
  const [batchMode, setBatchMode] = useState(false);
  const [selectedIds, setSelectedIds] = useState<ReadonlySet<string>>(new Set());
  const [showCreateForm, setShowCreateForm] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const [orderOverride, setOrderOverride] = useState<CategoryItem[] | null>(null);
  const [savingOrder, setSavingOrder] = useState(false);
  const [batchBusy, setBatchBusy] = useState(false);
  const [pageError, setPageError] = useState<string | null>(null);
  const [statusMessage, setStatusMessage] = useState("");

  const categories = state.status === "ready" ? state.categories : emptyCategoryItems;
  const metrics = useMemo(() => getCategoryMetrics(categories), [categories]);
  const reorderEnabled = canReorderCategories(sortMode, query, batchMode) && !savingOrder;

  // 列表数据刷新后，丢弃本地乐观顺序并收敛选择集合。
  useEffect(() => {
    setOrderOverride(null);
    if (state.status === "ready") {
      setSelectedIds((current) => pruneCategorySelection(current, state.categories));
    }
  }, [state]);

  const orderedCategories = useMemo(
    () => orderOverride ?? sortCategoriesForView(categories, sortMode),
    [orderOverride, categories, sortMode],
  );
  const visibleCategories = useMemo(
    () => filterCategories(orderedCategories, query),
    [orderedCategories, query],
  );

  const isEmpty = state.status === "ready" && categories.length === 0;
  const isSearchEmpty =
    state.status === "ready" && categories.length > 0 && visibleCategories.length === 0;
  const batchSummary = useMemo(
    () => summarizeBatchTargets(categories, selectedIds),
    [categories, selectedIds],
  );
  const metricValue = (value: number) => (state.status === "ready" ? String(value) : "-");
  const closeCreateForm = () => setShowCreateForm(false);

  useEffect(() => {
    if (!showCreateForm) {
      return;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        const target = event.target instanceof Element ? event.target : null;
        if (target?.closest(".category-color-picker") && document.querySelector(".category-color-popover")) {
          return;
        }
        setShowCreateForm(false);
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [showCreateForm]);

  const clearRowStates = () => {
    setEditingId(null);
    setDeletingId(null);
  };

  const openCreateForm = () => {
    setShowCreateForm(true);
    clearRowStates();
  };

  const toggleCreateForm = () => {
    setShowCreateForm((current) => !current);
    clearRowStates();
  };

  const toggleBatchMode = () => {
    setBatchMode((current) => {
      if (current) {
        setSelectedIds(new Set());
      }
      return !current;
    });
    setShowCreateForm(false);
    clearRowStates();
  };

  const commitOrder = (next: CategoryItem[] | null) => {
    if (!next) {
      return;
    }

    setOrderOverride(next);
    setSavingOrder(true);
    setPageError(null);
    void (async () => {
      try {
        for (const update of buildSortOrderUpdates(next)) {
          await updateCategory({ categoryId: update.categoryId, sortOrder: update.sortOrder });
        }
        setStatusMessage("分类顺序已保存。");
      } catch (err: unknown) {
        setPageError(getCategoryMutationErrorMessage(err, "保存分类顺序失败，请稍后重试。"));
      } finally {
        setSavingOrder(false);
        refresh();
      }
    })();
  };

  const runBatch = (label: string, operation: (categoryId: string) => Promise<void>) => {
    const targets = [...selectedIds];
    if (targets.length === 0 || batchBusy) {
      return;
    }

    setBatchBusy(true);
    setPageError(null);
    void (async () => {
      let failedCount = 0;
      for (const categoryId of targets) {
        try {
          await operation(categoryId);
        } catch {
          failedCount += 1;
        }
      }

      setBatchBusy(false);
      if (failedCount > 0) {
        setPageError(`${label}有 ${failedCount} 个分类处理失败，列表已刷新。`);
      } else {
        setStatusMessage(`${label}完成。`);
      }
      refresh();
    })();
  };

  const handleBatchDelete = () => {
    runBatch("批量删除", (categoryId) => deleteCategory(categoryId));
    setSelectedIds(new Set());
  };

  const handleBatchColor = (color: string) => {
    runBatch("批量改色", (categoryId) =>
      updateCategory({ categoryId, color: color === "" ? null : color }),
    );
  };

  const allVisibleSelected =
    visibleCategories.length > 0
    && visibleCategories.every((category) => selectedIds.has(category.id));

  const toggleSelectAllVisible = () => {
    setSelectedIds((current) => {
      if (allVisibleSelected) {
        const next = new Set(current);
        for (const category of visibleCategories) {
          next.delete(category.id);
        }
        return next;
      }
      return new Set([...current, ...visibleCategories.map((category) => category.id)]);
    });
  };

  return (
    <section className="category-page" aria-labelledby="category-title">
      <header className="category-page__header">
        <div className="category-page__title-block">
          <span className="category-page__eyebrow">
            <Tags size={15} strokeWidth={2.2} aria-hidden="true" />
            分类 / 标签
          </span>
          <h1 id="category-title">分类管理</h1>
          <p className="category-page__metrics">
            {state.status === "ready"
              ? `${metrics.total} 个分类 · 关联 ${metrics.linkedModCount} 个 Mod · ${metrics.emptyCategoryCount} 个空分类`
              : state.status === "loading"
                ? "正在读取分类数据…"
                : "分类数据暂时不可用"}
          </p>
          <div className="category-mode-tabs" role="tablist" aria-label="分类标签管理范围">
            <button type="button" className="is-active" role="tab" aria-selected="true" aria-current="page">
              分类
            </button>
            <button type="button" role="tab" aria-selected="false" aria-disabled="true" disabled>
              标签
            </button>
          </div>
        </div>
        <button
          type="button"
          className={`category-create-trigger ${showCreateForm ? "is-open" : ""}`}
          aria-expanded={showCreateForm}
          aria-controls="category-create-form"
          onClick={toggleCreateForm}
        >
          {showCreateForm
            ? <X size={15} strokeWidth={2.2} aria-hidden="true" />
            : <Plus size={15} strokeWidth={2.2} aria-hidden="true" />}
          {showCreateForm ? "取消新建" : "新建分类"}
        </button>
      </header>

      {showCreateForm && (
        <div className="category-create-floating">
          <svg className="category-create-distortion-filter" aria-hidden="true" focusable="false">
            <filter id="category-create-distortion" x="-20%" y="-20%" width="140%" height="140%">
              <feTurbulence
                type="fractalNoise"
                baseFrequency="0.018 0.03"
                numOctaves="2"
                seed="7"
                result="category-create-noise"
              />
              <feDisplacementMap
                in="SourceGraphic"
                in2="category-create-noise"
                scale="7"
                xChannelSelector="R"
                yChannelSelector="G"
              />
            </filter>
          </svg>
          <button
            type="button"
            className="category-create-scrim"
            aria-label="关闭新建分类"
            tabIndex={-1}
            onClick={closeCreateForm}
          />
          <section
            className="category-create-panel"
            role="dialog"
            aria-modal="true"
            aria-labelledby="category-create-title"
          >
            <div className="category-create-panel__header">
              <h2 id="category-create-title">新建分类</h2>
              <button
                type="button"
                className="category-create-panel__close"
                aria-label="关闭新建分类"
                onClick={closeCreateForm}
              >
                <X size={15} strokeWidth={2.2} aria-hidden="true" />
              </button>
            </div>
            <CreateCategoryForm
              allCategories={categories}
              onCreated={() => {
                setShowCreateForm(false);
                setStatusMessage("分类已创建。");
                refresh();
              }}
              onCancel={closeCreateForm}
            />
          </section>
        </div>
      )}

      <div className="category-main-card">
        <div className="category-summary-strip" aria-label="分类概览">
          <div className="category-summary-stat">
            <span>总分类</span>
            <strong>{metricValue(metrics.total)}</strong>
          </div>
          <div className="category-summary-stat">
            <span>关联 Mod</span>
            <strong>{metricValue(metrics.linkedModCount)}</strong>
          </div>
          <div className="category-summary-stat">
            <span>空分类</span>
            <strong>{metricValue(metrics.emptyCategoryCount)}</strong>
          </div>
          <div className="category-summary-stat">
            <span>已设置颜色</span>
            <strong>{metricValue(metrics.coloredCategoryCount)}</strong>
          </div>
        </div>

        <div className="category-toolbar">
          <div className="category-search">
            <Search size={15} strokeWidth={2.2} aria-hidden="true" />
            <input
              type="search"
              placeholder="搜索分类名称…"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              aria-label="搜索分类"
            />
          </div>
          <CategorySortMenu
            value={sortMode}
            options={SORT_MODE_OPTIONS}
            onChange={setSortMode}
          />
          <button
            type="button"
            className={`category-batch-toggle ${batchMode ? "is-active" : ""}`}
            aria-pressed={batchMode}
            onClick={toggleBatchMode}
            disabled={state.status !== "ready" || categories.length === 0}
          >
            <ListChecks size={15} strokeWidth={2.2} aria-hidden="true" />
            批量管理
          </button>
        </div>

        {batchMode && state.status === "ready" && (
          <BatchActionBar
            summary={batchSummary}
            busy={batchBusy}
            allSelected={allVisibleSelected}
            hasVisible={visibleCategories.length > 0}
            onToggleSelectAll={toggleSelectAllVisible}
            onBatchColor={handleBatchColor}
            onBatchDelete={handleBatchDelete}
            onExit={toggleBatchMode}
          />
        )}

        {pageError && (
          <p className="category-page-error" role="alert">
            <AlertTriangle size={14} strokeWidth={2.2} aria-hidden="true" />
            {pageError}
          </p>
        )}
        <p className="category-live-status category-visually-hidden" role="status" aria-live="polite">
          {statusMessage}
        </p>

        {state.status === "loading" && (
          <div className="category-state-card" role="status">
            <Loader2 size={18} className="category-spinner" aria-hidden="true" />
            <strong>正在读取分类</strong>
            <span>请稍候</span>
          </div>
        )}

        {state.status === "error" && (
          <div className="category-state-card is-error" role="alert">
            <AlertTriangle size={20} aria-hidden="true" />
            <strong>无法加载分类列表</strong>
            <span>分类数据暂时不可用。</span>
            <button type="button" className="category-action-button is-primary" onClick={refresh}>
              重试
            </button>
          </div>
        )}

        {isEmpty && (
          <div className="category-state-card is-empty" role="status">
            <Tags size={22} strokeWidth={1.8} aria-hidden="true" />
            <strong>还没有分类</strong>
            <span>新建后可在 Mod 库和详情面板中使用。</span>
            <button type="button" className="category-action-button is-primary" onClick={openCreateForm}>
              <Plus size={14} strokeWidth={2.2} aria-hidden="true" />
              新建分类
            </button>
          </div>
        )}

        {isSearchEmpty && (
          <div className="category-state-card is-empty" role="status">
            <SearchX size={22} strokeWidth={1.8} aria-hidden="true" />
            <strong>没有匹配的分类</strong>
            <span>换个关键词，或清除搜索查看全部分类。</span>
            <button type="button" className="category-action-button" onClick={() => setQuery("")}>
              清除搜索
            </button>
          </div>
        )}

        {visibleCategories.length > 0 && (
          <CategoryList
            categories={visibleCategories}
            allCategories={categories}
            reorderEnabled={reorderEnabled}
            batchMode={batchMode}
            selectedIds={selectedIds}
            editingId={editingId}
            deletingId={deletingId}
            savingOrder={savingOrder}
            onToggleSelect={(categoryId) =>
              setSelectedIds((current) => toggleCategorySelection(current, categoryId))
            }
            onStartEdit={(categoryId) => {
              setEditingId(categoryId);
              setDeletingId(null);
              setShowCreateForm(false);
            }}
            onCancelEdit={() => setEditingId(null)}
            onSaved={() => {
              setEditingId(null);
              setStatusMessage("分类已保存。");
              refresh();
            }}
            onStartDelete={(categoryId) => {
              setDeletingId(categoryId);
              setEditingId(null);
              setShowCreateForm(false);
            }}
            onCancelDelete={() => setDeletingId(null)}
            onDeleted={() => {
              setDeletingId(null);
              setStatusMessage("分类已删除。");
              refresh();
            }}
            onReorder={(fromIndex, insertIndex) =>
              commitOrder(reorderCategoryList(visibleCategories, fromIndex, insertIndex))
            }
            onMove={(categoryId, offset) =>
              commitOrder(moveCategoryByOffset(visibleCategories, categoryId, offset))
            }
          />
        )}

        {savingOrder && (
          <p className="category-live-status category-visually-hidden" role="status">
            正在保存顺序…
          </p>
        )}
      </div>
    </section>
  );
}

type CreateCategoryFormProps = {
  allCategories: CategoryItem[];
  onCreated: () => void;
  onCancel: () => void;
};

function CreateCategoryForm({ allCategories, onCreated, onCancel }: CreateCategoryFormProps) {
  const [name, setName] = useState("");
  const [color, setColor] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const duplicate = findDuplicateCategoryName(allCategories, name);
  const canSubmit = name.trim().length > 0 && !duplicate && !submitting;

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();
    if (!canSubmit) return;

    setError(null);
    setSubmitting(true);
    void (async () => {
      try {
        await createCategory({
          name: name.trim(),
          color: color.trim() || undefined,
          sortOrder: nextAppendSortOrder(allCategories),
        });
      } catch (err: unknown) {
        setError(getCategoryMutationErrorMessage(err, "创建分类失败，请稍后重试。"));
        setSubmitting(false);
        return;
      }

      onCreated();
    })();
  };

  return (
    <form id="category-create-form" className="category-create-form" onSubmit={handleSubmit} aria-label="新建分类">
      <label className="category-form-field is-grow">
        <span>名称</span>
        <input
          type="text"
          placeholder="例如：外观"
          value={name}
          onChange={(e) => setName(e.target.value)}
          autoFocus
        />
      </label>
      <div className="category-form-field">
        <span>颜色</span>
        <CategoryColorPicker value={color} onChange={setColor} triggerLabel="新分类颜色" />
      </div>
      <div className="category-form-actions">
        <button type="submit" className="category-action-button is-primary" disabled={!canSubmit}>
          {submitting ? "创建中…" : "创建"}
        </button>
        <button type="button" className="category-action-button" onClick={onCancel}>
          取消
        </button>
      </div>
      {duplicate && (
        <p className="category-inline-error" role="alert">
          已存在同名分类「{duplicate.name}」，请换一个名称。
        </p>
      )}
      {error && (
        <p className="category-inline-error" role="alert">
          {error}
        </p>
      )}
    </form>
  );
}

type BatchActionBarProps = {
  summary: { count: number; linkedModCount: number };
  busy: boolean;
  allSelected: boolean;
  hasVisible: boolean;
  onToggleSelectAll: () => void;
  onBatchColor: (color: string) => void;
  onBatchDelete: () => void;
  onExit: () => void;
};

function BatchActionBar({
  summary,
  busy,
  allSelected,
  hasVisible,
  onToggleSelectAll,
  onBatchColor,
  onBatchDelete,
  onExit,
}: BatchActionBarProps) {
  const [color, setColor] = useState("");
  const [confirmingDelete, setConfirmingDelete] = useState(false);

  const hasSelection = summary.count > 0;

  return (
    <div className="category-batch-bar" role="group" aria-label="批量操作">
      <label className="category-batch-bar__select-all">
        <input
          type="checkbox"
          checked={allSelected}
          onChange={onToggleSelectAll}
          disabled={!hasVisible || busy}
          aria-label="全选当前列表"
        />
        全选
      </label>
      <span className="category-batch-bar__summary" aria-live="polite">
        {hasSelection
          ? `已选 ${summary.count} 个分类 · 关联 ${summary.linkedModCount} 个 Mod`
          : "勾选分类后可批量操作"}
      </span>
      <div className="category-batch-bar__actions">
        <div className="category-batch-bar__color">
          <CategoryColorPicker value={color} onChange={setColor} triggerLabel="批量目标颜色" />
          <button
            type="button"
            className="category-action-button"
            disabled={!hasSelection || busy}
            onClick={() => onBatchColor(color.trim())}
          >
            <Palette size={14} strokeWidth={2.2} aria-hidden="true" />
            应用颜色
          </button>
        </div>
        <button
          type="button"
          className="category-action-button is-danger"
          disabled={!hasSelection || busy}
          onClick={() => setConfirmingDelete(true)}
        >
          <Trash2 size={14} strokeWidth={2.2} aria-hidden="true" />
          批量删除
        </button>
        <button type="button" className="category-action-button" onClick={onExit} disabled={busy}>
          退出批量
        </button>
      </div>

      {confirmingDelete && hasSelection && (
        <div className={`category-delete-confirm ${summary.linkedModCount > 0 ? "is-danger" : ""}`}>
          <p>
            {summary.linkedModCount > 0
              ? `确定删除已选的 ${summary.count} 个分类？共 ${summary.linkedModCount} 个 Mod 关联将被移除，Mod 本体不受影响。`
              : `确定删除已选的 ${summary.count} 个分类？`}
          </p>
          <div className="category-delete-confirm__actions">
            <button
              type="button"
              className="category-action-button is-danger"
              disabled={busy}
              onClick={() => {
                setConfirmingDelete(false);
                onBatchDelete();
              }}
            >
              {busy ? "删除中…" : "删除"}
            </button>
            <button
              type="button"
              className="category-action-button"
              onClick={() => setConfirmingDelete(false)}
            >
              取消
            </button>
          </div>
        </div>
      )}
      {busy && <p className="category-batch-bar__busy">正在处理批量操作…</p>}
    </div>
  );
}
