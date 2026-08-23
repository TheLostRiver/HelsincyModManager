import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const source = readFileSync("src/features/mods/ModLibraryQueryFeedback.tsx", "utf8");
// I18N-02 起文案钉在 modLibraryCopy 的 zh_cn 字典，组件只能经 copy 键渲染。
const copySource = readFileSync("src/features/mods/modLibraryCopy.ts", "utf8");
const css = readFileSync("src/features/mods/ModLibraryQueryFeedback.css", "utf8");

test("initial loading uses finite view-aware skeletons instead of mock cards", () => {
  assert.match(source, /viewMode === "classic" \|\| viewMode === "grid" \? 8 : 6/);
  assert.match(source, /aria-label=\{copy\.loadingAria\}/);
  assert.match(copySource, /正在加载 Mod 库/);
  assert.doesNotMatch(source, /fallbackModLibraryItems|modsLibraryData|ModPosterCard/);
  assert.match(css, /\.mod-library-skeleton\.view-list/);
  assert.match(css, /\.mod-library-skeleton\.view-tech/);
});

test("empty library and no-match states remain semantically distinct", () => {
  assert.match(source, /kind: "library"/);
  assert.match(source, /\{copy\.emptyTitle\}/);
  assert.match(source, /kind: "matches"/);
  assert.match(source, /\{copy\.noMatchTitle\}/);
  assert.match(source, /\{copy\.clearFilters\}/);
  assert.match(copySource, /尚未导入 Mod/);
  assert.match(copySource, /没有匹配的 Mod/);
  assert.match(copySource, /清除条件/);
});

test("profile-dependent filters show an explicit blocked state instead of silently querying all", () => {
  assert.match(source, /ModLibraryQueryBlockedState/);
  assert.match(source, /\{copy\.filterUnavailableTitle\}/);
  assert.match(source, /\{copy\.viewAllMods\}/);
  assert.match(copySource, /当前筛选暂不可用/);
  assert.match(copySource, /查看全部 Mod/);
});

test("refresh keeps lightweight busy and retry feedback outside the card grid", () => {
  assert.match(source, /className="mod-library-query-progress"/);
  assert.match(source, /className="mod-library-query-error" role="alert"/);
  assert.match(source, /<ModLibraryControlTooltip content=\{copy\.retryQueryAria\} describeControl=\{false\}>/);
  assert.match(source, /aria-label=\{copy\.retryQueryAria\}/);
  assert.match(copySource, /重试 Mod 库查询/);
  assert.doesNotMatch(source, /\btitle=/);
  assert.match(css, /\.mod-library-query-error__message/);
  assert.match(css, /\.mod-library-query-error[\s\S]*?> \.mod-library-control-tooltip[\s\S]*?inset-block-start:\s*calc\(100% \+ 8px\);/);
  assert.match(css, /\.mod-library-query-error[\s\S]*?> \.mod-library-control-tooltip[\s\S]*?inset-inline-end:\s*0;/);
  assert.doesNotMatch(source, /className="mod-card/);
});

test("query feedback uses semantic surfaces and disables motion when requested", () => {
  assert.doesNotMatch(css, /position:\s*fixed/);
  assert.match(css, /background:\s*var\(--color-surface\)/);
  assert.match(css, /@media \(prefers-reduced-motion: reduce\)/);
  assert.match(css, /animation:\s*none/);
});
