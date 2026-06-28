import {
  AlertTriangle,
  ArrowUpDown,
  Hash,
  Link2,
  Loader2,
  Palette,
  Pencil,
  Plus,
  Tags,
  Trash2,
  X,
} from "lucide-react";
import { useId, useMemo, useState, type FormEvent } from "react";
import {
  createCategory,
  deleteCategory,
  updateCategory,
  type CategoryItem,
} from "../mods/modCategoryApi";
import { useCategoryList } from "./useCategoryList";

const emptyCategoryItems: CategoryItem[] = [];
const DEFAULT_PICKER_COLOR = "#3B82F6";
const CATEGORY_COLOR_OPTIONS = [
  { label: "蓝色", value: "#2563EB" },
  { label: "青色", value: "#0891B2" },
  { label: "绿色", value: "#16A34A" },
  { label: "琥珀", value: "#D97706" },
  { label: "红色", value: "#DC2626" },
  { label: "粉色", value: "#DB2777" },
  { label: "紫色", value: "#7C3AED" },
  { label: "灰色", value: "#64748B" },
];

export function CategoryPage() {
  const { state, refresh } = useCategoryList();
  const [showCreateForm, setShowCreateForm] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [deletingId, setDeletingId] = useState<string | null>(null);

  const categories = state.status === "ready" ? state.categories : emptyCategoryItems;
  const isEmpty = state.status === "ready" && categories.length === 0;
  const metrics = useMemo(() => getCategoryMetrics(categories), [categories]);
  const summaryCards = [
    {
      icon: Tags,
      label: "分类总数",
      value: state.status === "ready" ? String(categories.length) : "-",
    },
    {
      icon: Link2,
      label: "已关联 Mod",
      value: state.status === "ready" ? String(metrics.linkedModCount) : "-",
    },
    {
      icon: Hash,
      label: "空分类",
      value: state.status === "ready" ? String(metrics.emptyCategoryCount) : "-",
    },
    {
      icon: Palette,
      label: "颜色标记",
      value: state.status === "ready" ? String(metrics.coloredCategoryCount) : "-",
    },
  ];

  const openCreateForm = () => {
    setShowCreateForm(true);
    setEditingId(null);
    setDeletingId(null);
  };

  const toggleCreateForm = () => {
    setShowCreateForm((current) => !current);
    setEditingId(null);
    setDeletingId(null);
  };

  return (
    <section className="category-page" aria-labelledby="category-title">
      <header className="category-page__header">
        <div className="category-page__title-block">
          <span className="category-page__eyebrow">
            <Tags size={15} strokeWidth={2.2} />
            分类 / 标签
          </span>
          <h1 id="category-title">分类管理</h1>
        </div>
        <button
          type="button"
          className={`category-create-trigger ${showCreateForm ? "is-open" : ""}`}
          aria-expanded={showCreateForm}
          aria-controls="category-create-form"
          onClick={toggleCreateForm}
        >
          {showCreateForm ? <X size={15} strokeWidth={2.2} /> : <Plus size={15} strokeWidth={2.2} />}
          {showCreateForm ? "取消新建" : "新建分类"}
        </button>
      </header>

      <div className="category-page__summary-grid" aria-label="分类摘要">
        {summaryCards.map((item) => {
          const Icon = item.icon;

          return (
            <div className="category-summary-card" key={item.label}>
              <span className="category-summary-card__icon" aria-hidden="true">
                <Icon size={18} strokeWidth={2.1} />
              </span>
              <span className="category-summary-card__label">{item.label}</span>
              <strong>{item.value}</strong>
            </div>
          );
        })}
      </div>

      <div className="category-workspace">
        <div className="category-main-card">
          <div className="category-main-card__header">
            <div>
              <h2>分类列表</h2>
              <span>
                {state.status === "ready"
                  ? `${categories.length} 个分类`
                  : state.status === "loading"
                    ? "读取中"
                    : "读取失败"}
              </span>
            </div>
          </div>

          {showCreateForm && (
            <CreateCategoryForm
              onCreated={() => {
                setShowCreateForm(false);
                refresh();
              }}
              onCancel={() => setShowCreateForm(false)}
            />
          )}

          {state.status === "loading" && (
            <div className="category-state-card" role="status">
              <Loader2 size={18} className="category-spinner" />
              <strong>正在读取分类</strong>
              <span>请稍候</span>
            </div>
          )}

          {state.status === "error" && (
            <div className="category-state-card is-error" role="alert">
              <AlertTriangle size={20} />
              <strong>无法加载分类列表</strong>
              <span>分类数据暂时不可用。</span>
              <button type="button" className="category-action-button is-primary" onClick={refresh}>
                重试
              </button>
            </div>
          )}

          {isEmpty && !showCreateForm && (
            <div className="category-state-card is-empty" role="status">
              <Tags size={22} strokeWidth={1.8} />
              <strong>还没有分类</strong>
              <span>新建后会显示在这里。</span>
              <button type="button" className="category-action-button is-primary" onClick={openCreateForm}>
                <Plus size={14} strokeWidth={2.2} />
                新建分类
              </button>
            </div>
          )}

          {categories.length > 0 && (
            <div className="category-list" role="list" aria-label="分类列表">
              <div className="category-list__header" aria-hidden="true">
                <span>分类</span>
                <span>排序</span>
                <span>关联</span>
                <span>颜色</span>
                <span>操作</span>
              </div>
              {categories.map((cat) => (
                <CategoryRow
                  key={cat.id}
                  category={cat}
                  isEditing={editingId === cat.id}
                  isDeleting={deletingId === cat.id}
                  onStartEdit={() => {
                    setEditingId(cat.id);
                    setDeletingId(null);
                    setShowCreateForm(false);
                  }}
                  onCancelEdit={() => setEditingId(null)}
                  onSaved={() => {
                    setEditingId(null);
                    refresh();
                  }}
                  onStartDelete={() => {
                    setDeletingId(cat.id);
                    setEditingId(null);
                    setShowCreateForm(false);
                  }}
                  onCancelDelete={() => setDeletingId(null)}
                  onDeleted={() => {
                    setDeletingId(null);
                    refresh();
                  }}
                />
              ))}
            </div>
          )}
        </div>

        <aside className="category-preview-panel" aria-label="分类颜色预览">
          <div className="category-preview-panel__header">
            <h2>颜色预览</h2>
            <span>{state.status === "ready" ? `${metrics.coloredCategoryCount} 个已设置` : "-"}</span>
          </div>

          {categories.length > 0 ? (
            <div className="category-preview-list">
              {categories.slice(0, 8).map((category) => (
                <span className="category-preview-chip" key={category.id}>
                  <span
                    className="category-swatch"
                    style={{ background: category.color || undefined }}
                    aria-hidden="true"
                  />
                  <span>{category.name}</span>
                </span>
              ))}
              {categories.length > 8 && (
                <span className="category-preview-chip is-more">+{categories.length - 8}</span>
              )}
            </div>
          ) : (
            <p className="category-preview-panel__empty">
              {state.status === "error" ? "暂无预览" : "等待分类数据"}
            </p>
          )}
        </aside>
      </div>
    </section>
  );
}

