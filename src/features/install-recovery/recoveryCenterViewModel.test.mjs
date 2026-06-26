import assert from "node:assert/strict";
import { test } from "node:test";

import { deriveRecoveryCenterViewModel } from "./recoveryCenterViewModel.ts";

const baseSummary = {
  profileId: "default",
  managedFileCount: 0,
  backupCount: 0,
  issueCount: 0,
  issues: [],
};

test("derives profile recovery center overview without path fields", () => {
  const viewModel = deriveRecoveryCenterViewModel([
    {
      ...baseSummary,
      modId: "healthy-mod",
      status: "completed",
      managedFileCount: 2,
      backupCount: 1,
    },
    {
      ...baseSummary,
      modId: "changed-mod",
      status: "repair_required",
      managedFileCount: 3,
      backupCount: 1,
      issueCount: 3,
      issues: [
        { issue: "target_changed", count: 2 },
        { issue: "backup_missing", count: 1 },
      ],
    },
    {
      ...baseSummary,
      modId: "unknown-mod",
      status: "unknown",
      managedFileCount: 1,
      issueCount: 1,
      issues: [{ issue: "target_read_failed", count: 1 }],
    },
  ]);

  assert.equal(viewModel.overview.status, "attention");
  assert.equal(viewModel.overview.scannedModCount, 3);
  assert.equal(viewModel.overview.completedModCount, 1);
  assert.equal(viewModel.overview.attentionModCount, 1);
  assert.equal(viewModel.overview.unknownModCount, 1);
  assert.equal(viewModel.overview.managedFileCount, 6);
  assert.equal(viewModel.overview.backupCount, 2);
  assert.equal(viewModel.overview.issueCount, 4);
  assert.deepEqual(viewModel.overview.issues, [
    {
      issue: "target_changed",
      count: 2,
      label: "目标变更",
      severity: "blocking",
      guidance: "暂停自动安装/卸载，等待受控恢复或重新安装流程确认目标状态。",
    },
    {
      issue: "target_read_failed",
      count: 1,
      label: "读取未知",
      severity: "unknown",
      guidance: "重新扫描；如果仍不可读，先检查权限或占用状态。",
    },
    {
      issue: "backup_missing",
      count: 1,
      label: "备份缺失",
      severity: "blocking",
      guidance: "不要自动恢复或卸载，先保留当前文件并进入人工确认。",
    },
  ]);
  assert.deepEqual(
    viewModel.mods.map((mod) => [mod.modId, mod.status, mod.statusLabel]),
    [
      ["changed-mod", "repair_required", "需要修复"],
      ["unknown-mod", "unknown", "状态未知"],
      ["healthy-mod", "completed", "正常"],
    ],
  );
  assert.equal("targetPath" in viewModel.overview, false);
  assert.equal("gameRoot" in viewModel.overview, false);
  assert.equal("backupRef" in viewModel.mods[0], false);
  assert.equal("manifestPath" in viewModel.mods[0], false);
});

test("derives read-only rich repair summary for unsafe recovery states", () => {
  const viewModel = deriveRecoveryCenterViewModel([
    {
      ...baseSummary,
      modId: "changed-mod",
      status: "repair_required",
      managedFileCount: 3,
      backupCount: 1,
      issueCount: 3,
      issues: [
        { issue: "target_changed", count: 2 },
        { issue: "backup_missing", count: 1 },
      ],
    },
    {
      ...baseSummary,
      modId: "unknown-mod",
      status: "unknown",
      managedFileCount: 1,
      issueCount: 1,
      issues: [{ issue: "target_read_failed", count: 1 }],
    },
  ]);

  assert.deepEqual(viewModel.overview.repairSummary, {
    status: "unknown",
    title: "恢复状态需要人工确认",
    description: "部分托管安装状态无法读取，自动安装、卸载和恢复都应保持阻断。",
    actionLabel: "刷新后仍异常则保留现场并人工处理",
    blockingReason: "存在 1 个状态未知 Mod 和 1 个需要修复 Mod",
  });

  assert.equal(viewModel.mods[0].repairSummary.status, "manual_required");
  assert.equal(viewModel.mods[0].repairSummary.blockingReason, "检测到 3 个恢复问题");
  assert.deepEqual(
    viewModel.mods[0].issues.map((issue) => [issue.issue, issue.severity, issue.guidance]),
    [
      ["target_changed", "blocking", "暂停自动安装/卸载，等待受控恢复或重新安装流程确认目标状态。"],
      ["backup_missing", "blocking", "不要自动恢复或卸载，先保留当前文件并进入人工确认。"],
    ],
  );
  assert.equal(viewModel.mods[1].repairSummary.status, "unknown");
  assert.equal(viewModel.mods[1].issues[0].guidance, "重新扫描；如果仍不可读，先检查权限或占用状态。");
  assert.equal("targetPath" in viewModel.overview.repairSummary, false);
  assert.equal("backupRef" in viewModel.mods[0].repairSummary, false);
  assert.equal("manifestPath" in viewModel.mods[0].issues[0], false);
});

