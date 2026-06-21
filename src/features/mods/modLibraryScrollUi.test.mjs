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
  assert.deepEqual(state.thumbStyle, {
    height: "213.7px",
    transform: "translateY(653.3px)",
  });
});
