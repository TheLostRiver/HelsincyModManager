import assert from "node:assert/strict";
import { test } from "node:test";
import { contextMenuPosition } from "./contextMenuPosition.ts";
import { modShortcutCopy, modShortcutErrorMessage } from "./modShortcutCopy.ts";

test("a full menu remains visible when opened at the bottom right", () => {
  const viewport = { width: 1280, height: 800 };
  const menu = { width: 300, height: 458 };
  const result = contextMenuPosition({ x: 1220, y: 762 }, menu, viewport);
  assert.ok(result.left >= 8 && result.left + menu.width <= viewport.width - 8);
  assert.ok(result.top >= 8 && result.top + menu.height <= viewport.height - 8);
});

test("position responds to measured height changes and clamps negative anchors", () => {
  const viewport = { width: 480, height: 600 };
  assert.deepEqual(contextMenuPosition({ x: -10, y: -10 }, { width: 200, height: 220 }, viewport), { left: 8, top: 8 });
  const short = contextMenuPosition({ x: 100, y: 480 }, { width: 300, height: 220 }, viewport);
  const tall = contextMenuPosition({ x: 100, y: 480 }, { width: 300, height: 450 }, viewport);
  assert.ok(tall.top < short.top);
  assert.equal(tall.top + 450, 592);
});

test("a viewport-limited scrollable menu keeps both edges inside the window", () => {
  assert.deepEqual(contextMenuPosition({ x: 300, y: 180 }, { width: 344, height: 224 }, { width: 360, height: 240 }), { left: 8, top: 8 });
});

test("shortcut failures explain the missing ID or folder and hide raw backend details", () => {
  for (const copy of Object.values(modShortcutCopy)) {
    assert.equal(modShortcutErrorMessage({ code: "mod_nexus_id_missing" }, copy), copy.nexusIdMissing);
    assert.equal(modShortcutErrorMessage({ code: "mod_folder_unavailable" }, copy), copy.folderUnavailable);
    assert.equal(modShortcutErrorMessage({ code: "mod_nexus_open_failed" }, copy), copy.nexusOpenFailed);
    assert.equal(modShortcutErrorMessage(new Error("private-path"), copy), copy.unavailable);
  }
});