test("derives safe manual handling decisions for recovery attention states", () => {
  const viewModel = deriveRecoveryCenterViewModel([
    {
      ...baseSummary,
      modId: "changed-mod",
      status: "repair_required",
      managedFileCount: 2,
      backupCount: 1,
      issueCount: 2,
      issues: [{ issue: "target_changed", count: 2 }],
    },
    {
      ...baseSummary,
      modId: "unknown-mod",
      status: "unknown",
      managedFileCount: 1,
      issueCount: 1,
      issues: [{ issue: "backup_read_failed", count: 1 }],
    },
  ]);

  assert.deepEqual(viewModel.overview.manualDecision, {
    status: "blocked",
    title: "需要人工处理",
    description: "恢复中心已阻断自动安装、卸载和恢复动作，当前只能执行只读复查或导出诊断。",
    recommendedAction: "先重新扫描；如果仍异常，导出诊断并保留现场。",
    safeguards: [
      "不删除未知文件",
      "不根据当前 Mod 包猜测恢复动作",
      "不写入 manifest 或 backup 状态",
    ],
    actions: [
      {
        id: "retry_scan",
        label: "重新扫描",
        description: "重新读取后端只读恢复摘要。",
        state: "available",
      },
      {
        id: "export_diagnostics",
        label: "导出诊断",
        description: "生成已脱敏的支持诊断包。",
        state: "available",
      },
      {
        id: "controlled_recovery",
        label: "受控修复",
        description: "需要后续 manifest 状态机和恢复执行器支持，当前不可用。",
        state: "unavailable",
      },
    ],
  });
  assert.equal("targetPath" in viewModel.overview.manualDecision, false);
  assert.equal("backupRef" in viewModel.overview.manualDecision.actions[0], false);
  assert.equal("manifestPath" in viewModel.overview.manualDecision.actions[2], false);
});

test("derives empty recovery center state for a profile without managed installs", () => {
  const viewModel = deriveRecoveryCenterViewModel([]);

  assert.equal(viewModel.overview.status, "empty");
  assert.deepEqual(viewModel.overview.repairSummary, {
    status: "clear",
    title: "无需处理",
    description: "当前配置档没有需要恢复中心处理的托管安装状态。",
    actionLabel: "保持观察",
    blockingReason: "没有托管安装记录",
  });
  assert.deepEqual(viewModel.overview.manualDecision, {
    status: "clear",
    title: "无需人工处理",
    description: "当前没有需要恢复中心人工处理的托管安装状态。",
    recommendedAction: "保持观察。",
    safeguards: [],
    actions: [
      {
        id: "retry_scan",
        label: "重新扫描",
        description: "重新读取后端只读恢复摘要。",
        state: "available",
      },
      {
        id: "export_diagnostics",
        label: "导出诊断",
        description: "生成已脱敏的支持诊断包。",
        state: "available",
      },
    ],
  });
  assert.equal(viewModel.mods.length, 0);
  assert.equal(viewModel.overview.scannedModCount, 0);
  assert.equal(viewModel.overview.issueCount, 0);
});
