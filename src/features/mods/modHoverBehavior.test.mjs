import assert from "node:assert/strict";
import { test } from "node:test";
import React, { act } from "react";
import TestRenderer from "react-test-renderer";
import { registerReactTestModules } from "../../shared/testing/reactModuleLoader.mjs";

registerReactTestModules({
  "features/mods/modLibraryApi.ts": "export const getModDetail = (input) => globalThis.__hoverTest.detail(input);",
  "features/replacements/replacementApi.ts": "export const getModReplacementSummary = (input) => globalThis.__hoverTest.summary(input);",
  "features/mods/ModLibrarySessionCacheProvider.tsx": "export const useModLibrarySessionCache = () => globalThis.__hoverTest.cache;",
  "features/mods/ModCardHover.tsx": "export const ModCardHover = ({ children }) => children;",
});
const { useModHoverDetails, MOD_HOVER_TIMEOUT_MILLIS } = await import("./useModHoverDetails.ts");
const { createModLibrarySessionStore } = await import("./modLibrarySessionStore.ts");
const { ModPosterCard } = await import("./ModPosterCard.tsx");
const { I18nProvider } = await import("../../shared/i18n/I18nProvider.tsx");
const options = { timeout: 5000, concurrency: false };
globalThis.IS_REACT_ACT_ENVIRONMENT = true;
const detailFor = (id = "a") => ({ id, name: `Name ${id}`, packageId: `pkg-${id}`, metadata: { tags: [], dependencies: [] } });
const summaryFor = (id = "a") => ({ modId: id, gameId: "mhw", packageId: `pkg-${id}`, sources: [], installedTargets: [] });

async function mountHover(t) {
  const cache = createModLibrarySessionStore();
  const details = [], summaries = [], timers = new Map();
  let timerId = 0;
  const state = {};
  const oldWindow = globalThis.window;
  const makeRequest = (list, input) => new Promise((resolve, reject) => list.push({ input, resolve, reject }));
  globalThis.__hoverTest = { cache, detail: (input) => makeRequest(details, input), summary: (input) => makeRequest(summaries, input) };
  globalThis.window = { setTimeout: (callback, delay) => { timers.set(++timerId, { callback, delay }); return timerId; }, clearTimeout: (id) => timers.delete(id),
    localStorage: { getItem: () => null, setItem() {} }, addEventListener() {}, removeEventListener() {} };
  function Capture({ modId, profileId }) { state.current = useModHoverDetails(modId, "mhw", profileId); return null; }
  const tree = (props) => React.createElement(React.StrictMode, null, React.createElement(Capture, props));
  let root;
  await act(async () => { root = TestRenderer.create(tree({ modId: "a", profileId: "one" })); });
  let removed = false;
  async function unmount() { if (!removed) await act(async () => root.unmount()); removed = true; }
  t.after(async () => { await unmount(); globalThis.window = oldWindow; delete globalThis.__hoverTest; });
  return { state, cache, details, summaries, timers, unmount,
    update: async (modId, profileId = "one") => { await act(async () => root.update(tree({ modId, profileId }))); },
    resolve: async (request, result) => { await act(async () => request.resolve(result)); },
    expire: async () => { await act(async () => { const pending = [...timers.values()]; timers.clear(); for (const { callback, delay } of pending) { assert.equal(delay, MOD_HOVER_TIMEOUT_MILLIS); callback(); } }); },
  };
}

test("hover StrictMode mount starts only one detail query and one replacement query", options, async (t) => {
  const h = await mountHover(t);
  assert.equal(h.details.length, 1);
  assert.equal(h.summaries.length, 1);
  assert.equal(h.timers.size, 1);
});

test("hover shows metadata without waiting for the replacement scan", options, async (t) => {
  const h = await mountHover(t);
  await h.resolve(h.details[0], detailFor());
  assert.equal(h.state.current.status, "ready");
  assert.equal(h.state.current.detail.name, "Name a");
  assert.equal(h.state.current.replacementLoading, true);
  await h.resolve(h.summaries[0], summaryFor());
  assert.equal(h.state.current.replacementLoading, false);
  assert.deepEqual(h.state.current.replacement, summaryFor());
  assert.equal(h.timers.size, 0);
});

