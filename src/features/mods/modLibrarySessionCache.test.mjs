import assert from "node:assert/strict";
import test from "node:test";

import {
  EMPTY_MOD_LIBRARY_SESSION_CACHE,
  MOD_LIBRARY_PAGE_CACHE_LIMIT,
  invalidateCachedLibraryPage,
  readCachedCategories,
  readCachedLibraryPage,
  writeCachedCategories,
  writeCachedLibraryPage,
} from "./modLibrarySessionCache.ts";

const pageOf = (...names) => ({
  items: names.map((name) => ({
    id: name,
    name,
    sizeLabel: "1 MB",
    status: "not_installed",
    categoryLabels: [],
  })),
  page: 1,
  pageSize: 24,
  libraryTotal: names.length,
  matchingTotal: names.length,
});

test("空缓存读不出东西，而不是抛错或返回空页", () => {
  assert.equal(readCachedLibraryPage(EMPTY_MOD_LIBRARY_SESSION_CACHE, "p1", "q1"), null);
  assert.equal(readCachedCategories(EMPTY_MOD_LIBRARY_SESSION_CACHE), null);
});

test("写进去的那一份能按同一个 (配置档, 查询) 原样取回", () => {
  const page = pageOf("a", "b");
  const cache = writeCachedLibraryPage(EMPTY_MOD_LIBRARY_SESSION_CACHE, "p1", "q1", page);
  assert.equal(readCachedLibraryPage(cache, "p1", "q1"), page);
});

test("配置档不同就是不同的事实——绝不跨档命中", () => {
  // 跨档命中会让玩家切档后看到另一个档的安装状态，而安装状态正是这一页的主信息。
  const cache = writeCachedLibraryPage(EMPTY_MOD_LIBRARY_SESSION_CACHE, "p1", "q1", pageOf("a"));
  assert.equal(readCachedLibraryPage(cache, "p2", "q1"), null);
});

test("查询不同就是不同的槽位——搜索词/页码/筛选都编在 key 里", () => {
  const cache = writeCachedLibraryPage(EMPTY_MOD_LIBRARY_SESSION_CACHE, "p1", "q1", pageOf("a"));
  assert.equal(readCachedLibraryPage(cache, "p1", "q2"), null);
});

test("同一个槽位重复写只留最新的一份，不会堆叠", () => {
  const stale = pageOf("old");
  const fresh = pageOf("new");
  let cache = writeCachedLibraryPage(EMPTY_MOD_LIBRARY_SESSION_CACHE, "p1", "q1", stale);
  cache = writeCachedLibraryPage(cache, "p1", "q1", fresh);

  assert.equal(readCachedLibraryPage(cache, "p1", "q1"), fresh);
  assert.equal(cache.pages.length, 1, "重新校验写回不该让槽位数增长");
});

test("写操作不改动传进来的缓存对象", () => {
  // 缓存挂在 Provider 的 ref 上，就地改会让「读到的是哪一版」变得不可推理。
  const before = writeCachedLibraryPage(EMPTY_MOD_LIBRARY_SESSION_CACHE, "p1", "q1", pageOf("a"));
  const snapshot = before.pages;
  const after = writeCachedLibraryPage(before, "p1", "q2", pageOf("b"));

  assert.equal(before.pages, snapshot, "旧缓存的数组引用不该被换掉");
  assert.equal(before.pages.length, 1);
  assert.equal(after.pages.length, 2);
  assert.equal(readCachedLibraryPage(before, "p1", "q2"), null);
});

test("槽位有上限，超出的按最久未写入丢弃", () => {
  // 搜索每停顿一次就是一个新 key，不设上限会随会话单调增长。
  let cache = EMPTY_MOD_LIBRARY_SESSION_CACHE;
  const total = MOD_LIBRARY_PAGE_CACHE_LIMIT + 3;
  for (let index = 0; index < total; index += 1) {
    cache = writeCachedLibraryPage(cache, "p1", `q${index}`, pageOf(`item-${index}`));
  }

  assert.equal(cache.pages.length, MOD_LIBRARY_PAGE_CACHE_LIMIT);
  assert.equal(
    readCachedLibraryPage(cache, "p1", `q${total - 1}`),
    cache.pages[0].page,
    "最后写的必须还在",
  );
  assert.equal(readCachedLibraryPage(cache, "p1", "q0"), null, "最早写的已被淘汰");
});

test("重新写一个已在缓存里的槽位会把它挪到最前，不占用新名额", () => {
  let cache = EMPTY_MOD_LIBRARY_SESSION_CACHE;
  for (let index = 0; index < MOD_LIBRARY_PAGE_CACHE_LIMIT; index += 1) {
    cache = writeCachedLibraryPage(cache, "p1", `q${index}`, pageOf(`item-${index}`));
  }
  // q0 是当前最老的一个；重新校验它之后它不该在下一次写入时被淘汰。
  cache = writeCachedLibraryPage(cache, "p1", "q0", pageOf("refreshed"));
  cache = writeCachedLibraryPage(cache, "p1", "extra", pageOf("extra"));

  assert.equal(cache.pages.length, MOD_LIBRARY_PAGE_CACHE_LIMIT);
  assert.notEqual(readCachedLibraryPage(cache, "p1", "q0"), null, "刚重新校验过的不该被淘汰");
  assert.equal(readCachedLibraryPage(cache, "p1", "q1"), null, "被淘汰的应该是真正最老的那个");
});

test("失效只丢指定槽位，别的槽位不受影响", () => {
  let cache = writeCachedLibraryPage(EMPTY_MOD_LIBRARY_SESSION_CACHE, "p1", "q1", pageOf("a"));
  cache = writeCachedLibraryPage(cache, "p1", "q2", pageOf("b"));

  const invalidated = invalidateCachedLibraryPage(cache, "p1", "q1");
  assert.equal(readCachedLibraryPage(invalidated, "p1", "q1"), null);
  assert.notEqual(readCachedLibraryPage(invalidated, "p1", "q2"), null);
});

test("失效一个不存在的槽位是无操作，返回同一个缓存", () => {
  const cache = writeCachedLibraryPage(EMPTY_MOD_LIBRARY_SESSION_CACHE, "p1", "q1", pageOf("a"));
  assert.equal(invalidateCachedLibraryPage(cache, "p1", "nope"), cache);
});

test("失效不碰分类槽位", () => {
  let cache = writeCachedCategories(EMPTY_MOD_LIBRARY_SESSION_CACHE, [{ id: "c1", name: "武器" }]);
  cache = writeCachedLibraryPage(cache, "p1", "q1", pageOf("a"));
  const invalidated = invalidateCachedLibraryPage(cache, "p1", "q1");

  assert.deepEqual(readCachedCategories(invalidated), [{ id: "c1", name: "武器" }]);
});

test("分类缓存存的是副本，调用方之后改自己的数组不会改到缓存", () => {
  const source = [{ id: "c1", name: "武器" }];
  const cache = writeCachedCategories(EMPTY_MOD_LIBRARY_SESSION_CACHE, source);
  source.push({ id: "c2", name: "防具" });

  assert.equal(readCachedCategories(cache).length, 1);
});

test("分类写入不会连带清空分页槽位", () => {
  let cache = writeCachedLibraryPage(EMPTY_MOD_LIBRARY_SESSION_CACHE, "p1", "q1", pageOf("a"));
  cache = writeCachedCategories(cache, [{ id: "c1", name: "武器" }]);

  assert.notEqual(readCachedLibraryPage(cache, "p1", "q1"), null);
});
