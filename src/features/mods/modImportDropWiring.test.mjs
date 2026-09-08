import assert from "node:assert/strict";
// 见文件末尾「setState updater 必须是纯的」——那条判据抓的是导致「一个包导入两份」的
// 那类写法，不是某一行具体代码。
import { readFileSync } from "node:fs";
import { test } from "node:test";

// 拖拽导入里**逻辑**部分的断言在 modImportDropState / modImportDropRunner 的测试里，
// 那些是真的跑起来的行为断言。这个文件只钉住几条行为测试够不着的接线不变量
// ——都是「一旦改错，功能会以很难看出来的方式失效」的那种。

function readSource(path) {
  return readFileSync(path, "utf8");
}

const provider = () => readSource("src/features/mods/ModImportDropProvider.tsx");
const overlay = () => readSource("src/features/mods/ModImportDropOverlay.tsx");

test("拖放只订阅 Tauri 的窗口级事件：只有它给得出真实文件路径", () => {
  const source = provider();

  assert.match(source, /getCurrentWebview\(\)\s*\n?\s*\.onDragDropEvent\(/);
  for (const kind of ['"enter"', '"over"', '"leave"']) {
    assert.match(source, new RegExp(`payload\\.type === ${kind}`), `未处理 ${kind}`);
  }
  assert.match(source, /handleDroppedPaths\(payload\.paths\)/);

  // 浏览器的 dragover/drop 在 webview 里给不出可用路径。用了就是拿不到文件的空功能。
  assert.doesNotMatch(source, /addEventListener\(\s*"(dragover|dragenter|dragleave|drop)"/);
  assert.doesNotMatch(source, /dataTransfer/);
});

test("队列必须活过路由切换：Provider 挂在 RouterOutlet 之上，不在页面里", () => {
  // 这是最贵的一条。挂在页面里的话，切页组件卸载、批量循环死在半路，
  // 队列剩下的**永远不会起而且不报错**——比界面卡住严重得多。
  const app = readSource("src/App.tsx");
  const providerIndex = app.indexOf("<ModImportDropProvider>");
  const outletIndex = app.indexOf("<RouterOutlet />");
  assert.ok(providerIndex > 0, "App.tsx 必须挂 ModImportDropProvider");
  assert.ok(outletIndex > 0);
  assert.ok(providerIndex < outletIndex, "Provider 必须包住 RouterOutlet，不能在它下面");

  // 页面里不得再挂一份：拖放事件是窗口级的，挂两处会把同一次拖拽处理两遍。
  for (const path of [
    "src/features/mods/ModLibraryPage.tsx",
    "src/features/mods/CompactActionPanel.tsx",
  ]) {
    assert.doesNotMatch(readSource(path), /<ModImportDropProvider|<ModImportDropOverlay/);
  }
});

test("浮层随时可关，导入不受影响", () => {
  // 首版 closeList 有 `if (busy) return;`，等于拖一批包就把整个 HMM 锁住几分钟。
  const source = overlay();
  assert.doesNotMatch(source, /if \(busy\)\s*return/);
  assert.doesNotMatch(source, /disabled=\{[^}]*summary\.active/, "关闭按钮不得因为在跑就禁用");
  assert.match(source, /onClick=\{onClose\}/);
  // Esc 也要能关。
  assert.match(source, /event\.key === "Escape"[\s\S]{0,40}onClose\(\)/);
});

test("导入进行中仍然接新的拖拽，并入同一份清单", () => {
  const source = provider();
  assert.match(source, /void handleDroppedPaths\(payload\.paths\)/);
  // 不得出现「在跑就不收」的守卫。
  assert.doesNotMatch(
    source,
    /if \(\s*(summary\.active|pumpRunningRef\.current)\s*\) return;\s*\n\s*void handleDroppedPaths/,
  );
  assert.match(source, /mergeDropRows\(current\.rows, previews\)/);
});

test("入队之后必须唤醒泵，否则新项没人跑", () => {
  const source = provider();
  assert.match(source, /queueRef\.current\.push\(\.\.\.queued\)/);

  // 顺序要紧：先入队再唤醒。反过来的话泵可能在队列还空的时候跑一圈就退出，
  // 新入队的项没人跑，而且不报错。
  const pushIndex = source.indexOf("queueRef.current.push(...queued)");
  const pumpIndex = source.indexOf("ensurePumpRunning();");
  assert.ok(pushIndex >= 0, "找不到入队");
  assert.ok(pumpIndex >= 0, "找不到唤醒泵");
  assert.ok(pushIndex < pumpIndex, "必须先入队再唤醒泵");

  // takeNext 取到 null 与清标志必须同一 tick，否则「刚跑空就入队」会丢项。
  assert.match(source, /if \(next === null\) pumpRunningRef\.current = false;/);
});

test("能不能导入由后端判定，前端不自己维护一份扩展名表", () => {
  // hmm-structural-rules-not-vocabularies：前端再写一份「什么算压缩包」，
  // 两处判定迟早会漂，而漂的那一天玩家看到的是「明明能导入却说读不了」。
  const source = provider();
  assert.match(source, /previewDroppedModArchives\(unique\)/);
  assert.doesNotMatch(source, /\.(endsWith|toLowerCase)\(\)?[\s\S]{0,40}"\.?(zip|rar|7z)"/);
});

test("拖进来先出清单，确认之后才导入", () => {
  const source = provider();
  const dropHandler = source.slice(
    source.indexOf("const handleDroppedPaths"),
    source.indexOf("// Tauri 的拖放事件是"),
  );
  assert.ok(dropHandler.length > 0, "找不到 drop 处理段落，测试锚点失效了");
  assert.doesNotMatch(dropHandler, /startImportModTask|ensurePumpRunning/);
  assert.match(source, /onConfirm=\{\(\) => \{/);
});

test("进度订阅没建起来时不允许确认：否则队列会等一个永远不来的终态", () => {
  assert.match(overlay(), /disabled=\{!canStartDropImport\(rows\) \|\| !listenerReady\}/);
});

test("读不了的行不可勾，判据走模型不在组件里重写一遍", () => {
  const source = overlay();
  assert.match(source, /disabled=\{!selectable\}/);
  assert.match(source, /const selectable = isDropRowSelectable\(row\)/);
  assert.doesNotMatch(source, /disabled=\{row\.status === "blocked"/);
});

test("超上限的拖拽整批拒绝并报出数量，不截断", () => {
  const source = provider();
  assert.match(source, /unique\.length > MAX_DROPPED_ARCHIVES/);
  assert.match(source, /tooMany\(unique\.length, MAX_DROPPED_ARCHIVES\)/);
  // 截断会长成 slice/splice。出现即视为静默丢弃。
  assert.doesNotMatch(source, /unique\.(slice|splice)\(/);
});

test("库页靠计数订阅刷新，回调穿不过路由", () => {
  const page = readSource("src/features/mods/ModLibraryPage.tsx");
  assert.match(page, /const \{ libraryRevision \} = useModImportDrop\(\)/);
  assert.match(page, /seenLibraryRevisionRef/);
  assert.match(provider(), /setLibraryRevision\(\(revision\) => revision \+ 1\)/);
});

test("关掉浮层之后进度和结果走任务通知，且能点回来", () => {
  const source = provider();
  assert.match(source, /showTaskNotice\(\{/);
  assert.match(source, /!visible && \(list\.rows\.length > 0 \|\| list\.checking > 0\)/);
  assert.match(source, /openDropList/);
});

test("提示语必须整句显示：不得 nowrap、不得省略号", () => {
  // 读不到「为什么装不了」等于没有提示。首版把它写成右对齐 nowrap + ellipsis，
  // 整句中文被截成半句，玩家只能自己猜。
  const css = readSource("src/features/mods/ModImportDropOverlay.css");
  const start = css.indexOf(".mod-import-drop__row-note {");
  assert.ok(start > 0, "找不到 row-note 规则，测试锚点失效了");
  const note = css.slice(start, css.indexOf("}", start));
  assert.doesNotMatch(note, /white-space:\s*nowrap/);
  assert.doesNotMatch(note, /text-overflow:\s*ellipsis/);
  // 它必须是块级独占一行，而不是挤在文件名右边。
  assert.match(overlay(), /<p className="mod-import-drop__row-note">\{note\}<\/p>/);
});

test("存储写入被冻结时，拖进来只说原因，不开清单", () => {
  // #275。不在开清单之前挡的话，玩家会拖 20 个包、确认、然后眼看着 20 条一个个失败。
  const source = provider();
  assert.match(source, /getModStorageFreezeReason\(modStorage\.writesFrozen, locale\)/);
  assert.match(source, /const frozen = freezeReasonRef\.current;/);
  // 必须在预检之前 return，而不是先开清单再说。
  const guard = source.indexOf("const frozen = freezeReasonRef.current;");
  const preview = source.indexOf("previewDroppedModArchives(unique)");
  assert.ok(guard > 0 && preview > guard, "冻结守卫必须早于预检");
  assert.match(source, /if \(frozen\) \{[\s\S]{0,200}pushToast\(/);
});

// ---------------------------------------------------------------------------
// setState 的 updater 必须是纯函数。
//
// StrictMode 在开发模式下会**故意把 updater 调用两次**，就是为了暴露不纯的 updater。
// 曾经入队写在 `setList` 的 updater 里：
//
//     setList((current) => {
//       const { rows, queued } = confirmDropSelection(current.rows);
//       queueRef.current.push(...queued);   // ← 被执行两次
//       ...
//     });
//
// 于是同一个包被 push 两次，泵照队列起了两个导入任务，**库里出现两份一模一样的 Mod**。
// 所有既有判据都没抓到：纯模型测的是 confirmDropSelection，源码正则测的是接线形状，
// 没有一条会因为「updater 里多了个副作用」而转红。
// ---------------------------------------------------------------------------

/** 粗略但够用的括号配对：从 `setList(` 开始截到它自己的右括号。 */
function extractSetListCalls(text) {
  const calls = [];
  const marker = "setList(";
  let cursor = text.indexOf(marker);

  while (cursor >= 0) {
    let depth = 0;
    let index = cursor + marker.length - 1;
    for (; index < text.length; index += 1) {
      const character = text[index];
      if (character === "(") depth += 1;
      else if (character === ")") {
        depth -= 1;
        if (depth === 0) break;
      }
    }
    calls.push(text.slice(cursor, index + 1));
    cursor = text.indexOf(marker, index + 1);
  }

  return calls;
}

test("setList 的 updater 里不得出现副作用——StrictMode 会把它调两次", () => {
  const source = readFileSync("src/features/mods/ModImportDropProvider.tsx", "utf8");
  const calls = extractSetListCalls(source);
  assert.ok(calls.length >= 4, `没能定位到 setList 调用，只找到 ${calls.length} 处`);

  // 每一条都会被执行两次；两次都做等于做了两遍。
  const forbidden = [
    "queueRef.current.push",
    "ensurePumpRunning(",
    "setLibraryRevision(",
    "invalidateAllPages(",
    "pushToast(",
    "startImportModTask(",
    "showTaskNotice(",
  ];

  for (const call of calls) {
    for (const effect of forbidden) {
      assert.ok(
        !call.includes(effect),
        `setList 的 updater 里出现了副作用 ${effect}：\n${call.slice(0, 260)}`,
      );
    }
  }
});

test("确认导入从同一份快照做决定：入队与标记 queued 不得看两份不同的状态", () => {
  // 分开取快照的话，入队的路径和被标成「排队中」的行可能对不上——
  // 一个包进了队列却没在清单里显示，或者反过来。
  const source = readFileSync("src/features/mods/ModImportDropProvider.tsx", "utf8");

  assert.match(source, /const current = listRef\.current;\s*\n\s*const \{ rows, queued \} = confirmDropSelection\(current\.rows\);/);
  assert.match(source, /queueRef\.current\.push\(\.\.\.queued\);\s*\n\s*setList\(\{ \.\.\.current, rows \}\);/);
  // 空选择直接返回：否则会白唤醒一次泵，也会白写一次 state。
  assert.match(source, /if \(queued\.length === 0\) return;/);
});

test("清单镜像走 layout effect 同步，不在 render 期间赋值", () => {
  const source = readFileSync("src/features/mods/ModImportDropProvider.tsx", "utf8");
  assert.match(source, /useLayoutEffect\(\(\) => \{\s*\n\s*listRef\.current = list;\s*\n\s*\}, \[list\]\);/);
  assert.doesNotMatch(source, /\n {2}listRef\.current = list;/, "不得在 render 期间直接赋值");
});
