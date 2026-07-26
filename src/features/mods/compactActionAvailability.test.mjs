import assert from "node:assert/strict";
import { test } from "node:test";
import { getCompactActionDisabledReason } from "./compactActionAvailability.ts";
import { compactActions } from "./modsLibraryData.ts";

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
    "批量操作暂未开放，请只选择一个 MOD",
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

test("no compact action is unconditionally disabled", () => {
  /*
   * "启用全部 MOD" / "禁用全部 MOD" 曾经无条件返回"暂不可用"，且 handleAction 里连 case 都没有——
   * 任何状态下都点不动，只是常驻工具栏的噪音。它们已随本次改动移除，等 T13 批量操作落地再回来。
   *
   * 这条断言防止再引入同类按钮：面板里的每个动作，都必须存在至少一种可用状态。
   */
  const actionIds = compactActions.map((action) => action.id);
  assert.ok(actionIds.length > 0);
  assert.ok(!actionIds.includes("enable-all"));
  assert.ok(!actionIds.includes("disable-all"));

  for (const actionId of actionIds) {
    // add / add-revision 由 ModImportAction 自行判定可用性，不走本函数。
    if (actionId === "add" || actionId === "add-revision") {
      continue;
    }

    const reason = getCompactActionDisabledReason({
      ...readyAction,
      actionId,
      canInstallSelection: true,
      canReinstallSelection: true,
      canUninstallSelection: true,
    });
    assert.equal(reason, undefined, `动作 ${actionId} 在完全就绪状态下仍不可用，属于死按钮`);
  }
});

test("selection-independent actions stay available", () => {
  assert.equal(
    getCompactActionDisabledReason({ ...readyAction, actionId: "refresh" }),
    undefined,
  );
});
