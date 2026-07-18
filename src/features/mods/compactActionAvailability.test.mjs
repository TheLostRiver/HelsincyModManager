import assert from "node:assert/strict";
import { test } from "node:test";
import { getCompactActionDisabledReason } from "./compactActionAvailability.ts";

const readyAction = {
  actionId: "install",
  selectedCount: 1,
  profileReady: true,
  installTaskActive: false,
  libraryQueryBusy: false,
  canInstallSelection: true,
  canReinstallSelection: false,
  canUninstallSelection: false,
};

test("compact lifecycle actions explain selection, task, profile, and durable-state blockers in priority order", () => {
  assert.equal(
    getCompactActionDisabledReason({ ...readyAction, selectedCount: 0 }),
    "请先选择一个 MOD",
  );
  assert.equal(
    getCompactActionDisabledReason({ ...readyAction, selectedCount: 2 }),
    "每次只能选择一个 MOD",
  );
  assert.equal(
    getCompactActionDisabledReason({ ...readyAction, installTaskActive: true }),
    "请等待当前安装任务完成",
  );
  assert.equal(
    getCompactActionDisabledReason({ ...readyAction, profileReady: false }),
    "选择配置档后可安装",
  );
  assert.equal(
    getCompactActionDisabledReason({ ...readyAction, canInstallSelection: false }),
    "仅未安装且状态安全的 MOD 可安装",
  );
  assert.equal(getCompactActionDisabledReason(readyAction), undefined);
});

test("query refresh blocks page selection and lifecycle actions with product copy", () => {
  for (const actionId of [
    "select-all",
    "invert",
    "preview-plan",
    "install",
    "reinstall",
    "uninstall",
  ]) {
    assert.equal(
      getCompactActionDisabledReason({ ...readyAction, actionId, libraryQueryBusy: true }),
      "Mod 列表正在更新，请稍候",
    );
  }

  assert.equal(
    getCompactActionDisabledReason({ ...readyAction, actionId: "refresh", libraryQueryBusy: true }),
    undefined,
  );
});

test("preview, reinstall, and uninstall expose action-specific fail-closed reasons", () => {
  assert.equal(
    getCompactActionDisabledReason({
      ...readyAction,
      actionId: "preview-plan",
      canInstallSelection: false,
    }),
    "仅未安装且状态安全的 MOD 可预览安装计划",
  );
  assert.equal(
    getCompactActionDisabledReason({ ...readyAction, actionId: "reinstall" }),
    "仅已安装且状态安全的 MOD 可重装",
  );
  assert.equal(
    getCompactActionDisabledReason({ ...readyAction, actionId: "uninstall" }),
    "仅已安装且状态安全的 MOD 可卸载",
  );
  assert.equal(
    getCompactActionDisabledReason({
      ...readyAction,
      actionId: "reinstall",
      canReinstallSelection: true,
    }),
    undefined,
  );
});

test("future batch actions stay explicitly unavailable without dispatching writes", () => {
  assert.equal(
    getCompactActionDisabledReason({ ...readyAction, actionId: "enable-all" }),
    "批量启用暂不可用",
  );
  assert.equal(
    getCompactActionDisabledReason({ ...readyAction, actionId: "disable-all" }),
    "批量禁用暂不可用",
  );
  assert.equal(
    getCompactActionDisabledReason({ ...readyAction, actionId: "refresh" }),
    undefined,
  );
});
