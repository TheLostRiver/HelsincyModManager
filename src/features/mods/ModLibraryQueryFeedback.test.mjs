import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const source = readFileSync("src/features/mods/ModLibraryQueryFeedback.tsx", "utf8");
const css = readFileSync("src/features/mods/ModLibraryQueryFeedback.css", "utf8");

test("initial loading uses finite view-aware skeletons instead of mock cards", () => {
  assert.match(source, /viewMode === "classic" \|\| viewMode === "grid" \? 8 : 6/);
  assert.match(source, /aria-label="正在加载 Mod 库"/);
  assert.doesNotMatch(source, /fallbackModLibraryItems|modsLibraryData|ModPosterCard/);
  assert.match(css, /\.mod-library-skeleton\.view-list/);
  assert.match(css, /\.mod-library-skeleton\.view-tech/);
});

test("empty library and no-match states remain semantically distinct", () => {
  assert.match(source, /kind: "library"/);
  assert.match(source, /尚未导入 Mod/);
  assert.match(source, /kind: "matches"/);
  assert.match(source, /没有匹配的 Mod/);
  assert.match(source, /清除条件/);
});

test("profile-dependent filters show an explicit blocked state instead of silently querying all", () => {
  assert.match(source, /ModLibraryQueryBlockedState/);
  assert.match(source, /当前筛选暂不可用/);
  assert.match(source, /查看全部 Mod/);
});

test("refresh keeps lightweight busy and retry feedback outside the card grid", () => {
  assert.match(source, /className="mod-library-query-progress"/);
  assert.match(source, /className="mod-library-query-error" role="alert"/);
  assert.match(source, /<ModLibraryControlTooltip content="重试 Mod 库查询" describeControl=\{false\}>/);
  assert.match(source, /aria-label="重试 Mod 库查询"/);
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
