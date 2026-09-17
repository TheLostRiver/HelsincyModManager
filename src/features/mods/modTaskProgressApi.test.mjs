import assert from "node:assert/strict";
import { test } from "node:test";
import { registerReactTestModules } from "../../shared/testing/reactModuleLoader.mjs";

registerReactTestModules({}, {
  "@tauri-apps/api/core": "export const invoke = (command, input) => globalThis.__taskProgressInvoke(command, input);",
});
const { getTaskProgress } = await import("./modTaskProgressApi.ts");

test("task observation reads one opaque identity and preserves an unknown result", async () => {
  globalThis.__taskProgressInvoke = (command, input) => {
    assert.equal(command, "get_task_progress");
    assert.deepEqual(input, { taskId: "task-one" });
    return Promise.resolve(null);
  };
  assert.equal(await getTaskProgress("task-one"), null);
  delete globalThis.__taskProgressInvoke;
});