test("moving hover to another Mod clears old data and rejects late responses", options, async (t) => {
  const h = await mountHover(t);
  await h.resolve(h.details[0], detailFor());
  await h.update("b");
  assert.equal(h.state.current.status, "loading");
  assert.equal(h.state.current.detail, null);
  await h.resolve(h.details[1], detailFor("b"));
  await h.resolve(h.summaries[1], summaryFor("b"));
  await h.resolve(h.summaries[0], summaryFor());
  assert.equal(h.state.current.replacement.modId, "b");
});

test("profile changes cannot reuse the previous profile replacement facts", options, async (t) => {
  const h = await mountHover(t);
  await h.update("a", "two");
  assert.equal(h.summaries[1].input.profileId, "two");
  await h.resolve(h.details[1], detailFor());
  await h.resolve(h.summaries[1], { ...summaryFor(), installedTargets: [{ id: "new" }] });
  await h.resolve(h.summaries[0], summaryFor());
  assert.deepEqual(h.state.current.replacement.installedTargets, [{ id: "new" }]);
});

test("library invalidation clears hover facts and rejects the pre-write response", options, async (t) => {
  const h = await mountHover(t);
  await h.resolve(h.details[0], detailFor());
  await act(async () => h.cache.invalidateAllPages());
  assert.equal(h.state.current.detail, null);
  assert.equal(h.details.length, 2);
  await h.resolve(h.details[1], { ...detailFor(), name: "After write" });
  await h.resolve(h.summaries[1], { ...summaryFor(), installedTargets: [{ id: "new" }] });
  await h.resolve(h.summaries[0], summaryFor());
  assert.equal(h.state.current.detail.name, "After write");
  assert.deepEqual(h.state.current.replacement.installedTargets, [{ id: "new" }]);
});

test("a failed hover detail query is visible without an automatic retry loop", options, async (t) => {
  const h = await mountHover(t);
  await act(async () => h.details[0].reject(new Error("fixture read failed")));
  await h.resolve(h.summaries[0], summaryFor());
  assert.equal(h.state.current.status, "unavailable");
  assert.equal(h.state.current.replacement, null);
  assert.equal(h.details.length, 1);
});

test("a failed replacement scan keeps metadata but does not claim no retarget", options, async (t) => {
  const h = await mountHover(t);
  await h.resolve(h.details[0], detailFor());
  await act(async () => h.summaries[0].reject(new Error("fixture scan failed")));
  assert.equal(h.state.current.status, "ready");
  assert.equal(h.state.current.replacement, null);
  assert.equal(h.state.current.replacementLoading, false);
});

test("hover timeout settles while preserving metadata and rejecting late scan results", options, async (t) => {
  const h = await mountHover(t);
  await h.resolve(h.details[0], detailFor());
  await h.expire();
  assert.equal(h.state.current.replacementLoading, false);
  assert.equal(h.state.current.replacement, null);
  await h.resolve(h.summaries[0], summaryFor());
  assert.equal(h.state.current.replacement, null);
  assert.equal(h.state.current.detail.name, "Name a");
});

test("unmount releases hover timers and ignores all pending responses", options, async (t) => {
  const h = await mountHover(t);
  await h.unmount();
  assert.equal(h.timers.size, 0);
  const snapshot = h.state.current;
  await h.resolve(h.details[0], detailFor());
  await h.resolve(h.summaries[0], summaryFor());
  assert.equal(h.state.current, snapshot);
});

for (const viewMode of ["classic", "grid", "list", "tech"]) {
  test(`${viewMode} card never invents a missing version`, options, async () => {
    let root;
    const item = { id: "a", name: "Fixture", sizeLabel: "1 MB", status: "not_installed", categoryLabels: [] };
    const tree = (versionLabel) => React.createElement(I18nProvider, null, React.createElement(ModPosterCard, {
      item: { ...item, versionLabel }, viewMode, gameId: "mhw", profileId: null, selected: false, selectionMode: "single", onSelect() {},
    }));
    try {
      await act(async () => { root = TestRenderer.create(tree(undefined)); });
      assert.doesNotMatch(JSON.stringify(root.toJSON()), /v1\.0\.0/);
      if (viewMode !== "classic") {
        await act(async () => root.update(tree("v2.3.4")));
        assert.match(JSON.stringify(root.toJSON()), /v2\.3\.4/);
      }
    } finally { if (root) await act(async () => root.unmount()); }
  });
}
