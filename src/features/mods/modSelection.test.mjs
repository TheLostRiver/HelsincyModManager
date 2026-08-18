import assert from "node:assert/strict";
import { test } from "node:test";
import {
  MAX_MOD_SELECTION_COUNT,
  applyModSelection,
  countSelectedOnPage,
  createInitialModSelectionState,
  reduceModSelection,
} from "./modSelection.ts";

function ids(selection) {
  return [...selection].sort();
}

function state(mode, selectedIds = []) {
  return { mode, selectedIds: new Set(selectedIds), notice: null };
}

function primary(modId) {
  return { type: "apply-intent", intent: { kind: "primary", modId, source: "pointer" } };
}

function ctrlToggle(modId) {
  return {
    type: "apply-intent",
    intent: { kind: "toggle", modId, source: "ctrl-pointer" },
  };
}

test("initial selection state starts empty in single mode", () => {
  const initial = createInitialModSelectionState();

  assert.equal(initial.mode, "single");
  assert.deepEqual(ids(initial.selectedIds), []);
  assert.equal(initial.notice, null);
});

test("replace selection keeps ordinary card clicks single-select by default", () => {
  const previous = new Set(["mod-a"]);

  const next = applyModSelection(previous, "mod-b", "replace");

  assert.deepEqual(ids(next), ["mod-b"]);
});

test("single mode primary clicks replace or cancel the only selected card", () => {
  const replaced = reduceModSelection(state("single", ["mod-a"]), primary("mod-b"));
  const cleared = reduceModSelection(replaced, primary("mod-b"));

  assert.equal(replaced.mode, "single");
  assert.deepEqual(ids(replaced.selectedIds), ["mod-b"]);
  assert.equal(cleared.mode, "single");
  assert.deepEqual(ids(cleared.selectedIds), []);
});

test("first Ctrl selection enters batch mode, preserves the prior item, and toggles the target", () => {
  const added = reduceModSelection(state("single", ["mod-a"]), ctrlToggle("mod-b"));
  const cancelled = reduceModSelection(state("single", ["mod-a"]), ctrlToggle("mod-a"));

  assert.equal(added.mode, "batch");
  assert.deepEqual(ids(added.selectedIds), ["mod-a", "mod-b"]);
  assert.equal(cancelled.mode, "batch");
  assert.deepEqual(ids(cancelled.selectedIds), []);
});

test("ordinary primary clicks toggle cards after batch mode is active", () => {
  const added = reduceModSelection(state("batch", ["mod-a"]), primary("mod-b"));
  const removed = reduceModSelection(added, primary("mod-a"));

  assert.equal(removed.mode, "batch");
  assert.deepEqual(ids(removed.selectedIds), ["mod-b"]);
});

test("batch mode stays active at zero or one item until explicitly exited", () => {
  const entered = reduceModSelection(state("single", ["mod-a"]), { type: "enter-batch" });
  const cleared = reduceModSelection(entered, { type: "clear-selection" });
  const exited = reduceModSelection(entered, { type: "exit-batch" });

  assert.equal(entered.mode, "batch");
  assert.deepEqual(ids(entered.selectedIds), ["mod-a"]);
  assert.equal(cleared.mode, "batch");
  assert.deepEqual(ids(cleared.selectedIds), []);
  assert.equal(exited.mode, "single");
  assert.deepEqual(ids(exited.selectedIds), []);
});

test("the 101st item is rejected while an already selected item can still be removed", () => {
  const selectedIds = Array.from(
    { length: MAX_MOD_SELECTION_COUNT },
    (_, index) => `mod-${index}`,
  );
  const full = state("batch", selectedIds);

  const rejected = reduceModSelection(full, primary("mod-over-limit"));
  const removed = reduceModSelection(rejected, primary("mod-0"));

  assert.equal(rejected.selectedIds.size, MAX_MOD_SELECTION_COUNT);
  assert.equal(rejected.selectedIds.has("mod-over-limit"), false);
  assert.equal(rejected.notice?.code, "mod_selection_limit_reached");
  assert.equal(removed.selectedIds.size, MAX_MOD_SELECTION_COUNT - 1);
  assert.equal(removed.selectedIds.has("mod-0"), false);
  assert.equal(removed.notice, null);
});

test("select page rejects the whole addition when it exceeds the remaining capacity", () => {
  const selectedIds = Array.from({ length: 99 }, (_, index) => `selected-${index}`);
  const previous = state("batch", selectedIds);

  const rejected = reduceModSelection(previous, {
    type: "select-page",
    modIds: ["page-a", "page-b"],
  });

  assert.deepEqual(ids(rejected.selectedIds), ids(previous.selectedIds));
  assert.equal(rejected.notice?.code, "mod_selection_page_limit_exceeded");
  assert.match(rejected.notice?.message ?? "", /当前仅剩 1 个名额/);
});

test("invert page rejects atomically when its complete result exceeds the limit", () => {
  const selectedIds = Array.from(
    { length: MAX_MOD_SELECTION_COUNT },
    (_, index) => `selected-${index}`,
  );
  const previous = state("batch", selectedIds);

  const rejected = reduceModSelection(previous, {
    type: "invert-page",
    modIds: ["page-new"],
  });

  assert.deepEqual(ids(rejected.selectedIds), ids(previous.selectedIds));
  assert.equal(rejected.notice?.code, "mod_selection_page_limit_exceeded");
});

test("query context reset clears selection, exits batch mode, and does not repeat once empty", () => {
  const reset = reduceModSelection(state("batch", ["mod-a", "mod-b"]), {
    type: "reset-context",
    reason: "筛选条件已变化",
  });
  const repeated = reduceModSelection(reset, {
    type: "reset-context",
    reason: "筛选条件已变化",
  });

  assert.equal(reset.mode, "single");
  assert.deepEqual(ids(reset.selectedIds), []);
  assert.equal(reset.notice?.code, "mod_selection_context_reset");
  assert.match(reset.notice?.message ?? "", /已清空 2 项选择/);
  assert.equal(repeated, reset);
});

test("selection notices can be dismissed without changing mode or selected ids", () => {
  const reset = reduceModSelection(state("batch", ["mod-a"]), {
    type: "reset-context",
    reason: "搜索条件已变化",
  });

  const dismissed = reduceModSelection(reset, { type: "dismiss-notice" });

  assert.equal(dismissed.mode, reset.mode);
  assert.equal(dismissed.selectedIds, reset.selectedIds);
  assert.equal(dismissed.notice, null);
});

test("page counts deduplicate page ids and ignore cross-page selections", () => {
  const selectedIds = new Set(["page-a", "page-b", "other-page"]);

  assert.equal(countSelectedOnPage(selectedIds, ["page-a", "page-a", "page-c"]), 1);
});
