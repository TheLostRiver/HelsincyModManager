import assert from "node:assert/strict";
import { test } from "node:test";
import { createModLibrarySessionStore } from "./modLibrarySessionStore.ts";
import { attachModLibraryWriteTracking, publishModLibraryTaskProgress, trackModLibraryTaskStart, trackModLibraryBatchWrite, trackModLibraryManifestScan } from "./modLibraryWriteTracking.ts";

const target = { gameId: "mhw", profileId: "scope", modId: "a" };
const task = (taskId, status = "queued") => ({ taskId, kind: "install", status });
function setup(t) {
  const store = createModLibrarySessionStore();
  store.setAvailable(true);
  t.after(attachModLibraryWriteTracking(store));
  return store;
}

test("detail integrity findings survive manifest-only reads and clear only after a fresh scan", async (t) => {
  const store = setup(t);
  const input = { gameId: target.gameId, profileId: target.profileId, modIds: [target.modId] };
  const summary = { profileId: target.profileId, modId: target.modId, status: "installed", managedFileCount: 1, backupCount: 0 };
  store.acceptInstallationStates({ ...input, epoch: "test", revision: 1, available: true, reset: false, summaries: [summary] }, store.getGeneration());
  const page = { items: [{ id: target.modId, status: "installed" }] };
  await trackModLibraryManifestScan(input, async () => [{ ...summary, status: "repair_required" }]);
  await trackModLibraryManifestScan({ profileId: target.profileId, modIds: input.modIds }, async () => [summary]);
  assert.equal(store.projectPage(page, target).items[0].status, "repair_required");
  await trackModLibraryManifestScan(input, async () => [summary]);
  assert.equal(store.projectPage(page, target).items[0].status, "installed");
});

test("a terminal event before the start reply closes the registered writer once", async (t) => {
  const store = setup(t);
  const generation = store.getGeneration();
  await trackModLibraryTaskStart(target, async () => {
    assert.equal(store.isWriting(), true, "registration precedes IPC");
    publishModLibraryTaskProgress(task("one"));
    publishModLibraryTaskProgress(task("one", "completed"));
    assert.equal(store.isWriting(), true, "a pending start reply still owns the boundary");
    return task("one");
  });
  assert.equal(store.isWriting(), false);
  assert.equal(store.getGeneration(), generation + 2);
  publishModLibraryTaskProgress(task("one", "running"));
  publishModLibraryTaskProgress(task("one", "completed"));
  assert.equal(store.getGeneration(), generation + 2);
  assert.deepEqual(store.pendingStatusModIds("mhw", "scope"), ["a"]);
});

test("failed starts release only their own occupancy", async (t) => {
  const store = setup(t);
  await trackModLibraryTaskStart(target, async () => task("still-running"));
  await assert.rejects(trackModLibraryTaskStart(target, async () => { throw new Error("start failed"); }));
  assert.equal(store.isWriting(), true);
  publishModLibraryTaskProgress(task("still-running", "failed"));
  assert.equal(store.isWriting(), false);
});

test("batch start and retry hold one boundary through parent events and replies", async (t) => {
  const store = setup(t);
  const generation = store.getGeneration();
  for (const attemptNumber of [0, 1]) {
    await trackModLibraryBatchWrite(async () => {
      assert.equal(store.isWriting(), true);
      const completed = task(`batch-${attemptNumber}`, "completed");
      publishModLibraryTaskProgress(completed);
      assert.equal(store.isWriting(), true);
      return { task: completed, batchId: "batch", attemptNumber };
    });
    assert.equal(store.isWriting(), false);
  }
  assert.equal(store.getGeneration(), generation + 4);
  assert.deepEqual(store.activeTaskIds(), [], "batch tasks never enter desktop polling after their terminal reply");
});

test("unknown task state cannot release an occupied writer", async (t) => {
  const store = setup(t);
  await trackModLibraryTaskStart(target, async () => task("lost-event"));
  assert.equal(store.isWriting(), true);
  assert.deepEqual(store.activeTaskIds(), ["lost-event"]);
  publishModLibraryTaskProgress(task("lost-event", "completed"));
  assert.equal(store.isWriting(), false);
});