type CategoryMetrics = {
  linkedModCount: number;
  emptyCategoryCount: number;
  coloredCategoryCount: number;
};

function getCategoryMetrics(categories: CategoryItem[]): CategoryMetrics {
  return categories.reduce<CategoryMetrics>(
    (metrics, category) => ({
      linkedModCount: metrics.linkedModCount + category.modCount,
      emptyCategoryCount: metrics.emptyCategoryCount + (category.modCount === 0 ? 1 : 0),
      coloredCategoryCount: metrics.coloredCategoryCount + (category.color ? 1 : 0),
    }),
    {
      linkedModCount: 0,
      emptyCategoryCount: 0,
      coloredCategoryCount: 0,
    },
  );
}

type CreateCategoryFormProps = {
  onCreated: () => void;
  onCancel: () => void;
};

function CreateCategoryForm({ onCreated, onCancel }: CreateCategoryFormProps) {
  const [name, setName] = useState("");
  const [color, setColor] = useState("");
  const [sortOrder, setSortOrder] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const parsedSortOrder = parseSortOrder(sortOrder);
  const sortOrderValid = sortOrder.trim() === "" || parsedSortOrder !== undefined;
  const canSubmit = name.trim().length > 0 && sortOrderValid && !submitting;

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();
    if (!canSubmit) return;

    setError(null);
    setSubmitting(true);
    void createCategory({
      name: name.trim(),
      color: color.trim() || undefined,
      sortOrder: parsedSortOrder,
    })
      .then(() => onCreated())
      .catch((err: unknown) => {
        setError(formatCategoryMutationError(err, "创建分类失败，请稍后重试。"));
        setSubmitting(false);
      });
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
        <CategoryColorPicker value={color} onChange={setColor} />
      </div>
      <label className="category-form-field">
        <span>排序</span>
        <input
          type="number"
          placeholder="0"
          value={sortOrder}
          onChange={(e) => setSortOrder(e.target.value)}
          className="category-sort-input"
        />
      </label>
      <div className="category-form-actions">
        <button type="submit" className="category-action-button is-primary" disabled={!canSubmit}>
          {submitting ? "创建中…" : "创建"}
        </button>
        <button type="button" className="category-action-button" onClick={onCancel}>
          取消
        </button>
      </div>
      {error && (
        <p className="category-inline-error" role="alert">
          {error}
        </p>
      )}
    </form>
  );
}

