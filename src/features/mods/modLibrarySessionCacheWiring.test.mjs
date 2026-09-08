// 会话缓存的**接线**判据。
//
// 缓存本身的取舍是纯函数，行为判据在 modLibrarySessionCache.test.mjs 与
// modLibraryQueryState.test.mjs（`resolveQueryStartExecutionState`）。这里剩下的
// 是 React 接线：Provider 挂在哪、缓存怎么被取用、命中之后请求还发不发。这些没有
// 渲染器就只能读源码断言——所以本文件只钉**结构**，不冒充行为验证。

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const appSource = readFileSync("src/App.tsx", "utf8");
const hookSource = readFileSync("src/features/mods/useModLibraryQuery.ts", "utf8");
const pageSource = readFileSync("src/features/mods/ModLibraryPage.tsx", "utf8");
const providerSource = readFileSync(
  "src/features/mods/ModLibrarySessionCacheProvider.tsx",
  "utf8",
);
const dropProviderSource = readFileSync(
  "src/features/mods/ModImportDropProvider.tsx",
  "utf8",
);

test("缓存挂在 RouterOutlet 之上，否则它跟着页面一起被卸载，等于不存在", () => {
  const provider = appSource.indexOf("<ModLibrarySessionCacheProvider>");
  const outlet = appSource.indexOf("<RouterOutlet />");
  const providerClose = appSource.indexOf("</ModLibrarySessionCacheProvider>");

  assert.notEqual(provider, -1, "App 里必须挂上缓存 Provider");
  assert.ok(provider < outlet, "Provider 必须在 RouterOutlet 之外");
  assert.ok(outlet < providerClose, "RouterOutlet 必须被它包住");
});

test("缓存 Provider 在拖拽导入之外，否则导入完成时够不到它", () => {
  // 后台导入完成必须能作废缓存。嵌套反了就只能靠库页转发，而库页此刻正是卸载的。
  const cacheProvider = appSource.indexOf("<ModLibrarySessionCacheProvider>");
  const dropProvider = appSource.indexOf("<ModImportDropProvider>");

  assert.notEqual(dropProvider, -1);
  assert.ok(cacheProvider < dropProvider, "缓存 Provider 必须包住拖拽 Provider");
});

test("后台导入成功要作废整份分页缓存", () => {
  // 这是「在别的页面拖拽导入，再切进 Mod 库」那条路径的唯一保护：库页是卸载的，
  // libraryRevision 那个计数订阅没人听，只有这里能让下次进页面重新查。
  const onSettled = dropProviderSource.slice(
    dropProviderSource.indexOf("onSettled: (archivePath, outcome) => {"),
    dropProviderSource.indexOf("}).finally(() => {"),
  );
  assert.ok(onSettled.length > 0, "没能定位到 onSettled");
  assert.match(onSettled, /librarySessionCache\.invalidateAllPages\(\)/);

  // 且只在成功时作废：失败的包没进库，凭什么让玩家多等一次全量重查。
  const successBranch = onSettled.slice(onSettled.indexOf("if (outcome === \"succeeded\")"));
  assert.match(successBranch, /librarySessionCache\.invalidateAllPages\(\)/);
});

test("写任务一开跑就作废分页缓存——玩家可能不等它结束就切走", () => {
  // 写完成时的刷新会把缓存填回来，但前提是页面还挂着。点了安装就切走的话，
  // 那次刷新会被 request gate 挡下，缓存里留着的是「装之前」的状态。
  assert.match(
    pageSource,
    /const libraryWriteInFlight =[\s\S]{0,400}?managedInstallTaskActive[\s\S]{0,400}?reinstallWorkflow\.taskActive[\s\S]{0,400}?deletionBusy[\s\S]{0,400}?batchWorkflow\.state\.status === "starting"/,
  );
  assert.match(
    pageSource,
    /if \(!libraryWriteInFlight\) return;\s*\n\s*librarySessionCache\.invalidateAllPages\(\);/,
  );
});

test("写判据取的是「写真的在跑」，不是「面板开着」——否则开个预览就白清缓存", () => {
  // reinstall 的 workflowActive 只表示预览弹窗开着（读），batch 的非 idle 也包含预览。
  // 用它们当判据会让「打开预览再关掉」白付一次全量重查。
  const predicate = pageSource.slice(
    pageSource.indexOf("const libraryWriteInFlight ="),
    pageSource.indexOf("if (!libraryWriteInFlight) return;"),
  );
  assert.ok(predicate.length > 0, "没能定位到写判据");
  assert.doesNotMatch(predicate, /workflowActive/, "重装要用 taskActive");
  assert.doesNotMatch(predicate, /status !== "idle"/, "批量要用 starting");
});

test("缓存放 ref 不放 state：写缓存不得引起重渲染", () => {
  // 放 state 的话，每次查询成功写一次缓存就多一轮全应用重渲染——
  // 为了少一屏骨架屏而让整棵树多渲染一遍，是净亏。
  assert.match(providerSource, /useRef<ModLibrarySessionCache>/);
  assert.doesNotMatch(providerSource, /useState/);
});

