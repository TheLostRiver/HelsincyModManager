import assert from "node:assert/strict";
import { test } from "node:test";
import { registerReactTestModules } from "../../shared/testing/reactModuleLoader.mjs";

registerReactTestModules({}, { "@tauri-apps/api/core": `export const invoke = async (command, args) => { globalThis.__reapplyCalls.push({ command, args }); return {}; };` });
const { previewEquipmentReapply, startEquipmentReapply } = await import("./equipmentRetargetApi.ts");

test("reapply transport sends only scope and token despite extra caller properties", async (t) => {
  globalThis.__reapplyCalls = [];
  t.after(() => { delete globalThis.__reapplyCalls; });
  const scope = { gameId: "mhw", profileId: "profile", modId: "mod" };
  const input = { ...scope, slots: [{ action: "retarget", targetId: "other" }], layerName: "other", targetPath: "not-an-input", intent: "force" };
  await previewEquipmentReapply(input);
  await startEquipmentReapply(input, "fixture-token");
  assert.deepEqual(globalThis.__reapplyCalls, [
    { command: "preview_equipment_reapply", args: { request: scope } },
    { command: "start_equipment_reapply_task", args: { request: { selection: scope, planToken: "fixture-token" } } },
  ]);
});