type CategoryColorPickerProps = {
  value: string;
  onChange: (value: string) => void;
};

function CategoryColorPicker({ value, onChange }: CategoryColorPickerProps) {
  const [open, setOpen] = useState(false);
  const popoverId = useId();
  const selectedColor = isValidColor(value) ? value.trim() : "";
  const nativePickerColor = isFullHexColor(selectedColor) ? selectedColor : DEFAULT_PICKER_COLOR;

  const selectColor = (color: string) => {
    onChange(color);
    setOpen(false);
  };

  return (
    <div className="category-color-picker">
      <button
        type="button"
        className="category-color-picker__trigger"
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-controls={popoverId}
        onClick={() => setOpen((current) => !current)}
      >
        <span
          className="category-swatch"
          style={{ background: selectedColor || undefined }}
          aria-hidden="true"
        />
        <span>{selectedColor ? "已选颜色" : "默认颜色"}</span>
      </button>

      {open && (
        <div className="category-color-popover" id={popoverId} role="dialog" aria-label="选择分类颜色">
          <div className="category-color-palette" aria-label="常用颜色">
            {CATEGORY_COLOR_OPTIONS.map((option) => {
              const isSelected = selectedColor.toLowerCase() === option.value.toLowerCase();

              return (
                <button
                  type="button"
                  className={`category-color-swatch-button ${isSelected ? "is-selected" : ""}`}
                  key={option.value}
                  onClick={() => selectColor(option.value)}
                  aria-label={`选择${option.label}`}
                  aria-pressed={isSelected}
                  title={option.label}
                >
                  <span
                    className="category-swatch"
                    style={{ background: option.value }}
                    aria-hidden="true"
                  />
                </button>
              );
            })}
          </div>
          <label className="category-custom-color">
            <span>自定义</span>
            <input
              type="color"
              value={nativePickerColor}
              onChange={(event) => onChange(event.target.value)}
            />
          </label>
          <button type="button" className="category-color-clear" onClick={() => selectColor("")}>
            默认颜色
          </button>
        </div>
      )}
    </div>
  );
}

type CategoryRowProps = {
  category: CategoryItem;
  isEditing: boolean;
  isDeleting: boolean;
  onStartEdit: () => void;
  onCancelEdit: () => void;
  onSaved: () => void;
  onStartDelete: () => void;
  onCancelDelete: () => void;
  onDeleted: () => void;
};

function CategoryRow({
  category,
  isEditing,
  isDeleting,
  onStartEdit,
  onCancelEdit,
  onSaved,
  onStartDelete,
  onCancelDelete,
  onDeleted,
}: CategoryRowProps) {
  if (isEditing) {
    return <CategoryEditRow category={category} onCancel={onCancelEdit} onSaved={onSaved} />;
  }

  return (
    <div className="category-row" role="listitem">
      <div className="category-row__main">
        <span
          className="category-swatch"
          style={{ background: category.color || undefined }}
          aria-hidden="true"
        />
        <div className="category-row__name-block">
          <strong>{category.name}</strong>
          <span>{category.color || "默认颜色"}</span>
        </div>
      </div>
      <span className="category-row__sort" title="排序值">
        <ArrowUpDown size={13} strokeWidth={2} />
        {category.sortOrder}
      </span>
      <span className="category-row__count">
        <Link2 size={13} strokeWidth={2} />
        {category.modCount} 个
      </span>
      <span className="category-row__color">{category.color ? "已设置" : "未设置"}</span>
      <div className="category-row__actions">
        <button
          type="button"
          className="category-icon-button"
          onClick={onStartEdit}
          aria-label={`编辑 ${category.name}`}
          title="编辑"
        >
          <Pencil size={15} strokeWidth={2} />
        </button>
        <button
          type="button"
          className="category-icon-button is-danger"
          onClick={onStartDelete}
          aria-label={`删除 ${category.name}`}
          title="删除"
        >
          <Trash2 size={15} strokeWidth={2} />
        </button>
      </div>

      {isDeleting && (
        <CategoryDeleteConfirm
          category={category}
          onCancel={onCancelDelete}
          onDeleted={onDeleted}
        />
      )}
    </div>
  );
}

type CategoryEditRowProps = {
  category: CategoryItem;
  onCancel: () => void;
  onSaved: () => void;
};

