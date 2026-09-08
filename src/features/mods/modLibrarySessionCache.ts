// 库页会话缓存的纯模型。
//
// # 为什么需要它
//
// `RouterOutlet` 会**卸载**页面组件（App.tsx 里那几条注释都在说同一件事），所以
// `useModLibraryQuery` 的 state 活不过一次切页：玩家每回到 Mod 库都要从零重新查一遍
// 库、拉一遍安装清单状态、扫一遍恢复状态，界面先给一屏骨架屏。
//
// # 语义：stale-while-revalidate，不是「省掉请求」
//
// 命中缓存**不取消**这次请求，只是先把上次的结果摆出来，等新结果回来再无声替换。
// 这条边界很重要：一个装完 Mod 却还显示旧列表的库，比多等一会儿糟得多。缓存只负责
// 「先给你看」，正确性仍然由每次挂载都发生的重新校验保证；最坏情况是看到一瞬间的旧数据，
// 而这段时间原本是空白骨架屏——两者都不是最新事实，但后者连自己的库都看不见。
//
// # 淘汰口径：最近写入优先，而不是最近读取优先
//
// 每次读命中后面**必定**跟着一次同 key 的重新校验写入（见上），所以「最近写入」与
// 「最近使用」在这里是同一个顺序，不必为了 LRU 再让读操作返回新缓存。

import type { CategoryItem } from "./modCategoryApi";
import type { ModLibraryPage } from "./modLibraryTypes";

/**
 * 保留多少个分页槽位。
 *
 * 分页与搜索都会产生不同的 key（搜索经 250ms 防抖，一次停顿一个 key），不设上限的话
 * 会随会话单调增长。8 足够覆盖「翻几页再切回来」，而一页最多 96 条轻量条目，
 * 不含缩略图字节（走自定义协议 URL），整体开销可以忽略。
 */
export const MOD_LIBRARY_PAGE_CACHE_LIMIT = 8;

export type ModLibraryPageCacheEntry = {
  profileKey: string;
  queryKey: string;
  page: ModLibraryPage;
};

export type ModLibrarySessionCache = {
  /** 最近写入的在前；超出 {@link MOD_LIBRARY_PAGE_CACHE_LIMIT} 的从尾部丢弃。 */
  pages: readonly ModLibraryPageCacheEntry[];
  /** 分类没有 profile 作用域（后端 `listCategories` 不吃参数），单槽位即可。 */
  categories: readonly CategoryItem[] | null;
};

export const EMPTY_MOD_LIBRARY_SESSION_CACHE: ModLibrarySessionCache = {
  pages: [],
  categories: null,
};

function isSameSlot(entry: ModLibraryPageCacheEntry, profileKey: string, queryKey: string) {
  return entry.profileKey === profileKey && entry.queryKey === queryKey;
}

/**
 * 取某个 (配置档, 查询) 槽位上次提交的结果；没有就是 null。
 *
 * profileKey 是 key 的一部分而不是过滤条件——两个配置档的同名查询是不同的事实，
 * 混用会让玩家在切档后看到另一个档的安装状态。
 */
export function readCachedLibraryPage(
  cache: ModLibrarySessionCache,
  profileKey: string,
  queryKey: string,
): ModLibraryPage | null {
  return cache.pages.find((entry) => isSameSlot(entry, profileKey, queryKey))?.page ?? null;
}

/**
 * 写入一次成功查询的结果。
 *
 * 存的是 `page` 的引用而不是深拷贝：调用方（`useModLibraryQuery`）从不就地改动它，
 * 改动一律走「造一个新 page 对象」。所以引用共享不会让缓存里的快照被悄悄改写。
 */
export function writeCachedLibraryPage(
  cache: ModLibrarySessionCache,
  profileKey: string,
  queryKey: string,
  page: ModLibraryPage,
): ModLibrarySessionCache {
  const others = cache.pages.filter((entry) => !isSameSlot(entry, profileKey, queryKey));
  return {
    ...cache,
    pages: [{ profileKey, queryKey, page }, ...others].slice(0, MOD_LIBRARY_PAGE_CACHE_LIMIT),
  };
}

/**
 * 丢掉一个槽位。
 *
 * 用在「页面已经知道自己手里这份不可信」的时候（例如安装终态校验失败后的
 * fail-closed 标记）。这种时候宁可让下次进页面重新查一遍，也不能把一份**已知有问题**的
 * 快照留着复用——缓存失效的方向必须倒向重新取数。
 */
export function invalidateCachedLibraryPage(
  cache: ModLibrarySessionCache,
  profileKey: string,
  queryKey: string,
): ModLibrarySessionCache {
  const pages = cache.pages.filter((entry) => !isSameSlot(entry, profileKey, queryKey));
  return pages.length === cache.pages.length ? cache : { ...cache, pages };
}

/**
 * 丢掉全部分页槽位。
 *
 * 用在「库本身变了，但本页没经手」的时候：在别的页面拖拽导入完成、或者写任务开跑之后
 * 玩家切走了。这时候**没法逐槽位判断**——新导入的 Mod 可能命中任何筛选、任何搜索词、
 * 落在任何一页，要判断得先知道它长什么样，而我们恰恰不知道。
 *
 * 一份「看起来完整、却少了刚导入那个」的列表比骨架屏糟得多：前者读作「导入失败了」。
 * 所以宁可整份丢掉重查。
 *
 * 分类不受影响：分类是玩家自己建的标签，导入不会凭空造出新分类。
 */
export function invalidateAllCachedLibraryPages(
  cache: ModLibrarySessionCache,
): ModLibrarySessionCache {
  return cache.pages.length === 0 ? cache : { ...cache, pages: [] };
}

export function readCachedCategories(
  cache: ModLibrarySessionCache,
): readonly CategoryItem[] | null {
  return cache.categories;
}

export function writeCachedCategories(
  cache: ModLibrarySessionCache,
  categories: readonly CategoryItem[],
): ModLibrarySessionCache {
  return { ...cache, categories: [...categories] };
}
