import assert from "node:assert/strict";
import { test } from "node:test";
import React, { act } from "react";
import TestRenderer from "react-test-renderer";
import { registerReactTestModules } from "../../shared/testing/reactModuleLoader.mjs";

registerReactTestModules({
  "features/mods/externalStateApi.ts": `
    export const getExternalModState = (input) => globalThis.__externalBehavior.read(input);
    export const startExternalModStateScan = (input) => globalThis.__externalBehavior.start(input);
    export const startExternalModAdopt = (input) => globalThis.__externalBehavior.start(input);`,
}, {
  "@tauri-apps/api/event": "export const listen = (_name, callback) => globalThis.__externalBehavior.listen(callback);",
});
const { useExternalModState } = await import("./useExternalModState.ts");
globalThis.IS_REACT_ACT_ENVIRONMENT = true;
const options = { concurrency: false, timeout: 5000 };

async function mount(t) {
  const reads = [], starts = [], results = [];
  let current, listener, root;
  const deferred = (list, input) => new Promise((resolve, reject) => list.push({ input, resolve, reject }));
  globalThis.__externalBehavior = {
    read: (input) => deferred(reads, input), start: (input) => deferred(starts, input),
    listen: async (callback) => { listener = callback; return () => { listener = null; }; },
  };
  function Capture({ profileId = "one", modId = "a" }) {
    current = useExternalModState({ gameId: "mhw", profileId, modId, active: true, onResult: (id, value) => results.push([id, value]) });
    return null;
  }
  await act(async () => { root = TestRenderer.create(React.createElement(Capture)); });
  let removed = false;
  const unmount = async () => { if (!removed) await act(async () => root.unmount()); removed = true; };
  t.after(async () => { await unmount(); delete globalThis.__externalBehavior; });
  return { reads, starts, results, unmount, get current() { return current; },
    update: async (props) => { await act(async () => root.update(React.createElement(Capture, props))); },
    resolve: async (request, value) => { await act(async () => request.resolve(value)); },
    emit: async (payload) => { await act(async () => listener({ payload })); },
  };
}

test("external state delivers the current query to the section and session store", options, async (t) => {
  const h = await mount(t);
  await h.resolve(h.reads[0], { summary: null, stale: false, lastError: null });
  assert.equal(h.current.loaded, true);
  assert.equal(h.results.length, 1);
});

test("late external state from another profile cannot contaminate the current session", options, async (t) => {
  const h = await mount(t);
  await h.update({ profileId: "two" });
  await h.resolve(h.reads[1], { source: "new profile" });
  await h.resolve(h.reads[0], { source: "old profile" });
  assert.deepEqual(h.results, [["a", { source: "new profile" }]]);
  assert.equal(h.current.state.source, "new profile");
});

test("a newer external state refresh wins over a slower initial query", options, async (t) => {
  const h = await mount(t);
  await act(async () => h.current.refresh());
  await h.resolve(h.reads[1], { source: "after scan" });
  await h.resolve(h.reads[0], { source: "before scan" });
  assert.equal(h.current.state.source, "after scan");
  assert.deepEqual(h.results, [["a", { source: "after scan" }]]);
});

test("unmounted external state queries cannot publish session facts", options, async (t) => {
  const h = await mount(t);
  await h.unmount();
  await h.resolve(h.reads[0], { source: "unmounted" });
  assert.deepEqual(h.results, []);
});

test("switching mods drops the previous mod query", options, async (t) => {
  const h = await mount(t);
  await h.update({ modId: "b" });
  await h.resolve(h.reads[0], { source: "old mod" });
  assert.deepEqual(h.results, []);
  assert.equal(h.current.state, null);
});

test("scan terminal events arriving before the start response are reconciled once", options, async (t) => {
  const h = await mount(t);
  await act(async () => { h.current.startScan(); h.current.startScan(); h.current.startAdopt(); });
  assert.equal(h.starts.length, 1);
  await h.emit({ taskId: "scan-a", kind: "external_state_scan", status: "completed", error: null });
  assert.equal(h.current.scanning, true);
  await h.resolve(h.starts[0], { task: { taskId: "scan-a" } });
  assert.equal(h.current.scanning, false);
  assert.equal(h.reads.length, 2);
});
