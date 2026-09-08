import assert from "node:assert/strict";
import { test } from "node:test";
import React, { act } from "react";
import TestRenderer from "react-test-renderer";
import { registerReactTestModules } from "../../shared/testing/reactModuleLoader.mjs";

registerReactTestModules({
  "features/about/updateCheckApi.ts": "export const checkAppUpdate = () => globalThis.__updateTest.check();",
});
const { useUpdateCheck, UPDATE_CHECK_UI_TIMEOUT_MILLIS } = await import("./useUpdateCheck.ts");
const { projectUpdateCheckView } = await import("./updateCheckView.ts");
const { UpdateCheckStatus } = await import("./UpdateCheckStatus.tsx");
const { aboutPageCopy } = await import("./aboutPageCopy.ts");
const { I18nProvider } = await import("../../shared/i18n/I18nProvider.tsx");
const { useI18n } = await import("../../shared/i18n/useI18n.ts");
const options = { timeout: 5000, concurrency: false };
globalThis.IS_REACT_ACT_ENVIRONMENT = true;
const current = { status: "up_to_date", currentVersion: "1.0.0", latestVersion: null };
const available = { status: "update_available", currentVersion: "1.0.0", latestVersion: "v1.1.0" };
const noRelease = { status: "no_release", currentVersion: "1.0.0", latestVersion: null };

async function mountCheck(t, preference = { autoCheckEnabled: false, lastCheckedAt: null }) {
  const requests = [];
  const timers = new Map();
  let nextTimer = 0;
  const writes = [];
  const storage = new Map([["helsincy.updateCheckPreference", JSON.stringify({ version: 1, preference })]]);
  const oldWindow = globalThis.window;
  const oldDocument = globalThis.document;
  const state = {};
  globalThis.window = {
    localStorage: { getItem: (key) => storage.get(key) ?? null, setItem: (key, value) => { storage.set(key, value); writes.push({ key, value }); } },
    setTimeout: (callback, delay) => { timers.set(++nextTimer, { callback, delay }); return nextTimer; },
    clearTimeout: (id) => timers.delete(id),
    addEventListener() {}, removeEventListener() {},
  };
  globalThis.document = { documentElement: { lang: "" } };
  globalThis.__updateTest = {
    check: () => new Promise((resolve, reject) => requests.push({ resolve, reject })),
  };
  function Capture() {
    state.check = useUpdateCheck();
    state.i18n = useI18n();
    return React.createElement(UpdateCheckStatus, { view: projectUpdateCheckView(state.check) });
  }
  let root;
  let unmounted = false;
  await act(async () => { root = TestRenderer.create(React.createElement(React.StrictMode, null,
    React.createElement(I18nProvider, null, React.createElement(Capture)))); });
  async function unmount() {
    if (!unmounted) await act(async () => root.unmount());
    unmounted = true;
  }
  t.after(async () => {
    await unmount();
    globalThis.window = oldWindow;
    globalThis.document = oldDocument;
    delete globalThis.__updateTest;
  });
  return {
    state, requests, timers, writes, unmount,
    preference: () => JSON.parse(storage.get("helsincy.updateCheckPreference")).preference,
    message: () => root.root.findByType("p").children.join(""),
    view: () => projectUpdateCheckView(state.check),
    start: async () => { await act(async () => state.check.refresh()); },
    settle: async (value, index = requests.length - 1) => { await act(async () => requests[index].resolve(value)); },
    timeout: async () => {
      assert.equal(timers.size, 1);
      await act(async () => {
        const pending = [...timers.values()];
        timers.clear();
        for (const timer of pending) {
          assert.equal(timer.delay, UPDATE_CHECK_UI_TIMEOUT_MILLIS);
          timer.callback();
        }
      });
    },
  };
}

test("manual update check has visible checking and latest-version results", options, async (t) => {
  const h = await mountCheck(t);
  assert.equal(h.message(), aboutPageCopy.zh_cn.release.notChecked);
  await h.start();
  assert.equal(h.message(), aboutPageCopy.zh_cn.release.checking);
  await h.settle(current);
  assert.equal(h.message(), aboutPageCopy.zh_cn.release.upToDate);
  assert.equal(h.timers.size, 0);
});

