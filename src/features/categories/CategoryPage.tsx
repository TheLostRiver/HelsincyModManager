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
import { resolveCopy, useI18n } from "../../shared/i18n";
import { createCategory, updateCategory, deleteCategory, type CategoryItem } from "./categoryApi";
import { categoryCopy, type CategoryCopy } from "./categoryCopy";
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

export function CategoryPage() {
  const { locale } = useI18n();
  const copy = resolveCopy(categoryCopy, locale);
  const sortModeOptions: CategorySortOption[] = [
    { value: "custom", label: copy.sort.modes.custom },
    { value: "name", label: copy.sort.modes.name },
    { value: "modCount", label: copy.sort.modes.modCount },
  ];
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
        setStatusMessage(copy.page.orderSaved);
      } catch (err: unknown) {
        setPageError(getCategoryMutationErrorMessage(err, copy.page.orderSaveFailed));
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
        setPageError(copy.batch.batchFailed(label, failedCount));
      } else {
        setStatusMessage(copy.batch.batchCompleted(label));
      }
      refresh();
    })();
  };

  const handleBatchDelete = () => {
    runBatch(copy.batch.batchDeleteLabel, (categoryId) => deleteCategory(categoryId));
    setSelectedIds(new Set());
  };

  const handleBatchColor = (color: string) => {
    runBatch(copy.batch.batchColorLabel, (categoryId) =>
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
      <header className="category-page__header" data-tour-id="categories.create">
        <div className="category-page__title-block">
          <span className="category-page__eyebrow">
            <Tags size={15} strokeWidth={2.2} aria-hidden="true" />
            {copy.page.eyebrow}
          </span>
          <h1 id="category-title">{copy.page.title}</h1>
          <p className="category-page__metrics">
            {state.status === "ready"
              ? copy.page.metricsReady(metrics.total, metrics.linkedModCount, metrics.emptyCategoryCount)
              : state.status === "loading"
                ? copy.page.metricsLoading
                : copy.page.metricsUnavailable}
          </p>
          <div className="category-mode-tabs" role="tablist" aria-label={copy.page.modeTabsAria}>
            <button type="button" className="is-active" role="tab" aria-selected="true" aria-current="page">
              {copy.page.tabCategories}
            </button>
            <button type="button" role="tab" aria-selected="false" aria-disabled="true" disabled>
              {copy.page.tabTags}
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
          {showCreateForm ? copy.page.createCancel : copy.page.createOpen}
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
            aria-label={copy.page.createCloseAria}
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
              <h2 id="category-create-title">{copy.page.createDialogTitle}</h2>
              <button
                type="button"
                className="category-create-panel__close"
                aria-label={copy.page.createCloseAria}
                onClick={closeCreateForm}
              >
                <X size={15} strokeWidth={2.2} aria-hidden="true" />
              </button>
            </div>
            <CreateCategoryForm
              copy={copy}
              allCategories={categories}
              onCreated={() => {
                setShowCreateForm(false);
                setStatusMessage(copy.page.created);
                refresh();
              }}
              onCancel={closeCreateForm}
            />
          </section>
        </div>
      )}

      <div className="category-main-card" data-tour-id="categories.manage">
        <div className="category-summary-strip" aria-label={copy.page.summaryAria}>
          <div className="category-summary-stat">
            <span>{copy.page.summaryTotal}</span>
            <strong>{metricValue(metrics.total)}</strong>
          </div>
          <div className="category-summary-stat">
            <span>{copy.page.summaryLinked}</span>
            <strong>{metricValue(metrics.linkedModCount)}</strong>
          </div>
          <div className="category-summary-stat">
            <span>{copy.page.summaryEmpty}</span>
            <strong>{metricValue(metrics.emptyCategoryCount)}</strong>
          </div>
          <div className="category-summary-stat">
            <span>{copy.page.summaryColored}</span>
            <strong>{metricValue(metrics.coloredCategoryCount)}</strong>
          </div>
        </div>

        <div className="category-toolbar">
          <div className="category-search">
            <Search size={15} strokeWidth={2.2} aria-hidden="true" />
            <input
              type="search"
              placeholder={copy.page.searchPlaceholder}
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              aria-label={copy.page.searchAria}
            />
          </div>
          <CategorySortMenu
            value={sortMode}
            options={sortModeOptions}
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
            {copy.page.batchToggle}
          </button>
        </div>

        {batchMode && state.status === "ready" && (
          <BatchActionBar
            copy={copy}
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
            <strong>{copy.page.loadingTitle}</strong>
            <span>{copy.page.loadingHint}</span>
          </div>
        )}

        {state.status === "error" && (
          <div className="category-state-card is-error" role="alert">
            <AlertTriangle size={20} aria-hidden="true" />
            <strong>{copy.page.errorTitle}</strong>
            <span>{copy.page.errorHint}</span>
            <button type="button" className="category-action-button is-primary" onClick={refresh}>
              {copy.page.retry}
            </button>
          </div>
        )}

        {isEmpty && (
          <div className="category-state-card is-empty" role="status">
            <Tags size={22} strokeWidth={1.8} aria-hidden="true" />
            <strong>{copy.page.emptyTitle}</strong>
            <span>{copy.page.emptyHint}</span>
            <button type="button" className="category-action-button is-primary" onClick={openCreateForm}>
              <Plus size={14} strokeWidth={2.2} aria-hidden="true" />
              {copy.page.emptyCreate}
            </button>
          </div>
        )}

        {isSearchEmpty && (
          <div className="category-state-card is-empty" role="status">
            <SearchX size={22} strokeWidth={1.8} aria-hidden="true" />
            <strong>{copy.page.searchEmptyTitle}</strong>
            <span>{copy.page.searchEmptyHint}</span>
            <button type="button" className="category-action-button" onClick={() => setQuery("")}>
              {copy.page.clearSearch}
            </button>
          </div>
        )}

        {visibleCategories.length > 0 && (
          <CategoryList
            copy={copy}
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
              setStatusMessage(copy.page.saved);
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
              setStatusMessage(copy.page.deleted);
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
            {copy.page.savingOrder}
          </p>
        )}
      </div>
    </section>
  );
}

