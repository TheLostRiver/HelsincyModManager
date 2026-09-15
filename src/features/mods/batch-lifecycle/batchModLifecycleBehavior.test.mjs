import assert from "node:assert/strict";
import { test } from "node:test";
import React, { act } from "react";
import TestRenderer from "react-test-renderer";
import { registerReactTestModules } from "../../../shared/testing/reactModuleLoader.mjs";

registerReactTestModules({}, {
  "@tauri-apps/api/core": "export const invoke = (command, input) => globalThis.__batchLifecycleTest.invoke(command, input);",
});
const { useBatchModLifecycleWorkflow } = await import("./useBatchModLifecycleWorkflow.ts");
globalThis.IS_REACT_ACT_ENVIRONMENT = true;
const options = { concurrency: false, timeout: 5000 };
const deferred = () => {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
};
const started = (attemptNumber = 0) => ({ batchId: "batch-a", attemptNumber,
  task: { taskId: `attempt-${attemptNumber}`, kind: "install", status: "completed" } });
const sealed = { batchId: "batch-a", planToken: "opaque-plan" };

async function mount(t) {
  const api = { calls: [], handlers: {}, settled: 0 };
  api.invoke = async (command, input) => {
    api.calls.push({ command, input });
    if (api.handlers[command]) return api.handlers[command](input);
    switch (command) {
      case "get_mod_plugin_selection": return null;
      case "preview_batch_mod_lifecycle": return { status: "ready", previewToken: "opaque-preview", blockedItemCount: 0 };
      case "seal_batch_mod_lifecycle": return sealed;
      case "start_batch_mod_lifecycle": return started();
      case "retry_batch_mod_lifecycle": return started(1);
      case "get_batch_mod_lifecycle_result": return { batchId: input.batchId, attemptNumber: input.attemptNumber,
        status: input.attemptNumber === 0 ? "completed_with_errors" : "completed", items: [], nextCursor: null };
      default: throw new Error(`unexpected command: ${command}`);
    }
  };
  globalThis.__batchLifecycleTest = api;
  let root, workflow;
  function Harness({ scopeId }) {
    workflow = useBatchModLifecycleWorkflow({
      gameId: "mhw", profileId: scopeId,
      loadManifestStatuses: async (ids) => ids.map((modId) => ({ modId, status: "not_installed" })),
      loadRevisions: async (modId) => ({ modId, originRevisionId: `rev-${modId}`, displayRevisionId: `rev-${modId}`,
        revisions: [{ revisionId: `rev-${modId}` }] }),
      onWriteSettled: () => { api.settled++; },
    });
    return null;
  }
  const tree = (scopeId = "default") => React.createElement(React.StrictMode, null, React.createElement(Harness, { scopeId }));
  await act(async () => { root = TestRenderer.create(tree()); });
  t.after(async () => { await act(async () => root.unmount()); delete globalThis.__batchLifecycleTest; });
  return { api, get workflow() { return workflow; },
    updateScope: async (scope) => { await act(async () => root.update(tree(scope))); },
    prepare: async () => { await act(async () => workflow.prepare("install", ["mod-a", "mod-b"])); },
    calls: (command) => api.calls.filter((call) => call.command === command),
  };
}

