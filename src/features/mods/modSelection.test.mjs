import assert from "node:assert/strict";
import { test } from "node:test";
import { applyModSelection } from "./modSelection.ts";

function ids(selection) {
  return [...selection].sort();
}

test("replace selection keeps ordinary card clicks single-select by default", () => {
  const previous = new Set(["mod-a"]);

  const next = applyModSelection(previous, "mod-b", "replace");

  assert.deepEqual(ids(next), ["mod-b"]);
});

test("replace selection collapses an existing multi-selection to the clicked card", () => {
  const previous = new Set(["mod-a", "mod-b"]);

  const next = applyModSelection(previous, "mod-b", "replace");

  assert.deepEqual(ids(next), ["mod-b"]);
});

test("replace selection toggles off the only selected card", () => {
  const previous = new Set(["mod-a"]);

  const next = applyModSelection(previous, "mod-a", "replace");

  assert.deepEqual(ids(next), []);
});

test("toggle mode preserves the future multi-select path", () => {
  const previous = new Set(["mod-a"]);

  const next = applyModSelection(previous, "mod-b", "toggle");

  assert.deepEqual(ids(next), ["mod-a", "mod-b"]);
});
