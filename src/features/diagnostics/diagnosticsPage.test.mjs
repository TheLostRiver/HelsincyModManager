import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { createLatestRequestController, createSingleFlightController, runDeferred } from "./diagnosticsPageLogic.ts";

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
  assert.match(page, /diagnostics-page__dialog-action is-secondary/);
  assert.match(page, /diagnostics-page__dialog-action is-primary/);
  assert.match(page, /busy=\{exporting\}/);
  assert.match(page, /initialFocusRef=\{cancelExportRef\}/);
  assert.match(page, /ref=\{cancelExportRef\}/);
  assert.match(page, /loadControllerRef\.current\.run/);
  assert.match(page, /loadController\.invalidate\(\)/);
  assert.match(page, /exportControllerRef\.current\.run/);
  assert.match(page, /runDeferred\(\(\) => navigator\.clipboard\.writeText\(value\)\)/);
  const css = read("src/features/diagnostics/DiagnosticsPage.css");
  assert.match(css, /\.diagnostics-page__dialog-action\.is-primary/);
  assert.match(css, /\.diagnostics-page__dialog-action:focus-visible/);
  assert.match(css, /\.diagnostics-page__dialog-action\{display:inline-flex;align-items:center;justify-content:center;gap:8px/);
  assertSelectorAfter(css, ".diagnostics-page__dialog-action.is-secondary:active:not(:disabled)", ".diagnostics-page__dialog-action.is-secondary:hover:not(:disabled)");
  assertSelectorAfter(css, ".diagnostics-page__dialog-action.is-primary:active:not(:disabled)", ".diagnostics-page__dialog-action.is-primary:hover:not(:disabled)");
  assert.match(css, /\.diagnostics-page\{grid-column:1\/-1;width:100%;min-width:0\}/);
  assert.match(page, /fields\.error_code/);
  assert.match(page, /fields\.task_id/);
  assert.match(page, /combinedStatus\(snapshot\.taskLogStatus/);
  assert.match(page, /diagnostics\.copy\.failed/);
  assert.match(page, /result\.appLogLineCount/);
  assert.match(page, /result\.taskLogLineCount/);
  assert.match(page, /result\.auditEventCount/);
  assert.match(page, /1024 \* 1024/);
  for (const forbidden of ["rawError", "localPath", "fullPath"]) assert.equal(page.includes(forbidden), false);
});

test("latest diagnostics request alone may update page state", async () => {
  const controller = createLatestRequestController();
  const older = deferred();
  const latest = deferred();
  const updates = [];
  const callbacks = {
    onSuccess: (value) => updates.push(`ready:${value}`),
    onFailure: () => updates.push("failed"),
  };

  const olderRun = controller.run(() => older.promise, callbacks);
  const latestRun = controller.run(() => latest.promise, callbacks);
  latest.resolve("latest");
  await latestRun;
  older.resolve("older");
  await olderRun;

  assert.deepEqual(updates, ["ready:latest"]);

  const invalidated = deferred();
  const invalidatedRun = controller.run(() => invalidated.promise, callbacks);
  controller.invalidate();
  invalidated.reject(new Error("stale failure"));
  await invalidatedRun;
  assert.deepEqual(updates, ["ready:latest"]);

  const failed = deferred();
  const failedRun = controller.run(() => failed.promise, callbacks);
  failed.reject(new Error("current failure"));
  await failedRun;
  assert.deepEqual(updates, ["ready:latest", "failed"]);
});

test("diagnostics export controller is synchronous single-flight and releases after settlement", async () => {
  const controller = createSingleFlightController();
  const pending = deferred();
  let calls = 0;

  const first = controller.run(() => {
    calls += 1;
    return pending.promise;
  });
  const duplicate = controller.run(() => {
    calls += 1;
    return "duplicate";
  });

  assert.ok(first);
  assert.equal(duplicate, null);
  await Promise.resolve();
  assert.equal(calls, 1);
  pending.resolve("done");
  assert.equal(await first, "done");

  const afterSettlement = controller.run(() => {
    calls += 1;
    return "next";
  });
  assert.ok(afterSettlement);
  assert.equal(await afterSettlement, "next");
  assert.equal(calls, 2);

  const failed = controller.run(() => {
    calls += 1;
    throw new Error("export failed");
  });
  assert.ok(failed);
  await assert.rejects(failed, /export failed/);

  const retry = controller.run(() => {
    calls += 1;
    return "retry";
  });
  assert.ok(retry);
  assert.equal(await retry, "retry");
  assert.equal(calls, 4);
});

test("deferred diagnostics operations convert synchronous failures into rejections", async () => {
  await assert.rejects(
    runDeferred(() => {
      throw new Error("sync failure");
    }),
    /sync failure/,
  );
});

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function assertSelectorAfter(css, laterSelector, earlierSelector) {
  const earlierIndex = css.indexOf(earlierSelector);
  const laterIndex = css.indexOf(laterSelector);
  assert.notEqual(earlierIndex, -1, `${earlierSelector} should exist`);
  assert.ok(laterIndex > earlierIndex, `${laterSelector} should follow ${earlierSelector}`);
}
