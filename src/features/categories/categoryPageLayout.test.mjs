import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(__dirname, "../../..");

function readProjectFile(relativePath) {
  return readFileSync(join(repoRoot, relativePath), "utf8");
}

function getRuleBody(css, selector) {
  const start = css.indexOf(`${selector} {`);
  assert.ok(start >= 0, `missing CSS rule: ${selector}`);

  const openBraceIndex = css.indexOf("{", start);
  const closeBraceIndex = css.indexOf("}", openBraceIndex);
  assert.ok(openBraceIndex >= 0 && closeBraceIndex > openBraceIndex, `invalid CSS rule: ${selector}`);
  return css.slice(openBraceIndex + 1, closeBraceIndex);
}

test("category page owns a dense single-column management layout with a workflow toolbar", () => {
  const source = readProjectFile("src/features/categories/CategoryPage.tsx");

  assert.match(source, /className="category-page__header"/);
  assert.match(source, /className="category-page__metrics"/);
  assert.match(source, /className="category-main-card"/);
  assert.match(source, /className="category-toolbar"/);
  assert.match(source, /className="category-search"/);
  assert.match(source, /<CategorySortMenu/);
  assert.match(source, /aria-label="搜索分类"/);
  assert.match(source, /className="category-state-card is-error"/);
  assert.match(source, /className="category-state-card is-empty"/);
  assert.match(source, /没有匹配的分类/);
  assert.doesNotMatch(source, /useSidebarMode|sidebarMode/);
  assert.doesNotMatch(source, /category-preview-panel|category-page__summary-grid/);
  assert.doesNotMatch(source, /<select/);
});

