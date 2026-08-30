import assert from "node:assert/strict";
import { test } from "node:test";
import { getCompactActionDisabledReason } from "./compactActionAvailability.ts";
import { modLibraryCopy } from "./modLibraryCopy.ts";

// I18N-02 起该函数按当前界面语言取词；测试固定用 zh_cn 字典钉住产品文案。
const zhCompact = modLibraryCopy.zh_cn.compact;
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
  canDeleteSelection: true,
};

test("compact lifecycle actions explain selection, task, profile, and durable-state blockers in priority order", () => {
  assert.equal(
    getCompactActionDisabledReason({ ...readyAction, selectedCount: 0 }, zhCompact),
    "请先选择一个 MOD",
  );
  // T13-07: multi-selection no longer blocks lifecycle actions; feasibility falls through
  // to the per-operation canXxxSelection facts (batch preview filters inapplicable items).
  assert.equal(
    getCompactActionDisabledReason({ ...readyAction, selectedCount: 2 }, zhCompact),
    undefined,
  );
  assert.equal(
    getCompactActionDisabledReason({ ...readyAction, selectedCount: 2, canInstallSelection: false }, zhCompact),
    "仅未安装且状态安全的 MOD 可安装",
  );
  assert.equal(
    getCompactActionDisabledReason({ ...readyAction, installTaskActive: true }, zhCompact),
    "请等待当前安装任务完成",
  );
  assert.equal(
    getCompactActionDisabledReason({ ...readyAction, profileReady: false }, zhCompact),
    "选择配置档后可安装",
  );
  assert.equal(
    getCompactActionDisabledReason({ ...readyAction, canInstallSelection: false }, zhCompact),
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
    "delete",
  ]) {
    assert.equal(
      getCompactActionDisabledReason({ ...readyAction, actionId, libraryQueryBusy: true }, zhCompact),
      "Mod 列表正在更新，请稍候",
    );
  }

  assert.equal(
    getCompactActionDisabledReason({ ...readyAction, actionId: "refresh", libraryQueryBusy: true }, zhCompact),
    undefined,
  );
});

test("preview, reinstall, and uninstall expose action-specific fail-closed reasons", () => {
  assert.equal(
    getCompactActionDisabledReason({
      ...readyAction,
      actionId: "preview-plan",
      canInstallSelection: false,
    }, zhCompact),
    "仅未安装且状态安全的 MOD 可预览安装计划",
  );
  assert.equal(
    getCompactActionDisabledReason({ ...readyAction, actionId: "reinstall" }, zhCompact),
    "仅已安装且状态安全的 MOD 可重装",
  );
  assert.equal(
    getCompactActionDisabledReason({ ...readyAction, actionId: "uninstall" }, zhCompact),
    "仅已安装且状态安全的 MOD 可卸载",
  );
  assert.equal(
    getCompactActionDisabledReason(
      { ...readyAction, actionId: "delete", canDeleteSelection: false },
      zhCompact,
    ),
    "仅未安装的 MOD 可删除；已安装的请先卸载",
  );
  assert.equal(
    getCompactActionDisabledReason({
      ...readyAction,
      actionId: "reinstall",
      canReinstallSelection: true,
    }, zhCompact),
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
  assert.ok(!actionIds.includes("enable-all"));
  assert.ok(!actionIds.includes("disable-all"));

  /*
   * 显式断言必须存在的动作。只遍历"已存在的动作"是不够的——误删 select-all 或 invert
   * 会让循环少跑一轮而静默通过，正好漏掉这条断言本该保护的东西。
   */
  for (const requiredId of [
    "select-all",
    "invert",
    "refresh",
    "preview-plan",
    "install",
    "reinstall",
    "uninstall",
    "delete",
  ]) {
    assert.ok(actionIds.includes(requiredId), `快捷操作栏缺少动作 ${requiredId}`);
  }

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
    }, zhCompact);
    assert.equal(reason, undefined, `动作 ${actionId} 在完全就绪状态下仍不可用，属于死按钮`);
  }
});

test("selection-independent actions stay available", () => {
  assert.equal(
    getCompactActionDisabledReason({ ...readyAction, actionId: "refresh" }, zhCompact),
    undefined,
  );
});
