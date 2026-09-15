import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

// 接线检查只覆盖真实 Provider 行为测试无法覆盖的边界；交互时序由行为测试验证。
const read = (path) => readFileSync(path, "utf8");
const provider = () => read("src/features/mods/ModImportDropProvider.tsx");

test("native drop remains a single window-level Tauri subscription", () => {
  const source = provider();
  assert.match(source, /getCurrentWebview\(\)\.onDragDropEvent\(/);
  assert.match(source, /handleDroppedPaths\(payload\.paths\)/);
  assert.doesNotMatch(source, /addEventListener\(\s*"(dragover|dragenter|dragleave|drop)"/);
  assert.doesNotMatch(source, /dataTransfer/);
});

test("the drop provider owns task lifetime above the routing outlet", () => {
  const app = read("src/App.tsx");
  const host = app.indexOf("<ModImportDropProvider>");
  const outlet = app.indexOf("<RouterOutlet />");
  assert.ok(host >= 0 && outlet > host);
  for (const path of ["src/features/mods/ModLibraryPage.tsx", "src/features/mods/CompactActionPanel.tsx"]) {
    assert.doesNotMatch(read(path), /<ModImportDropProvider|<ModImportDropOverlay/);
  }
});

test("archive suitability remains a backend preview decision", () => {
  const source = provider();
  assert.match(source, /previewDroppedModArchives\(request\.paths\)/);
  assert.doesNotMatch(source, /\.(endsWith|toLowerCase)\(\)?[\s\S]{0,40}"\.?(zip|rar|7z)"/);
  assert.match(source, /unique\.length > MAX_DROPPED_ARCHIVES/);
  assert.match(source, /tooMany\(unique\.length, MAX_DROPPED_ARCHIVES\)/);
  // 单次超限拒绝和大批次分段重试由真实 Provider 行为测试验证，不能把 slice 一律当成丢弃。
});

test("drop overlay reuses modal focus and motion without discarding on backdrop clicks", () => {
  const source = read("src/features/mods/ModImportDropOverlay.tsx");
  assert.match(source, /<ModalSurface/);
  assert.match(source, /closeOnBackdrop=\{false\}/);
  assert.doesNotMatch(source, /busy=\{/);
});

test("import tasks have a fixed reopen entry independent of the notification", () => {
  assert.match(read("src/features/mods/CompactActionPanel.tsx"), /<ModImportDropAction/);
  const action = read("src/features/mods/ModImportDropAction.tsx");
  assert.match(action, /useModImportDrop\(\)/);
  assert.match(action, /onClick=\{openDropList\}/);
});

test("full row explanations wrap below the filename", () => {
  const css = read("src/features/mods/ModImportDropOverlay.css");
  const start = css.indexOf(".mod-import-drop__row-note {");
  assert.ok(start >= 0);
  const rule = css.slice(start, css.indexOf("}", start));
  assert.doesNotMatch(rule, /white-space:\s*nowrap|text-overflow:\s*ellipsis/);
  assert.match(read("src/features/mods/ModImportDropRow.tsx"), /<p className="mod-import-drop__row-note">\{note\}<\/p>/);
});
