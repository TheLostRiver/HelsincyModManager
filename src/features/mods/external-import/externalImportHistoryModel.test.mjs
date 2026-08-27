import assert from "node:assert/strict";
import { test } from "node:test";

import { externalImportCopy } from "./externalImportCopy.ts";
import {
  appendExternalImportHistoryRows,
  formatExternalImportHistoryTime,
  getExternalImportHistoryErrorMessage,
  isExternalImportHistoryPage,
  resolveExternalImportHistoryState,
  toExternalImportHistoryRow,
} from "./externalImportHistoryModel.ts";

const zhHistory = externalImportCopy.zh_cn.history;

function counts(overrides = {}) {
  const base = {
    total: 3,
    imported: 2,
    alreadyImported: 0,
    skipped: 0,
    blocked: 0,
    failed: 1,
    cancelled: 0,
  };
  return { ...base, ...overrides };
}

function entry(overrides = {}) {
  return {
    batchId: "batch-a",
    adapterId: "hunting_box_directory_v1",
    scanStatus: "completed",
    importStatus: "completed_with_errors",
    createdAtUnixMillis: 1_724_000_000_000,
    candidateCount: 5,
    counts: counts(),
    ...overrides,
  };
}

function historyPage(overrides = {}) {
  return {
    batches: [entry()],
    totalCount: 1,
    nextCursor: null,
    ...overrides,
  };
}

test("history page accepts only the exact redacted contract", () => {
  assert.equal(isExternalImportHistoryPage(historyPage()), true);
  // running 批次正常列出:用户在导入过程中切到记录页不该看到空洞。
  assert.equal(
    isExternalImportHistoryPage(
      historyPage({
        batches: [entry({ importStatus: "running", counts: counts({ total: 0, imported: 0, failed: 0 }) })],
      }),
    ),
    true,
  );
  assert.equal(
    isExternalImportHistoryPage(historyPage({ sourceFingerprint: "private" })),
    false,
  );
  assert.equal(
    isExternalImportHistoryPage(
      historyPage({ batches: [entry({ selectionId: "leaked" })] }),
    ),
    false,
  );
  const legacyEntry = entry();
  delete legacyEntry.candidateCount;
  assert.equal(isExternalImportHistoryPage(historyPage({ batches: [legacyEntry] })), false);
  assert.equal(
    isExternalImportHistoryPage(
      historyPage({ batches: [entry({ batchId: "batch/path" })] }),
    ),
    false,
  );
  assert.equal(
    isExternalImportHistoryPage(
      historyPage({ batches: [entry({ importStatus: "future_status" })] }),
    ),
    false,
  );
  // 计数必须能对上分项之和,对不上视为后端契约漂移,整体 fail closed。
  assert.equal(
    isExternalImportHistoryPage(
      historyPage({ batches: [entry({ counts: counts({ total: 99 }) })] }),
    ),
    false,
  );
  assert.equal(
    isExternalImportHistoryPage(
      historyPage({ batches: [entry(), entry()], totalCount: 2 }),
    ),
    false,
  );
  assert.equal(
    isExternalImportHistoryPage(historyPage({ totalCount: 0 })),
    false,
  );
  assert.equal(
    isExternalImportHistoryPage(historyPage({ nextCursor: "next/page" })),
    false,
  );
  assert.equal(
    isExternalImportHistoryPage(historyPage({ totalCount: 2, nextCursor: "01" })),
    false,
  );
  assert.equal(
    isExternalImportHistoryPage(historyPage({ totalCount: 2, nextCursor: "2" })),
    false,
  );
  assert.equal(
    isExternalImportHistoryPage(historyPage({ totalCount: 3, nextCursor: "1" })),
    true,
  );

  const oversized = Array.from({ length: 51 }, (_, index) =>
    entry({ batchId: `batch-${index}` }),
  );
  assert.equal(
    isExternalImportHistoryPage(historyPage({ batches: oversized, totalCount: 51 })),
    false,
  );
});

