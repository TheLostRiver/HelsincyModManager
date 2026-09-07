import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

// 拖拽导入里**逻辑**部分的断言在 modImportDropState / modImportDropRunner 的测试里，
// 那些是真的跑起来的行为断言。这个文件只钉住几条行为测试够不着的接线不变量
// ——都是「一旦改错，功能会以很难看出来的方式失效」的那种。

function readSource(path) {
  return readFileSync(path, "utf8");
}

test("拖放只订阅 Tauri 的窗口级事件：只有它给得出真实文件路径", () => {
  const source = readSource("src/features/mods/ModImportDropZone.tsx");

  assert.match(source, /getCurrentWebview\(\)\s*\n?\s*\.onDragDropEvent\(/);
  for (const kind of ['"enter"', '"over"', '"leave"']) {
    assert.match(source, new RegExp(`payload\\.type === ${kind}`), `未处理 ${kind}`);
  }
  // drop 是 else 分支（enter/over/leave 之外），所以钉住它取的是 payload.paths。
  assert.match(source, /handleDroppedPaths\(payload\.paths\)/);

  // 浏览器的 dragover/drop 在 webview 里给不出可用路径。用了就是拿不到文件的空功能。
  assert.doesNotMatch(source, /addEventListener\(\s*"(dragover|dragenter|dragleave|drop)"/);
  assert.doesNotMatch(source, /dataTransfer/);
});

test("能不能导入由后端判定，前端不自己维护一份扩展名表", () => {
  // hmm-structural-rules-not-vocabularies：前端再写一份「什么算压缩包」，
  // 两处判定迟早会漂，而漂的那一天玩家看到的是「明明能导入却说读不了」。
  const source = readSource("src/features/mods/ModImportDropZone.tsx");

  assert.match(source, /previewDroppedModArchives\(unique\)/);
  assert.match(source, /dropRowsFromPreviews\(previews\)/);
  assert.doesNotMatch(source, /\.(endsWith|toLowerCase)\(\)?[\s\S]{0,40}"\.?(zip|rar|7z)"/);
});

test("拖进来先出清单，确认之后才导入", () => {
  const source = readSource("src/features/mods/ModImportDropZone.tsx");

  // drop 处理里不能直接起导入任务——那样批量拖十几个文件是不可撤销的。
  const dropHandler = source.slice(
    source.indexOf("const handleDroppedPaths"),
    source.indexOf("// Tauri 的拖放事件是"),
  );
  assert.ok(dropHandler.length > 0, "找不到 drop 处理段落，测试锚点失效了");
  assert.doesNotMatch(dropHandler, /startImportModTask|runDropImportBatch/);
  assert.match(source, /onClick=\{\(\) => void confirmImport\(\)\}/);
  assert.match(source, /runDropImportBatch\(rows, \{/);
});

test("进度订阅没建起来时不允许确认：否则整批会等一个永远不来的终态", () => {
  const source = readSource("src/features/mods/ModImportDropZone.tsx");
  assert.match(source, /disabled=\{!canStartDropImport\(rows\) \|\| !listenerReady\}/);
  assert.match(source, /listState\.status !== "ready" \|\| !canStartDropImport\(rows\) \|\| !listenerReady/);
});

test("被挡住的行不可勾选，且带上与导入失败同一套的三语文案", () => {
  const source = readSource("src/features/mods/ModImportDropZone.tsx");
  assert.match(source, /disabled=\{row\.status === "blocked" \|\| importing\}/);
  assert.match(source, /getDropRowBlockedMessage\(row, copy\)/);
});

test("整批跑完只刷新一次库", () => {
  // 每导完一个刷一次，会在批量导入时反复重排列表，玩家的滚动位置一直被打断。
  const source = readSource("src/features/mods/ModImportDropZone.tsx");
  const afterBatch = source.slice(source.indexOf("await runDropImportBatch"));
  assert.equal(
    (afterBatch.match(/onImported\(\)/g) ?? []).length,
    1,
    "onImported 应当只在整批之后调用一次",
  );
});

test("拖拽区只挂一处：窗口级事件挂两处会把同一次拖拽处理两遍", () => {
  const mounts = readSource("src/features/mods/ModLibraryPage.tsx").match(
    /<ModImportDropZone\b/g,
  );
  assert.deepEqual(mounts?.length, 1);

  // 其余文件不得再挂。ModImportDropZone.tsx 自身是定义处，不算挂载。
  for (const path of [
    "src/features/mods/CompactActionPanel.tsx",
    "src/app/AppShell.tsx",
    "src/main.tsx",
  ]) {
    let source;
    try {
      source = readSource(path);
    } catch {
      continue;
    }
    assert.doesNotMatch(source, /<ModImportDropZone\b/, `${path} 不应重复挂载拖拽区`);
  }
});

test("清单开着时不收新的拖拽，也不给能放的暗示", () => {
  // 直接替换会把玩家刚勾好的选择悄悄丢掉；只挡住 drop 而照样高亮，则是白拖一次。
  const source = readSource("src/features/mods/ModImportDropZone.tsx");
  assert.match(source, /dropsBlockedRef\.current = listState\.status !== "idle"/);
  assert.match(source, /if \(!dropsBlockedRef\.current\) setDragActive\(true\)/);
  assert.match(source, /if \(dropsBlockedRef\.current\) return;\s+void handleDroppedPaths/);
});

test("存储写入被冻结时，拖进来只说原因，不开清单", () => {
  const zone = readSource("src/features/mods/ModImportDropZone.tsx");
  const page = readSource("src/features/mods/ModLibraryPage.tsx");

  assert.match(zone, /const reason = disabledReasonRef\.current;/);
  assert.match(zone, /if \(reason\) \{[\s\S]{0,200}pushToast\(/);
  assert.match(page, /disabledReason=\{storageWriteFreezeReason \?\? null\}/);
});