test("category page CSS fills the route cleanly and stays responsive", () => {
  const css = readProjectFile("src/features/categories/CategoryPage.css");

  assert.match(css, /\.route-transition__layer\[data-route-id="categories"\]\s*{[\s\S]*?grid-template-columns:\s*minmax\(0,\s*1fr\);/);
  assert.match(getRuleBody(css, ".category-page"), /gap:\s*var\(--layout-content-gap\);/);
  assert.match(getRuleBody(css, ".category-main-card"), /box-shadow:\s*var\(--shadow-soft\);/);
  assert.match(getRuleBody(css, ".category-list__header,\n.category-row"), /grid-template-columns:\s*var\(--category-list-columns\);/);
  assert.match(getRuleBody(css, ".category-state-card.is-error"), /border:\s*1px\s+solid/);
  assert.doesNotMatch(getRuleBody(css, ".category-state-card.is-error"), /background:\s*var\(--color-warning-bg\);/);
  assert.match(css, /@media\s*\(max-width:\s*860px\)\s*{[\s\S]*?\.category-row\s*{[\s\S]*?grid-template-columns:\s*1fr;/);
  assert.match(css, /@media\s*\(prefers-reduced-motion:\s*reduce\)/);
});

test("category ordering uses drag handles and keyboard move buttons instead of numeric input", () => {
  const page = readProjectFile("src/features/categories/CategoryPage.tsx");
  const list = readProjectFile("src/features/categories/CategoryList.tsx");
  const workflow = readProjectFile("src/features/categories/categoryWorkflow.ts");
  const css = readProjectFile("src/features/categories/CategoryPage.css");

  assert.match(list, /draggable=\{reorderEnabled\}/);
  assert.match(list, /onDragStart/);
  assert.match(list, /onDragOver/);
  assert.match(list, /onDrop/);
  assert.match(list, /aria-label=\{`上移 \$\{category.name\}`\}/);
  assert.match(list, /aria-label=\{`下移 \$\{category.name\}`\}/);
  assert.doesNotMatch(list, /type="number"/);
  assert.doesNotMatch(page, /type="number"/);
  assert.match(page, /buildSortOrderUpdates/);
  assert.match(page, /reorderCategoryList/);
  assert.match(page, /moveCategoryByOffset/);
  assert.match(page, /nextAppendSortOrder/);
  assert.match(workflow, /export function buildSortOrderUpdates/);
  assert.match(getRuleBody(css, ".category-row.is-drop-before"), /var\(--color-accent\)/);
  assert.match(getRuleBody(css, ".category-row.is-dragging"), /opacity/);
});

test("category page ships search, sort views and batch operations", () => {
  const page = readProjectFile("src/features/categories/CategoryPage.tsx");
  const sortMenu = readProjectFile("src/features/categories/CategorySortMenu.tsx");
  const workflow = readProjectFile("src/features/categories/categoryWorkflow.ts");
  const css = readProjectFile("src/features/categories/CategoryPage.css");

  assert.match(page, /filterCategories/);
  assert.match(page, /sortCategoriesForView/);
  assert.match(page, /canReorderCategories/);
  assert.match(sortMenu, /role="listbox"/);
  assert.match(sortMenu, /role="option"/);
  assert.match(sortMenu, /aria-selected=\{selected\}/);
  assert.match(sortMenu, /Escape/);
  assert.match(sortMenu, /pointerdown/);
  assert.match(getRuleBody(css, ".category-sort-menu__trigger"), /border-radius:\s*6px;/);
  assert.match(getRuleBody(css, ".category-sort-menu__popover"), /position:\s*absolute;/);
  assert.match(getRuleBody(css, ".category-sort-menu__popover"), /border-radius:\s*8px;/);
  assert.match(
    getRuleBody(css, ".category-sort-menu__option"),
    /grid-template-columns:\s*minmax\(0,\s*1fr\)\s+auto;/,
  );
  assert.match(getRuleBody(css, ".category-sort-menu__option"), /border-radius:\s*6px;/);
  assert.doesNotMatch(sortMenu, /<select/);
  assert.match(page, /aria-pressed=\{batchMode\}/);
  assert.match(page, /aria-label="全选当前列表"/);
  assert.match(page, /批量删除/);
  assert.match(page, /应用颜色/);
  assert.match(getRuleBody(css, ".category-batch-bar"), /animation:\s*category-batch-bar-enter/);
  assert.match(css, /@keyframes\s+category-batch-bar-enter/);
  assert.match(
    css,
    /@media\s*\(prefers-reduced-motion:\s*reduce\)\s*{[\s\S]*?\.category-batch-bar\s*{[\s\S]*?animation:\s*none;/,
  );
  assert.match(page, /summarizeBatchTargets/);
  assert.match(page, /aria-live="polite"/);
  assert.match(workflow, /export type CategorySortMode = "custom" \| "name" \| "modCount";/);
});

test("category workspace exposes scoped mode tabs and a compact metric strip", () => {
  const page = readProjectFile("src/features/categories/CategoryPage.tsx");
  const list = readProjectFile("src/features/categories/CategoryList.tsx");
  const css = readProjectFile("src/features/categories/CategoryPage.css");

  assert.match(page, /className="category-mode-tabs"/);
  assert.match(page, /aria-label="分类标签管理范围"/);
  assert.match(page, /aria-current="page"/);
  assert.match(page, /aria-disabled="true"/);
  assert.match(page, /className="category-summary-strip"/);
  assert.match(page, /metrics\.coloredCategoryCount/);
  assert.match(page, /已设置颜色/);
  assert.match(list, /className="category-row__order"/);
  assert.match(list, /className="category-row__meta"/);
  assert.match(getRuleBody(css, ".category-mode-tabs"), /display:\s*inline-flex;/);
  assert.match(getRuleBody(css, ".category-summary-strip"), /grid-template-columns:\s*repeat\(4,\s*minmax\(0,\s*1fr\)\);/);
  assert.match(getRuleBody(css, ".category-row__order"), /font-variant-numeric:\s*tabular-nums;/);
  assert.match(getRuleBody(css, ".category-row__meta"), /display:\s*flex;/);
});

test("category create workflow opens in an accessible floating glass panel", () => {
  const page = readProjectFile("src/features/categories/CategoryPage.tsx");
  const css = readProjectFile("src/features/categories/CategoryPage.css");

  assert.match(page, /className="category-create-floating"/);
  assert.match(page, /className="category-create-scrim"/);
  assert.match(page, /className="category-create-panel"/);
  assert.match(page, /role="dialog"/);
  assert.match(page, /aria-modal="true"/);
  assert.match(page, /aria-labelledby="category-create-title"/);
  assert.match(page, /id="category-create-title"/);
  assert.match(page, /id="category-create-distortion"/);
  assert.match(page, /关闭新建分类/);
  assert.match(page, /key === "Escape"/);
  assert.doesNotMatch(
    page,
    /<div className="category-main-card">[\s\S]*?\{showCreateForm && \(\s*<CreateCategoryForm/,
  );

  assert.match(getRuleBody(css, ".category-create-floating"), /position:\s*fixed;/);
  assert.match(getRuleBody(css, ".category-create-floating"), /inset:\s*0;/);
  assert.match(getRuleBody(css, ".category-create-scrim"), /backdrop-filter:\s*blur/);
  assert.match(getRuleBody(css, ".category-create-panel::before"), /backdrop-filter:\s*blur/);
  assert.match(getRuleBody(css, ".category-create-panel::before"), /filter:\s*url\("#category-create-distortion"\)/);
  assert.match(getRuleBody(css, ".category-create-panel"), /width:\s*min/);
  assert.match(getRuleBody(css, ".category-create-panel"), /border-radius:\s*10px;/);
  assert.match(getRuleBody(css, ".category-create-panel"), /animation:\s*category-create-panel-enter/);
  assert.match(css, /@keyframes\s+category-create-panel-enter/);
  assert.match(
    css,
    /@media\s*\(prefers-reduced-motion:\s*reduce\)\s*{[\s\S]*?\.category-create-panel\s*{[\s\S]*?animation:\s*none;/,
  );
});

test("category color editing uses a visual picker instead of hex text entry", () => {
  const picker = readProjectFile("src/features/categories/CategoryColorPicker.tsx");
  const css = readProjectFile("src/features/categories/CategoryPage.css");

  assert.match(picker, /CATEGORY_COLOR_OPTIONS/);
  assert.match(picker, /className="category-color-picker__trigger"/);
  assert.match(picker, /aria-expanded=\{open\}/);
  assert.match(picker, /className="category-color-popover"/);
  assert.match(picker, /className="category-color-palette"/);
  assert.match(picker, /className=\{`category-color-swatch-button/);
  assert.match(picker, /aria-pressed=\{isSelected\}/);
  assert.match(picker, /type="color"/);
  assert.match(picker, /className="category-color-clear"/);
  assert.match(picker, /Escape/);
  assert.match(picker, /pointerdown/);
  assert.match(getRuleBody(css, ".category-color-picker"), /position:\s*relative;/);
  assert.match(getRuleBody(css, ".category-color-popover"), /position:\s*absolute;/);
  assert.match(getRuleBody(css, ".category-color-popover"), /z-index:\s*30;/);
});

test("category row editing and mutations keep accessible feedback", () => {
  const page = readProjectFile("src/features/categories/CategoryPage.tsx");
  const list = readProjectFile("src/features/categories/CategoryList.tsx");
  const workflow = readProjectFile("src/features/categories/categoryWorkflow.ts");
  const css = readProjectFile("src/features/categories/CategoryPage.css");

  assert.match(list, /<form[\s\S]*className="category-row is-editing"[\s\S]*role="listitem"/);
  assert.match(workflow, /export function formatCategoryMutationError/);
  assert.match(workflow, /export function getCategoryMutationErrorMessage/);
  assert.match(workflow, /export function isCategoryCommandError/);
  assert.match(list, /className="category-inline-error" role="alert"/);
  assert.match(page, /className="category-inline-error" role="alert"/);
  assert.match(page, /创建分类失败，请稍后重试。/);
  assert.match(list, /保存分类失败，请稍后重试。/);
  assert.match(list, /删除分类失败，请稍后重试。/);
  assert.match(page, /保存分类顺序失败，请稍后重试。/);
  assert.match(page, /已存在同名分类/);
  assert.match(list, /已存在同名分类/);
  assert.match(getRuleBody(css, ".category-inline-error"), /grid-column:\s*1\s*\/\s*-1;/);
  assert.match(css, /\.category-delete-confirm\s*{[\s\S]*?flex-wrap:\s*wrap;/);
  assert.match(
    getRuleBody(css, ".category-delete-confirm .category-inline-error"),
    /flex-basis:\s*100%;/,
  );
});

test("category typed API stays feature-local and mods re-exports it", () => {
  const categoryApi = readProjectFile("src/features/categories/categoryApi.ts");
  const useCategoryList = readProjectFile("src/features/categories/useCategoryList.ts");
  const modCategoryApi = readProjectFile("src/features/mods/modCategoryApi.ts");

  assert.match(categoryApi, /invoke\("create_category"/);
  assert.match(categoryApi, /invoke\("update_category"/);
  assert.match(categoryApi, /invoke\("delete_category"/);
  assert.match(categoryApi, /invoke\("list_categories"/);
  assert.match(useCategoryList, /from "\.\/categoryApi"/);
  assert.doesNotMatch(useCategoryList, /from "\.\.\/mods\/modCategoryApi"/);
  assert.match(modCategoryApi, /from "\.\.\/categories\/categoryApi"/);
  assert.match(modCategoryApi, /invoke\("set_mod_categories"/);
  assert.match(modCategoryApi, /invoke\("get_mod_categories"/);
  assert.doesNotMatch(modCategoryApi, /invoke\("create_category"/);
});
