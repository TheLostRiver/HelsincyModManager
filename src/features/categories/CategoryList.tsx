import {
  ChevronDown,
  ChevronUp,
  GripVertical,
  Link2,
  Pencil,
  Trash2,
} from "lucide-react";
import { useState, type DragEvent, type FormEvent } from "react";
import { deleteCategory, updateCategory, type CategoryItem } from "./categoryApi";
import { CategoryColorPicker } from "./CategoryColorPicker";
import {
  findDuplicateCategoryName,
  getCategoryMutationErrorMessage,
} from "./categoryWorkflow";

type CategoryListProps = {
  categories: CategoryItem[];
  allCategories: CategoryItem[];
  reorderEnabled: boolean;
  batchMode: boolean;
  selectedIds: ReadonlySet<string>;
  editingId: string | null;
  deletingId: string | null;
  savingOrder: boolean;
  onToggleSelect: (categoryId: string) => void;
  onStartEdit: (categoryId: string) => void;
  onCancelEdit: () => void;
  onSaved: () => void;
  onStartDelete: (categoryId: string) => void;
  onCancelDelete: () => void;
  onDeleted: () => void;
  onReorder: (fromIndex: number, insertIndex: number) => void;
  onMove: (categoryId: string, offset: -1 | 1) => void;
};

export function CategoryList({
  categories,
  allCategories,
  reorderEnabled,
  batchMode,
  selectedIds,
  editingId,
  deletingId,
  savingOrder,
  onToggleSelect,
  onStartEdit,
  onCancelEdit,
  onSaved,
  onStartDelete,
  onCancelDelete,
  onDeleted,
  onReorder,
  onMove,
}: CategoryListProps) {
  const [dragIndex, setDragIndex] = useState<number | null>(null);
  const [dropIndex, setDropIndex] = useState<number | null>(null);

  const clearDragState = () => {
    setDragIndex(null);
    setDropIndex(null);
  };

  const handleDragStart = (event: DragEvent<HTMLDivElement>, index: number) => {
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", categories[index].id);
    setDragIndex(index);
  };

  const handleDragOver = (event: DragEvent<HTMLDivElement>, index: number) => {
    if (dragIndex === null) {
      return;
    }
    event.preventDefault();
    event.dataTransfer.dropEffect = "move";
    const rect = event.currentTarget.getBoundingClientRect();
    const before = event.clientY < rect.top + rect.height / 2;
    setDropIndex(before ? index : index + 1);
  };

  const handleDrop = (event: DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    if (dragIndex !== null && dropIndex !== null) {
      onReorder(dragIndex, dropIndex);
    }
    clearDragState();
  };

  return (
    <div className="category-list" role="list" aria-label="分类列表">
      <div className="category-list__header" aria-hidden="true">
        <span className="category-list__header-lead">分类</span>
        <span>关联 Mod</span>
        <span className="category-list__header-actions">操作</span>
      </div>
      {categories.map((category, index) => {
        if (editingId === category.id) {
          return (
            <CategoryEditRow
              key={category.id}
              category={category}
              allCategories={allCategories}
              onCancel={onCancelEdit}
              onSaved={onSaved}
            />
          );
        }

        const dropBefore = dropIndex === index && dragIndex !== null;
        const dropAfter = dropIndex === index + 1 && index === categories.length - 1 && dragIndex !== null;

        return (
          <CategoryRow
            key={category.id}
            category={category}
            index={index}
            total={categories.length}
            reorderEnabled={reorderEnabled && editingId === null}
            batchMode={batchMode}
            selected={selectedIds.has(category.id)}
            isDeleting={deletingId === category.id}
            isDragSource={dragIndex === index}
            dropBefore={dropBefore}
            dropAfter={dropAfter}
            savingOrder={savingOrder}
            onToggleSelect={() => onToggleSelect(category.id)}
            onStartEdit={() => onStartEdit(category.id)}
            onStartDelete={() => onStartDelete(category.id)}
            onCancelDelete={onCancelDelete}
            onDeleted={onDeleted}
            onMove={(offset) => onMove(category.id, offset)}
            onDragStart={(event) => handleDragStart(event, index)}
            onDragOver={(event) => handleDragOver(event, index)}
            onDrop={handleDrop}
            onDragEnd={clearDragState}
          />
        );
      })}
    </div>
  );
}

type CategoryRowProps = {
  category: CategoryItem;
  index: number;
  total: number;
  reorderEnabled: boolean;
  batchMode: boolean;
  selected: boolean;
  isDeleting: boolean;
  isDragSource: boolean;
  dropBefore: boolean;
  dropAfter: boolean;
  savingOrder: boolean;
  onToggleSelect: () => void;
  onStartEdit: () => void;
  onStartDelete: () => void;
  onCancelDelete: () => void;
  onDeleted: () => void;
  onMove: (offset: -1 | 1) => void;
  onDragStart: (event: DragEvent<HTMLDivElement>) => void;
  onDragOver: (event: DragEvent<HTMLDivElement>) => void;
  onDrop: (event: DragEvent<HTMLDivElement>) => void;
  onDragEnd: () => void;
};

