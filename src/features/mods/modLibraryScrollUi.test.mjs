import assert from "node:assert/strict";
import { test } from "node:test";
import { getModLibraryScrollUiState } from "./modLibraryScrollUi.ts";

test("hides scroll UI when content is at the very top", () => {
  const state = getModLibraryScrollUiState({
    scrollTop: 0,
    scrollHeight: 3518,
    clientHeight: 867,
  });

  assert.equal(state.isScrollable, true);
  assert.equal(state.isAtTop, true);
  assert.equal(state.showScrollUi, false);
  assert.equal(state.showBackToTop, false);
  assert.deepEqual(state.thumbStyle, {
    height: "213.7px",
    transform: "translateY(0px)",
  });
});

test("shows scroll UI when content has moved away from the top", () => {
  const state = getModLibraryScrollUiState({
    scrollTop: 520,
    scrollHeight: 3518,
    clientHeight: 867,
  });

  assert.equal(state.isScrollable, true);
  assert.equal(state.isAtTop, false);
  assert.equal(state.showScrollUi, true);
  assert.equal(state.showBackToTop, false, "a partial viewport scroll only shows the scrollbar");
  assert.deepEqual(state.thumbStyle, {
    height: "213.7px",
    transform: "translateY(128.2px)",
  });
});

test("keeps scroll UI visible while scrolling upward before reaching the top", () => {
  const state = getModLibraryScrollUiState({
    scrollTop: 280,
    scrollHeight: 3518,
    clientHeight: 867,
  });

  assert.equal(state.isScrollable, true);
  assert.equal(state.isAtTop, false);
  assert.equal(state.showScrollUi, true);
  assert.deepEqual(state.thumbStyle, {
    height: "213.7px",
    transform: "translateY(69px)",
  });
});

test("hides scroll UI when content is not scrollable", () => {
  const state = getModLibraryScrollUiState({
    scrollTop: 0,
    scrollHeight: 640,
    clientHeight: 640,
  });

  assert.equal(state.isScrollable, false);
  assert.equal(state.isAtTop, true);
  assert.equal(state.showScrollUi, false);
  assert.equal(state.showBackToTop, false);
  assert.deepEqual(state.thumbStyle, {
    height: "0px",
    transform: "translateY(0px)",
  });
});

test("treats subpixel scrollTop near zero as top to avoid flicker", () => {
  const state = getModLibraryScrollUiState({
    scrollTop: 0.5,
    scrollHeight: 3518,
    clientHeight: 867,
  });

  assert.equal(state.isScrollable, true);
  assert.equal(state.isAtTop, true);
  assert.equal(state.showScrollUi, false);
});

test("clamps thumb position when scrollTop exceeds the maximum scroll range", () => {
  const state = getModLibraryScrollUiState({
    scrollTop: 9999,
    scrollHeight: 3518,
    clientHeight: 867,
  });

  assert.equal(state.isScrollable, true);
  assert.equal(state.showScrollUi, true);
  assert.equal(state.showBackToTop, true);
  assert.deepEqual(state.thumbStyle, {
    height: "213.7px",
    transform: "translateY(653.3px)",
  });
});

test("back-to-top follows a full viewport of actual scroll and recalculates after resizing", () => {
  for (const [scrollTop, clientHeight, expected] of [
    [10, 600, false], [599, 600, false], [600, 600, true], [650, 700, false], [650, 500, true],
  ]) {
    const state = getModLibraryScrollUiState({ scrollTop, clientHeight, scrollHeight: 2200 });
    assert.equal(state.showScrollUi, true);
    assert.equal(state.showBackToTop, expected);
  }
  const shortened = getModLibraryScrollUiState({ scrollTop: 1400, clientHeight: 600, scrollHeight: 800 });
  assert.equal(shortened.showBackToTop, false, "shrinking results must hide an obsolete back-to-top action");
});