test("confirmation and running batches survive selection reset and reject repeated submissions", options, async (t) => {
  const h = await mount(t);
  const seal = deferred(), start = deferred();
  h.api.handlers.seal_batch_mod_lifecycle = () => seal.promise;
  h.api.handlers.start_batch_mod_lifecycle = () => start.promise;
  await h.prepare();
  let pending;
  await act(async () => { pending = h.workflow.confirmAndStart(); });
  assert.equal(h.workflow.state.status, "confirming");
  await act(async () => { h.workflow.reset(); await h.workflow.confirmAndStart(); await h.workflow.prepare("install", []); });
  assert.equal(h.workflow.state.status, "confirming");
  assert.equal(h.calls("seal_batch_mod_lifecycle").length, 1);
  assert.equal(h.calls("seal_batch_mod_lifecycle")[0].input.request.items.length, 2);
  await act(async () => { seal.resolve(sealed); });
  assert.equal(h.workflow.state.status, "starting");
  assert.equal(h.workflow.taskActive, true);
  await act(async () => { h.workflow.reset(); await h.workflow.prepare("install", ["other"]); });
  assert.equal(h.workflow.state.status, "starting");
  assert.equal(h.calls("preview_batch_mod_lifecycle").length, 1);
  assert.deepEqual(h.calls("start_batch_mod_lifecycle")[0].input, sealed);
  await act(async () => { start.resolve(started()); await pending; });
  assert.equal(h.workflow.state.status, "result");
  assert.equal(h.workflow.taskActive, false);
  assert.equal(h.api.settled, 1);
  const result = h.workflow.state;
  await act(async () => h.workflow.invalidatePreview());
  assert.equal(h.workflow.state, result, "filter refresh must not dismiss the completed result");
  await act(async () => h.workflow.reset());
  assert.equal(h.workflow.state.status, "idle", "explicit close still dismisses the result");
});

test("retry is a protected running attempt and queries the new attempt only once", options, async (t) => {
  const h = await mount(t);
  await h.prepare();
  await act(async () => h.workflow.confirmAndStart());
  const retry = deferred();
  h.api.handlers.retry_batch_mod_lifecycle = () => retry.promise;
  let pending;
  await act(async () => { pending = h.workflow.retry(); });
  assert.equal(h.workflow.state.status, "retrying");
  assert.equal(h.workflow.taskActive, true);
  await act(async () => { await h.workflow.retry(); h.workflow.reset(); await h.workflow.prepare("install", []); });
  assert.equal(h.workflow.state.status, "retrying");
  assert.equal(h.calls("retry_batch_mod_lifecycle").length, 1);
  assert.deepEqual(h.calls("retry_batch_mod_lifecycle")[0].input, { batchId: "batch-a", expectedAttemptNumber: 0 });
  await act(async () => { retry.resolve(started(1)); await pending; });
  assert.equal(h.workflow.state.status, "result");
  assert.equal(h.workflow.state.attemptNumber, 1);
  assert.deepEqual(h.calls("get_batch_mod_lifecycle_result").map((call) => call.input.attemptNumber), [0, 1]);
  assert.equal(h.api.settled, 2);
});

for (const failedCommand of ["start_batch_mod_lifecycle", "get_batch_mod_lifecycle_result"]) {
  test(`${failedCommand} failure still invalidates library facts and retains the batch identity`, options, async (t) => {
    const h = await mount(t);
    h.api.handlers[failedCommand] = () => { throw { code: "batch_result_unavailable" }; };
    await h.prepare();
    await act(async () => h.workflow.confirmAndStart());
    assert.equal(h.workflow.state.status, "result-error");
    assert.equal(h.workflow.state.batchId, "batch-a");
    assert.equal(h.workflow.state.attemptNumber, 0);
    assert.equal(h.api.settled, 1);
  });
}

test("an old write settling after a scope change invalidates caches without replacing the new view", options, async (t) => {
  const h = await mount(t);
  const start = deferred();
  h.api.handlers.start_batch_mod_lifecycle = () => start.promise;
  await h.prepare();
  let pending;
  await act(async () => { pending = h.workflow.confirmAndStart(); });
  assert.equal(h.workflow.state.status, "starting");
  await h.updateScope("installation-other");
  await h.prepare();
  const currentPreview = h.workflow.state;
  assert.equal(currentPreview.request.profileId, "installation-other");
  await act(async () => { start.resolve(started()); await pending; });
  assert.equal(h.workflow.state, currentPreview);
  assert.equal(h.api.settled, 1);
  assert.equal(h.calls("get_batch_mod_lifecycle_result").length, 0);
});
