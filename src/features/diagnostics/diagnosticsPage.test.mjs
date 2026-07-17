import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const read = (path) => readFileSync(path, "utf8");

test("diagnostics route is enabled and uses a feature-local narrow API", () => {
  const routes = read("src/app/routing/routeRegistry.tsx");
  const nav = read("src/app/shell/navigation/navItems.ts");
  const api = read("src/features/diagnostics/diagnosticsApi.ts");
  const types = read("src/features/diagnostics/diagnosticsTypes.ts");
  assert.match(routes, /id:\s*"diagnostics"/);
  assert.match(routes, /path:\s*"\/diagnostics"/);
  assert.equal(nav.split("\n").find((line) => line.includes('id: "diagnostics"'))?.includes("disabledReason"), false);
  assert.match(api, /invoke<DiagnosticsPageSnapshot>\("get_diagnostics_page_snapshot"\)/);
  for (const forbidden of ["readTextFile", "readFile", "convertFileSrc", "diagnosticsPath", "logPath"]) assert.equal(api.includes(forbidden), false);
  for (const field of ["taskLogStatus", "auditLogStatus", "taskLogWriteFailureCount", "auditWriteFailureCount", "auditWriteFailureAfterCommitCount"]) assert.match(types, new RegExp(`${field}:`));
});

test("diagnostics page exposes stable states, controlled export confirmation and safe copying", () => {
  const page = read("src/features/diagnostics/DiagnosticsPage.tsx");
  assert.match(page, /status:\s*"loading"/);
  assert.match(page, /status:\s*"failed"/);
  assert.match(page, /确认导出诊断包/);
  assert.match(page, /fields\.error_code/);
  assert.match(page, /fields\.task_id/);
  assert.match(page, /combinedStatus\(snapshot\.taskLogStatus/);
  assert.match(page, /diagnostics\.copy\.failed/);
  for (const forbidden of ["rawError", "localPath", "fullPath"]) assert.equal(page.includes(forbidden), false);
});