test("库页把缓存传给查询 hook", () => {
  assert.match(pageSource, /const librarySessionCache = useModLibrarySessionCache\(\)/);
  assert.match(
    pageSource,
    /useModLibraryQuery\(\{[\s\S]*?cache: librarySessionCache,[\s\S]*?\}\)/,
  );
});

test("分类也从缓存起步，并在拉到之后写回", () => {
  assert.match(pageSource, /useState<CategoryItem\[\]>\(\s*\(\) => \[\.\.\.\(librarySessionCache\.readCategories\(\) \?\? \[\]\)\],?\s*\)/);
  assert.match(pageSource, /librarySessionCache\.writeCategories\(loadedCategories\)/);
});

test("缓存经 ref 取用；只有同步 ref 那个 effect 可以依赖 cache", () => {
  // 调用方若传了个每渲染都新建的对象，而它又进了**驱动请求**的依赖里，就会变成
  // 「每渲染一次重发一次请求」——缓存这个可选优化不该有能力把主查询拖垮。
  // 同步 ref 那个 effect 依赖 cache 是它的本职，不在此列。
  assert.match(hookSource, /const cacheRef = useRef\(cache\)/);

  const arraysMentioningCache = (hookSource.match(/\}, \[[^\]]*\]\)/g) ?? []).filter(
    (dependencyArray) => /\bcache\b/.test(dependencyArray),
  );
  assert.deepEqual(
    arraysMentioningCache,
    ["}, [cache])"],
    "除了同步 ref 的那个 effect，任何依赖数组都不该出现 cache",
  );

  // 驱动请求的两个依赖数组逐字钉住：掺进任何额外依赖都会改变发请求的时机。
  assert.match(hookSource, /\}, \[profileKey, queryInput, queryKey\]\);/);
  assert.match(hookSource, /\}, \[executeQuery, queryKey\]\);/);
  assert.match(hookSource, /\n {4}\[loadPage\],\n {2}\);/);
});

test("缓存 ref 在 layout effect 里同步，不在 render 期间赋值", () => {
  // render 期间写 ref 违反 React 约定（唯一例外是惰性初始化）：render 可能被丢弃或重跑。
  // 今天没有并发特性、写的又是恒定对象，所以看不出差别——但那是碰巧无害。
  assert.match(
    hookSource,
    /useLayoutEffect\(\(\) => \{\s*\n\s*cacheRef\.current = cache;\s*\n\s*\}, \[cache\]\);/,
  );
  // 组件体里的直接赋值是两格缩进；effect 里的是四格。用缩进区分这两种形态。
  assert.doesNotMatch(
    hookSource,
    /\n {2}cacheRef\.current = cache;/,
    "不得在 render 期间直接赋值",
  );
});

test("命中缓存不取消请求——发请求那段完全不知道缓存的存在", () => {
  // 这是整个设计里最容易被「顺手优化掉」的一条：一旦命中就 return，库页就会显示
  // 一份永不更新的旧快照，而玩家刚装完的 Mod 不会出现在里面。
  const requestEffect = hookSource.slice(
    hookSource.indexOf("if (skippedCommittedQueryEffectKeyRef.current === queryKey)"),
    hookSource.indexOf("}, [executeQuery, queryKey]);"),
  );
  assert.ok(requestEffect.length > 0, "没能定位到发请求的 effect");
  assert.doesNotMatch(requestEffect, /cache/i, "发请求的判断不得掺入缓存");

  // 读缓存那一句之后，同一段里不得出现提前 return。
  const readIndex = hookSource.indexOf("cacheRef.current?.readPage(profileKey, queryKey)");
  assert.notEqual(readIndex, -1);
  const afterRead = hookSource.slice(readIndex, hookSource.indexOf("}, [profileKey, queryInput, queryKey]);"));
  assert.doesNotMatch(afterRead, /\breturn\b/, "读到缓存之后不得提前返回");
});

test("查询成功要写回缓存，两条分支都不能漏", () => {
  // 漏掉夹紧那条分支，越界页码的结果就永远进不了缓存。
  assert.equal(hookSource.match(/cacheRef\.current\?\.writePage\(/g)?.length, 2);
  assert.match(
    hookSource,
    /cacheRef\.current\?\.writePage\(request\.profileKey, skippedClampQueryKeyRef\.current, page\)/,
  );
  assert.match(
    hookSource,
    /cacheRef\.current\?\.writePage\(request\.profileKey, request\.queryKey, page\)/,
  );
});

test("fail-closed 标记要把缓存槽位丢掉，而不是留着复用", () => {
  // 留着等于下次进页面把一份**已知有问题**的安装状态又摆回来。
  const updateFn = hookSource.slice(
    hookSource.indexOf("const updateCurrentPageItems = useCallback("),
    hookSource.indexOf("const page = executionState.record?.profileKey === profileKey"),
  );
  assert.ok(updateFn.length > 0, "没能定位到 updateCurrentPageItems");
  assert.match(updateFn, /cacheRef\.current\?\.invalidatePage\(profileKey, committedQueryKey\)/);
});