for (const [label, result] of [["null", null], ["unknown", { ...current, status: "unknown" }], ["malformed", { ...current, latestVersion: 42 }]]) {
  test(`${label} update result exits checking and offers honest failure feedback`, options, async (t) => {
    const h = await mountCheck(t);
    await h.start();
    await h.settle(result);
    assert.equal(h.state.check.checking, false);
    assert.equal(h.message(), aboutPageCopy.zh_cn.release.unavailable);
    assert.equal(h.preference().lastCheckedAt, null, "An inconclusive attempt cannot suppress checks for 24 hours");
    assert.equal(h.requests.length, 1, "No automatic failure loop");
  });
}

test("rejected update check settles and can be retried", options, async (t) => {
  const h = await mountCheck(t);
  await h.start();
  await act(async () => h.requests[0].reject(new Error("fixture failure")));
  assert.equal(h.message(), aboutPageCopy.zh_cn.release.unavailable);
  await h.start();
  await h.settle(available);
  assert.equal(h.message(), aboutPageCopy.zh_cn.release.updateAvailable("v1.1.0"));
});

test("a failed recheck removes the old success and preserves retry", options, async (t) => {
  const h = await mountCheck(t);
  await h.start();
  await h.settle(current);
  await h.start();
  await h.settle(null);
  assert.equal(h.state.check.status, null);
  assert.equal(h.view().kind, "unavailable");
  assert.equal(h.message(), aboutPageCopy.zh_cn.release.unavailable);
});

test("no usable release is distinct from current and failed in every language", options, async (t) => {
  const h = await mountCheck(t);
  await h.start();
  await h.settle(noRelease);
  for (const locale of ["zh_cn", "en", "ja"]) {
    await act(async () => h.state.i18n.setPreference(locale));
    assert.equal(h.message(), aboutPageCopy[locale].release.noRelease);
    assert.notEqual(h.message(), aboutPageCopy[locale].release.upToDate);
    assert.notEqual(h.message(), aboutPageCopy[locale].release.unavailable);
  }
  assert.equal(h.view().kind, "no_release");
});

test("StrictMode auto check and repeated clicks share one active request", options, async (t) => {
  const h = await mountCheck(t, { autoCheckEnabled: true, lastCheckedAt: null });
  assert.equal(h.requests.length, 1);
  assert.equal(h.timers.size, 1);
  await act(async () => { h.state.check.refresh(); h.state.check.refresh(); });
  assert.equal(h.requests.length, 1);
  await h.settle(current);
  assert.equal(h.writes.length, 1, "Storage writes must stay outside StrictMode state updaters");
});

test("preference changes during a request are saved once and not reverted by its completion", options, async (t) => {
  const h = await mountCheck(t, { autoCheckEnabled: true, lastCheckedAt: null });
  await act(async () => h.state.check.setAutoCheckEnabled(false));
  assert.equal(h.writes.length, 1);
  await h.settle(current);
  assert.equal(h.writes.length, 2);
  assert.equal(h.preference().autoCheckEnabled, false);
  assert.equal(h.state.check.autoCheckEnabled, false);
  assert.ok(h.preference().lastCheckedAt > 0);
});

test("a hung update check times out and its late response cannot replace the retry", options, async (t) => {
  const h = await mountCheck(t);
  await h.start();
  await h.timeout();
  assert.equal(h.view().kind, "unavailable");
  await h.start();
  await h.settle(available, 1);
  await h.settle(current, 0);
  assert.deepEqual(h.view(), { kind: "update_available", version: "v1.1.0" });
});

test("unmount clears the timer and ignores late result and stale refresh callbacks", options, async (t) => {
  const h = await mountCheck(t);
  await h.start();
  const refresh = h.state.check.refresh;
  await h.unmount();
  assert.equal(h.timers.size, 0);
  await h.settle(current);
  await act(async () => refresh());
  assert.equal(h.writes.length, 0);
  assert.equal(h.requests.length, 1);
});
