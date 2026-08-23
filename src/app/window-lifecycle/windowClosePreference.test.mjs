import assert from "node:assert/strict";
import { test } from "node:test";
import {
  getWindowLifecycleErrorCode,
  getWindowLifecycleErrorMessage,
} from "./windowLifecycleError.ts";
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

test("maps only stable Tauri command error codes to local messages", async () => {
  const { appShellCopy } = await import("../appShellCopy.ts");
  const zhLifecycle = appShellCopy.zh_cn.windowLifecycle;
  assert.equal(
    getWindowLifecycleErrorMessage({ code: "window_hide_failed", message: "C:/Users/Alice/save" }, zhLifecycle),
    "窗口隐藏失败，请重试。",
  );
  assert.equal(
    getWindowLifecycleErrorMessage({ code: "exit_confirmation_required", message: "raw backend message" }, zhLifecycle),
    "退出前需要确认后台保护状态。",
  );
  assert.equal(
    getWindowLifecycleErrorMessage({ code: "exit_authorization_unavailable", message: "raw backend message" }, zhLifecycle),
    "退出确认状态不可用，请暂时留在托盘或重启应用后再试。",
  );
  assert.equal(getWindowLifecycleErrorMessage(new Error("C:/Users/Alice/save"), zhLifecycle), "窗口关闭操作失败");
  assert.equal(getWindowLifecycleErrorMessage("raw backend message", zhLifecycle), "窗口关闭操作失败");
  assert.equal(getWindowLifecycleErrorMessage({ code: "missing_message" }, zhLifecycle), "窗口关闭操作失败");
  assert.equal(getWindowLifecycleErrorMessage({ code: "toString" }, zhLifecycle), "窗口关闭操作失败");
  assert.equal(getWindowLifecycleErrorMessage({ code: "constructor" }, zhLifecycle), "窗口关闭操作失败");
});

test("extracts stable error codes without reading backend messages", () => {
  assert.equal(getWindowLifecycleErrorCode({ code: "exit_confirmation_required", message: "ignored" }), "exit_confirmation_required");
  assert.equal(getWindowLifecycleErrorCode({ code: 42, message: "ignored" }), null);
  assert.equal(getWindowLifecycleErrorCode(new Error("exit_confirmation_required")), null);
});