function CategoryEditRow({ category, onCancel, onSaved }: CategoryEditRowProps) {
  const [name, setName] = useState(category.name);
  const [color, setColor] = useState(category.color ?? "");
  const [sortOrder, setSortOrder] = useState(String(category.sortOrder));
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const parsedSortOrder = parseSortOrder(sortOrder);
  const sortOrderValid = sortOrder.trim() === "" || parsedSortOrder !== undefined;
  const canSave = name.trim().length > 0 && sortOrderValid && !submitting;

  const handleSave = (e: FormEvent) => {
    e.preventDefault();
    if (!canSave) return;

    const trimmedColor = color.trim();
    const originalColor = category.color ?? "";

    setError(null);
    setSubmitting(true);
    void updateCategory({
      categoryId: category.id,
      name: name.trim() !== category.name ? name.trim() : undefined,
      color: trimmedColor !== originalColor
        ? (trimmedColor === "" ? null : trimmedColor)
        : undefined,
      sortOrder: parsedSortOrder !== undefined && parsedSortOrder !== category.sortOrder
        ? parsedSortOrder
        : undefined,
    })
      .then(() => onSaved())
      .catch((err: unknown) => {
        setError(formatCategoryMutationError(err, "保存分类失败，请稍后重试。"));
        setSubmitting(false);
      });
  };

  return (
    <form
      className="category-row is-editing"
      role="listitem"
      onSubmit={handleSave}
      aria-label={`编辑 ${category.name}`}
    >
      <label className="category-edit-field category-edit-field--name">
        <span>名称</span>
        <input
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          className="category-edit-name"
          autoFocus
        />
      </label>
      <label className="category-edit-field category-edit-field--sort">
        <span>排序</span>
        <input
          type="number"
          value={sortOrder}
          onChange={(e) => setSortOrder(e.target.value)}
          className="category-sort-input"
        />
      </label>
      <span className="category-row__count">
        <Link2 size={13} strokeWidth={2} />
        {category.modCount} 个
      </span>
      <div className="category-edit-field category-edit-field--color">
        <span>颜色</span>
        <CategoryColorPicker value={color} onChange={setColor} />
      </div>
      <div className="category-row__edit-actions">
        <button type="submit" className="category-action-button is-primary" disabled={!canSave}>
          {submitting ? "保存中…" : "保存"}
        </button>
        <button type="button" className="category-action-button" onClick={onCancel}>
          取消
        </button>
      </div>
      {error && (
        <p className="category-inline-error" role="alert">
          {error}
        </p>
      )}
    </form>
  );
}

type CategoryDeleteConfirmProps = {
  category: CategoryItem;
  onCancel: () => void;
  onDeleted: () => void;
};

function CategoryDeleteConfirm({ category, onCancel, onDeleted }: CategoryDeleteConfirmProps) {
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleConfirm = () => {
    setError(null);
    setSubmitting(true);
    void deleteCategory(category.id)
      .then(() => onDeleted())
      .catch((err: unknown) => {
        setError(formatCategoryMutationError(err, "删除分类失败，请稍后重试。"));
        setSubmitting(false);
      });
  };

  return (
    <div className={`category-delete-confirm ${category.modCount > 0 ? "is-danger" : ""}`}>
      <p>
        {category.modCount > 0
          ? `确定删除「${category.name}」？有 ${category.modCount} 个 Mod 关联将被移除。`
          : `确定删除「${category.name}」？`}
      </p>
      <div className="category-delete-confirm__actions">
        <button
          type="button"
          className="category-action-button is-danger"
          onClick={handleConfirm}
          disabled={submitting}
        >
          {submitting ? "删除中…" : "删除"}
        </button>
        <button type="button" className="category-action-button" onClick={onCancel}>
          取消
        </button>
      </div>
      {error && (
        <p className="category-inline-error" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}

function formatCategoryMutationError(error: unknown, fallback: string): string {
  if (typeof error === "string" && error.trim()) {
    return error;
  }

  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }

  if (typeof error === "object" && error !== null && "message" in error) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === "string" && message.trim()) {
      return message;
    }
  }

  return fallback;
}

function parseSortOrder(value: string): number | undefined {
  const trimmed = value.trim();
  if (trimmed === "") {
    return undefined;
  }

  const parsed = Number(trimmed);
  return Number.isFinite(parsed) ? parsed : undefined;
}

function isValidColor(value: string): boolean {
  return /^#(?:[0-9a-fA-F]{3}){1,2}$/.test(value.trim());
}

function isFullHexColor(value: string): boolean {
  return /^#[0-9a-fA-F]{6}$/.test(value.trim());
}