type CreateCategoryFormProps = {
  copy: CategoryCopy;
  allCategories: CategoryItem[];
  onCreated: () => void;
  onCancel: () => void;
};

function CreateCategoryForm({ copy, allCategories, onCreated, onCancel }: CreateCategoryFormProps) {
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
        setError(getCategoryMutationErrorMessage(err, copy.form.createFailed));
        setSubmitting(false);
        return;
      }

      onCreated();
    })();
  };

  return (
    <form id="category-create-form" className="category-create-form" onSubmit={handleSubmit} aria-label={copy.form.formAria}>
      <label className="category-form-field is-grow">
        <span>{copy.form.nameField}</span>
        <input
          type="text"
          placeholder={copy.form.namePlaceholder}
          value={name}
          onChange={(e) => setName(e.target.value)}
          autoFocus
        />
      </label>
      <div className="category-form-field">
        <span>{copy.form.colorField}</span>
        <CategoryColorPicker value={color} onChange={setColor} triggerLabel={copy.form.newColorAria} />
      </div>
      <div className="category-form-actions">
        <button type="submit" className="category-action-button is-primary" disabled={!canSubmit}>
          {submitting ? copy.form.creating : copy.form.create}
        </button>
        <button type="button" className="category-action-button" onClick={onCancel}>
          {copy.form.cancel}
        </button>
      </div>
      {duplicate && (
        <p className="category-inline-error" role="alert">
          {copy.form.duplicateName(duplicate.name)}
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
  copy: CategoryCopy;
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
  copy,
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
    <div className="category-batch-bar" role="group" aria-label={copy.batch.barAria}>
      <label className="category-batch-bar__select-all">
        <input
          type="checkbox"
          checked={allSelected}
          onChange={onToggleSelectAll}
          disabled={!hasVisible || busy}
          aria-label={copy.batch.selectAllAria}
        />
        {copy.batch.selectAll}
      </label>
      <span className="category-batch-bar__summary" aria-live="polite">
        {hasSelection
          ? copy.batch.selectionSummary(summary.count, summary.linkedModCount)
          : copy.batch.selectionHint}
      </span>
      <div className="category-batch-bar__actions">
        <div className="category-batch-bar__color">
          <CategoryColorPicker value={color} onChange={setColor} triggerLabel={copy.batch.targetColorAria} />
          <button
            type="button"
            className="category-action-button"
            disabled={!hasSelection || busy}
            onClick={() => onBatchColor(color.trim())}
          >
            <Palette size={14} strokeWidth={2.2} aria-hidden="true" />
            {copy.batch.applyColor}
          </button>
        </div>
        <button
          type="button"
          className="category-action-button is-danger"
          disabled={!hasSelection || busy}
          onClick={() => setConfirmingDelete(true)}
        >
          <Trash2 size={14} strokeWidth={2.2} aria-hidden="true" />
          {copy.batch.batchDelete}
        </button>
        <button type="button" className="category-action-button" onClick={onExit} disabled={busy}>
          {copy.batch.exitBatch}
        </button>
      </div>

      {confirmingDelete && hasSelection && (
        <div className={`category-delete-confirm ${summary.linkedModCount > 0 ? "is-danger" : ""}`}>
          <p>
            {summary.linkedModCount > 0
              ? copy.batch.confirmDeleteLinked(summary.count, summary.linkedModCount)
              : copy.batch.confirmDelete(summary.count)}
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
              {busy ? copy.batch.deleting : copy.batch.delete}
            </button>
            <button
              type="button"
              className="category-action-button"
              onClick={() => setConfirmingDelete(false)}
            >
              {copy.batch.cancel}
            </button>
          </div>
        </div>
      )}
      {busy && <p className="category-batch-bar__busy">{copy.batch.busy}</p>}
    </div>
  );
}
