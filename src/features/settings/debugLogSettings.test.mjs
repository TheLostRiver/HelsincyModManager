import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const read = (path) => readFileSync(path, "utf8");

test("debug logging uses a persistent feature-local narrow API", () => {
  const api = read("src/features/settings/debugLogSettingsApi.ts");
  const types = read("src/features/settings/debugLogSettingsTypes.ts");
  const panel = read("src/features/settings/DebugLogSettingsPanel.tsx");
  const page = read("src/features/settings/SettingsPage.tsx");

  assert.match(api, /invoke<DebugLogSettingsDto>\("get_debug_log_settings"\)/);
  assert.match(api, /invoke<DebugLogSettingsDto>\("set_debug_log_settings",\s*\{\s*enabled\s*\}\)/);
  assert.match(types, /enabled:\s*boolean/);
  assert.match(panel, /loading|正在读取/);
  assert.match(panel, /saving|保存/);
  assert.match(panel, /role="alert"/);
  assert.match(page, /<DebugLogSettingsPanel\s*\/>/);
  assert.doesNotMatch(page, /diagnosticDetails/);
  for (const forbidden of ["readTextFile", "writeTextFile", "readFile", "writeFile", "convertFileSrc", "logPath"]) {
    assert.equal(api.includes(forbidden), false);
    assert.equal(panel.includes(forbidden), false);
  }
});
