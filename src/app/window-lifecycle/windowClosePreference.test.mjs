import assert from "node:assert/strict";
import { test } from "node:test";
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
  saveWindowClosePreference(storage, "tray");
  assert.equal(loadWindowClosePreference(storage), "tray");
  saveWindowClosePreference(storage, "exit");
  assert.equal(loadWindowClosePreference(storage), "exit");
});

test("resolves stored preferences to close actions", () => {
  assert.equal(resolveWindowCloseAction("ask"), "show_dialog");
  assert.equal(resolveWindowCloseAction("tray"), "hide_to_tray");
  assert.equal(resolveWindowCloseAction("exit"), "exit_app");
});