test("history state derivation collapses scan/import status into one user-facing word", () => {
  assert.deepEqual(resolveExternalImportHistoryState("completed", "running"), {
    key: "running",
    tone: "progress",
  });
  assert.deepEqual(resolveExternalImportHistoryState("completed", "completed"), {
    key: "completed",
    tone: "ready",
  });
  assert.deepEqual(resolveExternalImportHistoryState("completed", "completed_with_errors"), {
    key: "completedWithErrors",
    tone: "warning",
  });
  // 启动恢复把中断 running 收敛为 failed,历史用中性「未完成」表述。
  assert.deepEqual(resolveExternalImportHistoryState("completed", "failed"), {
    key: "incomplete",
    tone: "warning",
  });
  assert.deepEqual(resolveExternalImportHistoryState("completed", "pending"), {
    key: "scanOnly",
    tone: "neutral",
  });
  assert.deepEqual(resolveExternalImportHistoryState("failed", "pending"), {
    key: "scanFailed",
    tone: "danger",
  });
  assert.deepEqual(resolveExternalImportHistoryState("running", "pending"), {
    key: "scanning",
    tone: "progress",
  });
  assert.deepEqual(resolveExternalImportHistoryState("cancelled", "pending"), {
    key: "cancelled",
    tone: "neutral",
  });
});

test("history time labels stay relative within seven days and absolute beyond", () => {
  const now = 1_724_000_000_000;
  const time = zhHistory.time;
  assert.equal(formatExternalImportHistoryTime(now - 59_000, now, time, "zh_cn"), "刚刚");
  assert.equal(formatExternalImportHistoryTime(now - 60_000, now, time, "zh_cn"), "1 分钟前");
  assert.equal(
    formatExternalImportHistoryTime(now - 23 * 60 * 60_000, now, time, "zh_cn"),
    "23 小时前",
  );
  assert.equal(
    formatExternalImportHistoryTime(now - 6 * 24 * 60 * 60_000, now, time, "zh_cn"),
    "6 天前",
  );
  const absolute = formatExternalImportHistoryTime(
    now - 8 * 24 * 60 * 60_000,
    now,
    time,
    "zh_cn",
  );
  assert.doesNotMatch(absolute, /前|刚刚/);
  // 时钟漂移(created_at 晚于 now)不得出现负数,按「刚刚」处理。
  assert.equal(formatExternalImportHistoryTime(now + 5_000, now, time, "zh_cn"), "刚刚");
});

test("history rows map adapter labels, fall back for unknown adapters, and dedupe on append", () => {
  const now = 1_724_000_000_000;
  const row = toExternalImportHistoryRow(entry(), zhHistory, "zh_cn", now);
  assert.equal(row.adapterLabel, "狩技盒子");
  assert.equal(row.stateKey, "completedWithErrors");
  assert.equal(row.hasDetails, true);
  assert.equal(row.total, 3);

  const unknown = toExternalImportHistoryRow(
    entry({ adapterId: "future_adapter_v9" }),
    zhHistory,
    "zh_cn",
    now,
  );
  assert.equal(unknown.adapterLabel, zhHistory.unknownAdapter);

  const scanOnly = toExternalImportHistoryRow(
    entry({
      importStatus: "pending",
      counts: counts({ total: 0, imported: 0, failed: 0 }),
    }),
    zhHistory,
    "zh_cn",
    now,
  );
  assert.equal(scanOnly.hasDetails, false);

  const merged = appendExternalImportHistoryRows(
    [row],
    [entry(), entry({ batchId: "batch-b" })],
    zhHistory,
    "zh_cn",
    now,
  );
  assert.deepEqual(
    merged.map((mergedRow) => mergedRow.batchId),
    ["batch-a", "batch-b"],
  );
});

test("history error copy stays stable for unknown codes", () => {
  assert.equal(
    getExternalImportHistoryErrorMessage("C:\\private\\raw-error", zhHistory),
    zhHistory.fallbackError,
  );
  assert.equal(
    getExternalImportHistoryErrorMessage("external_import_batch_unavailable", zhHistory),
    zhHistory.errors.external_import_batch_unavailable,
  );
});