function CategoryRow({
  category,
  index,
  total,
  reorderEnabled,
  batchMode,
  selected,
  isDeleting,
  isDragSource,
  dropBefore,
  dropAfter,
  savingOrder,
  onToggleSelect,
  onStartEdit,
  onStartDelete,
  onCancelDelete,
  onDeleted,
  onMove,
  onDragStart,
  onDragOver,
  onDrop,
  onDragEnd,
}: CategoryRowProps) {
  const rowClasses = [
    "category-row",
    isDragSource ? "is-dragging" : "",
    dropBefore ? "is-drop-before" : "",
    dropAfter ? "is-drop-after" : "",
    batchMode && selected ? "is-selected" : "",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div
      className={rowClasses}
      role="listitem"
      draggable={reorderEnabled}
      onDragStart={reorderEnabled ? onDragStart : undefined}
      onDragOver={reorderEnabled ? onDragOver : undefined}
      onDrop={reorderEnabled ? onDrop : undefined}
      onDragEnd={reorderEnabled ? onDragEnd : undefined}
    >
      <div className="category-row__lead">
        {batchMode ? (
          <input
            type="checkbox"
            className="category-row__checkbox"
            checked={selected}
            onChange={onToggleSelect}
            aria-label={`选择 ${category.name}`}
          />
        ) : (
          <span
            className={`category-row__grip ${reorderEnabled ? "" : "is-disabled"}`}
            title={reorderEnabled ? "拖拽调整顺序" : "在“自定义排序”视图且未搜索时可拖拽排序"}
            aria-hidden="true"
          >
            <GripVertical size={15} strokeWidth={2} />
          </span>
        )}
        <span className="category-row__order" aria-hidden="true">
          {String(index + 1).padStart(2, "0")}
        </span>
        <span
          className="category-swatch"
          style={{ background: category.color || undefined }}
          aria-hidden="true"
        />
        <div className="category-row__name-block">
          <strong>{category.name}</strong>
          <span className="category-row__meta">
            <span>{category.color ? category.color.toUpperCase() : "默认颜色"}</span>
            <span>顺序 {String(index + 1).padStart(2, "0")}</span>
          </span>
        </div>
      </div>
      <span className={`category-row__count ${category.modCount === 0 ? "is-empty" : ""}`}>
        <Link2 size={13} strokeWidth={2} aria-hidden="true" />
        {category.modCount === 0 ? "空分类" : `${category.modCount} 个 Mod`}
      </span>
      {!batchMode && (
        <div className="category-row__actions">
          <button
            type="button"
            className="category-icon-button"
            onClick={() => onMove(-1)}
            disabled={!reorderEnabled || savingOrder || index === 0}
            aria-label={`上移 ${category.name}`}
            title={reorderEnabled ? "上移" : "在“自定义排序”视图且未搜索时可调整顺序"}
          >
            <ChevronUp size={15} strokeWidth={2.2} />
          </button>
          <button
            type="button"
            className="category-icon-button"
            onClick={() => onMove(1)}
            disabled={!reorderEnabled || savingOrder || index === total - 1}
            aria-label={`下移 ${category.name}`}
            title={reorderEnabled ? "下移" : "在“自定义排序”视图且未搜索时可调整顺序"}
          >
            <ChevronDown size={15} strokeWidth={2.2} />
          </button>
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
      )}

      {isDeleting && !batchMode && (
        <CategoryDeleteConfirm category={category} onCancel={onCancelDelete} onDeleted={onDeleted} />
      )}
    </div>
  );
}

type CategoryEditRowProps = {
  category: CategoryItem;
  allCategories: CategoryItem[];
  onCancel: () => void;
  onSaved: () => void;
};

function CategoryEditRow({ category, allCategories, onCancel, onSaved }: CategoryEditRowProps) {
  const [name, setName] = useState(category.name);
  const [color, setColor] = useState(category.color ?? "");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const duplicate = findDuplicateCategoryName(allCategories, name, category.id);
  const canSave = name.trim().length > 0 && !duplicate && !submitting;

  const handleSave = (e: FormEvent) => {
    e.preventDefault();
    if (!canSave) return;

    const trimmedColor = color.trim();
    const originalColor = category.color ?? "";

    setError(null);
    setSubmitting(true);
    void (async () => {
      try {
        await updateCategory({
          categoryId: category.id,
          name: name.trim() !== category.name ? name.trim() : undefined,
          color: trimmedColor !== originalColor
            ? (trimmedColor === "" ? null : trimmedColor)
            : undefined,
        });
      } catch (err: unknown) {
        setError(getCategoryMutationErrorMessage(err, "保存分类失败，请稍后重试。"));
        setSubmitting(false);
        return;
      }

      onSaved();
    })();
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
          autoFocus
        />
      </label>
      <div className="category-edit-field category-edit-field--color">
        <span>颜色</span>
        <CategoryColorPicker
          value={color}
          onChange={setColor}
          triggerLabel={`编辑 ${category.name} 的颜色`}
          align="end"
        />
      </div>
      <div className="category-row__edit-actions">
        <button type="submit" className="category-action-button is-primary" disabled={!canSave}>
          {submitting ? "保存中…" : "保存"}
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
    void (async () => {
      try {
        await deleteCategory(category.id);
      } catch (err: unknown) {
        setError(getCategoryMutationErrorMessage(err, "删除分类失败，请稍后重试。"));
        setSubmitting(false);
        return;
      }

      onDeleted();
    })();
  };

  return (
    <div className={`category-delete-confirm ${category.modCount > 0 ? "is-danger" : ""}`}>
      <p>
        {category.modCount > 0
          ? `确定删除「${category.name}」？有 ${category.modCount} 个 Mod 关联将被移除，Mod 本体不受影响。`
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
