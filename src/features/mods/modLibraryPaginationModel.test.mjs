import assert from "node:assert/strict";
import { test } from "node:test";

import {
  DEFAULT_MOD_LIBRARY_PAGE_SIZE,
  MOD_LIBRARY_PAGE_SIZES,
  getModLibraryEllipsisTarget,
  getModLibraryItemRange,
  getModLibraryPageSlots,
  getModLibraryTotalPages,
  isModLibraryPageSize,
} from "./modLibraryPaginationModel.ts";

test("exposes the fixed page-size allowlist and default", () => {
  assert.deepEqual(MOD_LIBRARY_PAGE_SIZES, [12, 24, 48, 96]);
  assert.equal(DEFAULT_MOD_LIBRARY_PAGE_SIZE, 24);
  assert.equal(isModLibraryPageSize(12), true);
  assert.equal(isModLibraryPageSize(96), true);
  assert.equal(isModLibraryPageSize(0), false);
  assert.equal(isModLibraryPageSize(25), false);
});

test("calculates total pages without inventing a page for empty results", () => {
  assert.equal(getModLibraryTotalPages(0, 24), 0);
  assert.equal(getModLibraryTotalPages(1, 24), 1);
  assert.equal(getModLibraryTotalPages(24, 24), 1);
  assert.equal(getModLibraryTotalPages(25, 24), 2);
  assert.equal(getModLibraryTotalPages(96, 48), 2);
});

test("calculates inclusive 1-based item ranges and clamps the last page", () => {
  assert.deepEqual(getModLibraryItemRange(1, 24, 50), { start: 1, end: 24 });
  assert.deepEqual(getModLibraryItemRange(2, 24, 50), { start: 25, end: 48 });
  assert.deepEqual(getModLibraryItemRange(3, 24, 50), { start: 49, end: 50 });
  assert.deepEqual(getModLibraryItemRange(99, 24, 50), { start: 49, end: 50 });
});

test("normalizes non-finite pages before calculating ranges and page slots", () => {
  assert.deepEqual(getModLibraryItemRange(Number.NaN, 24, 50), { start: 1, end: 24 });
  assert.deepEqual(
    getModLibraryPageSlots(Number.NaN, 12),
    [1, 2, 3, 4, 5, "ellipsis", 12],
  );
});

test("returns the empty 1-based range when there are no matching items", () => {
  assert.deepEqual(getModLibraryItemRange(1, 24, 0), { start: 0, end: 0 });
});

test("renders every page when the total fits within seven slots", () => {
  assert.deepEqual(getModLibraryPageSlots(1, 0), []);
  assert.deepEqual(getModLibraryPageSlots(1, 1), [1]);
  assert.deepEqual(getModLibraryPageSlots(4, 7), [1, 2, 3, 4, 5, 6, 7]);
});

test("uses one trailing ellipsis near the start of a long page range", () => {
  assert.deepEqual(getModLibraryPageSlots(1, 12), [1, 2, 3, 4, 5, "ellipsis", 12]);
  assert.deepEqual(getModLibraryPageSlots(4, 12), [1, 2, 3, 4, 5, "ellipsis", 12]);
});

test("uses two ellipses around a middle page", () => {
  assert.deepEqual(getModLibraryPageSlots(6, 12), [1, "ellipsis", 5, 6, 7, "ellipsis", 12]);
});

test("uses one leading ellipsis near the end of a long page range", () => {
  assert.deepEqual(getModLibraryPageSlots(9, 12), [1, "ellipsis", 8, 9, 10, 11, 12]);
  assert.deepEqual(getModLibraryPageSlots(12, 12), [1, "ellipsis", 8, 9, 10, 11, 12]);
});

test("clamps an out-of-range current page before creating slots", () => {
  assert.deepEqual(getModLibraryPageSlots(99, 12), [1, "ellipsis", 8, 9, 10, 11, 12]);
  assert.deepEqual(getModLibraryPageSlots(-10, 12), [1, 2, 3, 4, 5, "ellipsis", 12]);
});

test("ellipsis target lands in the middle of the collapsed range", () => {
  // 头部折叠：[1, …, 16, 17, 18, 19, 20]，省略号覆盖 2..15，中点 8。
  const nearEnd = getModLibraryPageSlots(18, 20);
  assert.deepEqual(nearEnd, [1, "ellipsis", 16, 17, 18, 19, 20]);
  assert.equal(getModLibraryEllipsisTarget(nearEnd, 1), 8);

  // 尾部折叠：[1, 2, 3, 4, 5, …, 20]，省略号覆盖 6..19，中点 12。
  const nearStart = getModLibraryPageSlots(2, 20);
  assert.deepEqual(nearStart, [1, 2, 3, 4, 5, "ellipsis", 20]);
  assert.equal(getModLibraryEllipsisTarget(nearStart, 5), 12);

  // 两端折叠时各自取各自区间的中点，互不影响。
  const middle = getModLibraryPageSlots(10, 20);
  assert.deepEqual(middle, [1, "ellipsis", 9, 10, 11, "ellipsis", 20]);
  assert.equal(getModLibraryEllipsisTarget(middle, 1), 5);
  assert.equal(getModLibraryEllipsisTarget(middle, 5), 15);
});

test("ellipsis target fails closed when there is no real gap to jump into", () => {
  // 相邻槽位不是数字（越界）时不可跳转。
  assert.equal(getModLibraryEllipsisTarget(["ellipsis", 2], 0), null);
  assert.equal(getModLibraryEllipsisTarget([1, "ellipsis"], 1), null);
  assert.equal(getModLibraryEllipsisTarget([1, "ellipsis", "ellipsis", 5], 1), null);

  // 相邻页码之间没有空隙时不可跳转，避免出现点了却停在原地的按钮。
  assert.equal(getModLibraryEllipsisTarget([4, "ellipsis", 5], 1), null);
  assert.equal(getModLibraryEllipsisTarget([4, "ellipsis", 4], 1), null);

  // 七页以内不折叠，不应出现省略号。
  assert.deepEqual(getModLibraryPageSlots(3, 7), [1, 2, 3, 4, 5, 6, 7]);
});
