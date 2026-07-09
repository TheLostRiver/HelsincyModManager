import assert from "node:assert/strict";
import { test } from "node:test";
import { getWindowLifecycleErrorMessage } from "./windowLifecycleError.ts";
import {
  CLOSE_BEHAVIOR_STORAGE_KEY,
  loadWindowClosePreference,
  resolveWindowCloseAction,
  saveWindowClosePreference,
} from "./windowClosePreference.ts";

function createStorage(initial = null) {
  const store = new Map();
  if (initial !== null) store.set(CLOSE_BEHAVIOR_STORAGE_KEY, initial);
  return {
    getItem: (key) => (store.has(key) ? store.get(key) : null),
    setItem: (key, value) => store.set(key, value),
  };
}

test("loads ask when storage is unavailable or invalid", () => {
  assert.equal(loadWindowClosePreference(undefined), "ask");
  assert.equal(loadWindowClosePreference(createStorage(JSON.stringify("bad"))), "ask");
  assert.equal(loadWindowClosePreference(createStorage("not-json")), "ask");
});

test("saves and loads stable close behavior values", () => {
  const storage = createStorage();
  assert.equal(saveWindowClosePreference(storage, "tray"), true);
  assert.equal(loadWindowClosePreference(storage), "tray");
  assert.equal(saveWindowClosePreference(storage, "exit"), true);
  assert.equal(loadWindowClosePreference(storage), "exit");
});

test("saveWindowClosePreference reports storage failures without throwing", () => {
  const storage = {
    getItem: () => null,
    setItem: () => {
      throw new Error("quota exceeded");
    },
  };

  assert.equal(saveWindowClosePreference(storage, "tray"), false);
});

test("resolves stored preferences to close actions", () => {
  assert.equal(resolveWindowCloseAction("ask"), "show_dialog");
  assert.equal(resolveWindowCloseAction("tray"), "hide_to_tray");
  assert.equal(resolveWindowCloseAction("exit"), "exit_app");
});

test("extracts user visible messages from Tauri command error DTOs", () => {
  assert.equal(
    getWindowLifecycleErrorMessage({ code: "window_hide_failed", message: "窗口隐藏失败" }),
    "窗口隐藏失败",
  );
  assert.equal(getWindowLifecycleErrorMessage(new Error("原生错误")), "原生错误");
  assert.equal(getWindowLifecycleErrorMessage("字符串错误"), "字符串错误");
  assert.equal(getWindowLifecycleErrorMessage({ code: "missing_message" }), "窗口关闭操作失败");
});
